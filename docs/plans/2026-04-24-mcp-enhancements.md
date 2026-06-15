# MCP Enhancement: Metadata, Sampling, Pagination, Save-As Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add 4 new MCP tools to grep-excel: `get_metadata`, `get_sheet_sample`, `get_sheet_data`, and `save_as`.

**Architecture:** Extend the existing `SearchEngine` trait with new methods, implement them across all 3 engine backends (memory, duckdb, sqlite), and expose them as MCP tools via rmcp. Add `rust_xlsxwriter` dependency for Excel writing capability.

**Tech Stack:** Rust, rmcp (MCP protocol), calamine (Excel reading), rust_xlsxwriter (Excel writing), existing engine infrastructure.

---

## Existing Codebase Context

### Key Files
- `src/mcp.rs` — MCP tool handlers, defines `GrepExcelServer` with `#[tool]` methods
- `src/engine/mod.rs` — `SearchEngine` trait definition + shared helpers
- `src/engine/memory.rs` — `MemEngine` (default, stores sheets as `Vec<MemSheet>`)
- `src/engine/duckdb.rs` — `DuckDbEngine` (uses DuckDB tables, `sheets` meta-table stores `col_names` as `\x1f`-separated)
- `src/engine/sqlite.rs` — `SqliteEngine` (same schema as duckdb but SQLite SQL syntax)
- `src/types.rs` — Shared types: `FileInfo`, `FileSample`, `SearchResult`, `SqlResult`, etc.
- `src/excel.rs` — Excel/CSV parsing with `SheetData` struct
- `Cargo.toml` — Dependencies with feature flags: `mcp-server`, `engine-memory`, `engine-duckdb`, `engine-sqlite`

### Sheet Identification Convention
- Files are identified by **basename** (e.g., `"data.xlsx"`)
- Sheets within a file are identified by **sheet name** (e.g., `"Employees"`)
- In SQL engines, tables are named `sheet_{file_id}_{sheet_idx}` and metadata is stored in `sheets` table with `col_names` field (`\x1f`-separated)

### MCP Pattern (from existing code)
- Tool params use `#[derive(Debug, Deserialize, schemars::JsonSchema)]`
- Handlers are `async fn` with `Parameters(params): Parameters<XxxParams>`
- Engine access: `db.read()` for reads, `db.write()` for writes
- All engine calls are wrapped in `tokio::task::spawn_blocking`
- Responses are JSON strings via `serde_json::to_string_pretty`

---

### Task 1: Add New Types

**Files:**
- Modify: `src/types.rs`

**Step 1: Add new types to `types.rs`**

Add these types at the end of the file:

```rust
#[derive(Debug, Clone)]
pub struct SheetMetadataInfo {
    pub sheet_name: String,
    pub row_count: usize,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FileMetadataInfo {
    pub file_name: String,
    pub sheet_count: usize,
    pub sheets: Vec<SheetMetadataInfo>,
}

#[derive(Debug, Clone)]
pub struct SheetDataResult {
    pub file_name: String,
    pub sheet_name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub total_rows: usize,
    pub truncated: bool,
}
```

**Step 2: Verify compilation**

Run: `cargo check 2>&1 | head -20`
Expected: Clean (no errors from types.rs)

**Step 3: Commit**

```bash
git add src/types.rs
git commit -m "feat: add new types for MCP metadata, sampling, and pagination"
```

---

### Task 2: Extend SearchEngine Trait

**Files:**
- Modify: `src/engine/mod.rs`

**Step 1: Add new methods to the trait**

Add these method signatures to the `SearchEngine` trait (after `execute_sql`):

```rust
fn get_metadata(&self, file_name: &str) -> Result<FileMetadataInfo>;
fn get_sheet_sample(&self, file_name: &str, sheet_name: &str, sample_size: usize) -> Result<SheetDataResult>;
fn get_sheet_data(
    &self,
    file_name: &str,
    sheet_name: &str,
    start_row: Option<usize>,
    end_row: Option<usize>,
    columns: Option<&[String]>,
) -> Result<SheetDataResult>;
fn save_as(&self, file_name: &str, output_path: &Path) -> Result<()>;
```

Note: The import `use crate::types::*;` at the top of mod.rs already brings in all types.

**Step 2: Verify compilation fails** (expected — engines don't implement new methods yet)

Run: `cargo check 2>&1 | head -30`
Expected: Errors about missing method implementations

---

### Task 3: Implement New Methods in MemEngine

**Files:**
- Modify: `src/engine/memory.rs`

**Step 1: Implement `get_metadata`**

In `impl SearchEngine for MemEngine`, add:

```rust
fn get_metadata(&self, file_name: &str) -> Result<FileMetadataInfo> {
    let sheets: Vec<&MemSheet> = self.sheets.iter()
        .filter(|s| s.file_name == file_name)
        .collect();

    if sheets.is_empty() {
        anyhow::bail!("File '{}' not found. Use list_files to see imported files.", file_name);
    }

    let sheet_infos: Vec<SheetMetadataInfo> = sheets.iter()
        .map(|s| SheetMetadataInfo {
            sheet_name: s.sheet_name.clone(),
            row_count: s.rows.len(),
            columns: s.headers.clone(),
        })
        .collect();

    Ok(FileMetadataInfo {
        file_name: file_name.to_string(),
        sheet_count: sheet_infos.len(),
        sheets: sheet_infos,
    })
}
```

**Step 2: Implement `get_sheet_sample`**

```rust
fn get_sheet_sample(&self, file_name: &str, sheet_name: &str, sample_size: usize) -> Result<SheetDataResult> {
    let sheet = self.sheets.iter()
        .find(|s| s.file_name == file_name && s.sheet_name == sheet_name)
        .ok_or_else(|| anyhow::anyhow!(
            "Sheet '{}' in file '{}' not found. Use get_metadata to discover sheets.",
            sheet_name, file_name
        ))?;

    let total_rows = sheet.rows.len();
    let sample_size = sample_size.min(total_rows);

    // Deterministic sampling: pick evenly spaced rows
    let mut sampled = Vec::new();
    if sample_size > 0 && total_rows > 0 {
        if sample_size >= total_rows {
            sampled = sheet.rows.clone();
        } else {
            for i in 0..sample_size {
                let idx = i * total_rows / sample_size;
                sampled.push(sheet.rows[idx].clone());
            }
        }
    }

    Ok(SheetDataResult {
        file_name: file_name.to_string(),
        sheet_name: sheet_name.to_string(),
        columns: sheet.headers.clone(),
        rows: sampled,
        row_count: sample_size.min(total_rows),
        total_rows,
        truncated: sample_size < total_rows,
    })
}
```

**Step 3: Implement `get_sheet_data`**

```rust
fn get_sheet_data(
    &self,
    file_name: &str,
    sheet_name: &str,
    start_row: Option<usize>,
    end_row: Option<usize>,
    columns: Option<&[String]>,
) -> Result<SheetDataResult> {
    let sheet = self.sheets.iter()
        .find(|s| s.file_name == file_name && s.sheet_name == sheet_name)
        .ok_or_else(|| anyhow::anyhow!(
            "Sheet '{}' in file '{}' not found. Use get_metadata to discover sheets.",
            sheet_name, file_name
        ))?;

    let total_rows = sheet.rows.len();
    let start = start_row.unwrap_or(0).min(total_rows);
    let end = end_row.unwrap_or(total_rows).min(total_rows);

    let rows_slice = &sheet.rows[start..end];

    // Filter columns if specified
    let (col_indices, result_columns): (Vec<usize>, Vec<String>) = if let Some(cols) = columns {
        let indices: Vec<usize> = cols.iter()
            .filter_map(|c| sheet.headers.iter().position(|h| h == c))
            .collect();
        let names: Vec<String> = indices.iter()
            .map(|&i| sheet.headers[i].clone())
            .collect();
        (indices, names)
    } else {
        let indices: Vec<usize> = (0..sheet.headers.len()).collect();
        (indices, sheet.headers.clone())
    };

    let result_rows: Vec<Vec<String>> = rows_slice.iter()
        .map(|row| {
            col_indices.iter()
                .map(|&i| row.get(i).cloned().unwrap_or_default())
                .collect()
        })
        .collect();

    Ok(SheetDataResult {
        file_name: file_name.to_string(),
        sheet_name: sheet_name.to_string(),
        columns: result_columns,
        rows: result_rows,
        row_count: result_rows.len(),
        total_rows,
        truncated: false,
    })
}
```

**Step 4: Implement `save_as`**

```rust
fn save_as(&self, file_name: &str, output_path: &Path) -> Result<()> {
    use crate::engine::write_xlsx;

    let sheets: Vec<&MemSheet> = self.sheets.iter()
        .filter(|s| s.file_name == file_name)
        .collect();

    if sheets.is_empty() {
        anyhow::bail!("File '{}' not found. Use list_files to see imported files.", file_name);
    }

    let sheet_data: Vec<(&str, &[String], &[Vec<String>])> = sheets.iter()
        .map(|s| (s.sheet_name.as_str(), &s.headers[..], &s.rows[..]))
        .collect();

    write_xlsx(&sheet_data, output_path)
}
```

**Step 5: Commit**

```bash
git add src/engine/memory.rs
git commit -m "feat: implement new trait methods in MemEngine"
```

---

### Task 4: Implement New Methods in DuckDbEngine

**Files:**
- Modify: `src/engine/duckdb.rs`

**Step 1: Implement `get_metadata`**

In `impl SearchEngine for DuckDbEngine`, add:

```rust
fn get_metadata(&self, file_name: &str) -> Result<FileMetadataInfo> {
    let mut stmt = self.conn.prepare(
        "SELECT s.sheet_name, s.row_count, s.col_names
         FROM sheets s JOIN files f ON s.file_id = f.file_id
         WHERE f.file_name = ?
         ORDER BY s.sheet_id"
    )?;

    let sheet_infos: Vec<SheetMetadataInfo> = stmt.query_map(params![file_name], |row| {
        let sheet_name: String = row.get(0)?;
        let row_count: i32 = row.get(1)?;
        let col_names_str: String = row.get(2)?;
        let columns: Vec<String> = if col_names_str.is_empty() {
            vec![]
        } else {
            col_names_str.split('\x1f').map(|s| s.to_string()).collect()
        };
        Ok(SheetMetadataInfo {
            sheet_name,
            row_count: row_count as usize,
            columns,
        })
    })?.collect::<Result<Vec<_>, _>>()?;

    if sheet_infos.is_empty() {
        anyhow::bail!("File '{}' not found. Use list_files to see imported files.", file_name);
    }

    Ok(FileMetadataInfo {
        file_name: file_name.to_string(),
        sheet_count: sheet_infos.len(),
        sheets: sheet_infos,
    })
}
```

**Step 2: Implement `get_sheet_sample`**

```rust
fn get_sheet_sample(&self, file_name: &str, sheet_name: &str, sample_size: usize) -> Result<SheetDataResult> {
    let meta = self.get_sheet_metadata_query(file_name, sheet_name)?;

    let col_list: String = meta.col_names.iter()
        .map(|c| quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        "SELECT {} FROM {} USING SAMPLE {}",
        col_list,
        quote_ident(&meta.table_name),
        sample_size
    );

    let rows = self.query_rows(&sql, &meta.col_names)?;
    let total_rows = meta.row_count;
    let row_count = rows.len();

    Ok(SheetDataResult {
        file_name: file_name.to_string(),
        sheet_name: sheet_name.to_string(),
        columns: meta.col_names,
        rows,
        row_count,
        total_rows,
        truncated: row_count < total_rows,
    })
}
```

**Step 3: Implement `get_sheet_data`**

```rust
fn get_sheet_data(
    &self,
    file_name: &str,
    sheet_name: &str,
    start_row: Option<usize>,
    end_row: Option<usize>,
    columns: Option<&[String]>,
) -> Result<SheetDataResult> {
    let meta = self.get_sheet_metadata_query(file_name, sheet_name)?;

    let selected_cols: Vec<String> = if let Some(cols) = columns {
        cols.to_vec()
    } else {
        meta.col_names.clone()
    };

    let col_indices: Vec<usize> = selected_cols.iter()
        .filter_map(|c| meta.col_names.iter().position(|h| h == c))
        .collect();
    let col_names: Vec<String> = col_indices.iter()
        .map(|&i| meta.col_names[i].clone())
        .collect();

    let col_list: String = col_names.iter()
        .map(|c| quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");

    let start = start_row.unwrap_or(0);
    let limit = end_row.unwrap_or(meta.row_count).saturating_sub(start);

    let sql = format!(
        "SELECT {} FROM {} LIMIT {} OFFSET {}",
        col_list,
        quote_ident(&meta.table_name),
        limit,
        start
    );

    let rows = self.query_rows(&sql, &col_names)?;
    let total_rows = meta.row_count;

    Ok(SheetDataResult {
        file_name: file_name.to_string(),
        sheet_name: sheet_name.to_string(),
        columns: col_names,
        rows,
        row_count: rows.len(),
        total_rows,
        truncated: false,
    })
}
```

**Step 4: Implement `save_as` and helper methods**

Add helper methods to `impl DuckDbEngine` (the non-trait impl block):

```rust
struct SheetQueryMeta {
    table_name: String,
    col_names: Vec<String>,
    row_count: usize,
}

fn get_sheet_metadata_query(&self, file_name: &str, sheet_name: &str) -> Result<SheetQueryMeta> {
    let result = self.conn.query_row(
        "SELECT s.table_name, s.col_names, s.row_count
         FROM sheets s JOIN files f ON s.file_id = f.file_id
         WHERE f.file_name = ? AND s.sheet_name = ?",
        params![file_name, sheet_name],
        |row| {
            let table_name: String = row.get(0)?;
            let col_names_str: String = row.get(1)?;
            let col_names: Vec<String> = if col_names_str.is_empty() {
                vec![]
            } else {
                col_names_str.split('\x1f').map(|s| s.to_string()).collect()
            };
            let row_count: i32 = row.get(2)?;
            Ok((table_name, col_names, row_count as usize))
        }
    );

    match result {
        Ok((table_name, col_names, row_count)) => Ok(SheetQueryMeta { table_name, col_names, row_count }),
        Err(_) => anyhow::bail!(
            "Sheet '{}' in file '{}' not found. Use get_metadata to discover sheets.",
            sheet_name, file_name
        ),
    }
}

fn query_rows(&self, sql: &str, col_names: &[String]) -> Result<Vec<Vec<String>>> {
    let mut stmt = self.conn.prepare(sql)?;
    let col_count = col_names.len();
    let rows: Vec<Vec<String>> = stmt.query_map([], |row| {
        let mut values = Vec::with_capacity(col_count);
        for i in 0..col_count {
            values.push(row.get::<_, Option<String>>(i)?.unwrap_or_default());
        }
        Ok(values)
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
```

For `save_as` in the trait impl:

```rust
fn save_as(&self, file_name: &str, output_path: &Path) -> Result<()> {
    use crate::engine::write_xlsx;

    let mut stmt = self.conn.prepare(
        "SELECT s.sheet_name, s.table_name, s.col_names, s.row_count
         FROM sheets s JOIN files f ON s.file_id = f.file_id
         WHERE f.file_name = ?
         ORDER BY s.sheet_id"
    )?;

    let sheet_rows: Vec<(String, String, Vec<String>)> = stmt.query_map(params![file_name], |row| {
        let sheet_name: String = row.get(0)?;
        let table_name: String = row.get(1)?;
        let col_names_str: String = row.get(2)?;
        let col_names: Vec<String> = if col_names_str.is_empty() {
            vec![]
        } else {
            col_names_str.split('\x1f').map(|s| s.to_string()).collect()
        };
        Ok((sheet_name, table_name, col_names))
    })?.collect::<Result<Vec<_>, _>>()?;

    if sheet_rows.is_empty() {
        anyhow::bail!("File '{}' not found. Use list_files to see imported files.", file_name);
    }

    // Query all data for each sheet and write to xlsx
    let mut sheets_data: Vec<(String, Vec<String>, Vec<Vec<String>>)> = Vec::new();
    for (sheet_name, table_name, col_names) in &sheet_rows {
        let col_list: String = col_names.iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!("SELECT {} FROM {}", col_list, quote_ident(table_name));
        let rows = self.query_rows(&sql, col_names)?;
        sheets_data.push((sheet_name.clone(), col_names.clone(), rows));
    }

    let refs: Vec<(&str, &[String], &[Vec<String>])> = sheets_data.iter()
        .map(|(name, headers, rows)| (name.as_str(), &headers[..], &rows[..]))
        .collect();

    write_xlsx(&refs, output_path)
}
```

**Step 5: Commit**

```bash
git add src/engine/duckdb.rs
git commit -m "feat: implement new trait methods in DuckDbEngine"
```

---

### Task 5: Implement New Methods in SqliteEngine

**Files:**
- Modify: `src/engine/sqlite.rs`

Follow the same pattern as DuckDbEngine but with SQLite-compatible SQL:
- Use `ORDER BY RANDOM() LIMIT n` for sampling (DuckDB has `USING SAMPLE n`)
- Use `LIMIT n OFFSET m` for pagination (same as DuckDB)
- Use `last_insert_rowid()` instead of `currval()` (already done in existing code)
- Same `get_sheet_metadata_query` and `query_rows` helper pattern

**Step 1: Implement all 4 trait methods + helpers**

The implementation mirrors DuckDbEngine exactly, except:
- `get_sheet_sample`: Use `SELECT ... FROM {} ORDER BY RANDOM() LIMIT {}` instead of `USING SAMPLE {}`
- `query_rows`: Use `rusqlite` API instead of `duckdb` API (same pattern, different types)
- `get_sheet_metadata_query`: Same SQL, use `rusqlite` params

**Step 2: Commit**

```bash
git add src/engine/sqlite.rs
git commit -m "feat: implement new trait methods in SqliteEngine"
```

---

### Task 6: Add `write_xlsx` Helper and `rust_xlsxwriter` Dependency

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/engine/mod.rs`

**Step 1: Add dependency to `Cargo.toml`**

Add `rust_xlsxwriter` to `[dependencies]`:
```toml
rust_xlsxwriter = { version = "0.82", optional = true }
```

Add to `mcp-server` feature:
```toml
mcp-server = ["dep:rmcp", "dep:tokio", "dep:serde", "dep:serde_json", "dep:schemars", "dep:rust_xlsxwriter"]
```

**Step 2: Add `write_xlsx` function to `engine/mod.rs`**

Add this function in the shared helpers section (after `validate_sql`):

```rust
/// Write multiple sheets to a new xlsx file (Save As).
/// Each tuple is (sheet_name, headers, rows).
#[cfg(feature = "rust_xlsxwriter")]
pub fn write_xlsx(
    sheets: &[(&str, &[String], &[Vec<String>])],
    output_path: &Path,
) -> Result<()> {
    use rust_xlsxwriter::Workbook;

    let mut workbook = Workbook::new();

    for (sheet_name, headers, rows) in sheets {
        let worksheet = workbook.add_worksheet()
            .set_name(*sheet_name)
            .map_err(|e| anyhow::anyhow!("Failed to create sheet '{}': {}", sheet_name, e))?;

        // Write headers
        for (col_idx, header) in headers.iter().enumerate() {
            worksheet.write_string(0, col_idx as u16, header)
                .map_err(|e| anyhow::anyhow!("Write error: {}", e))?;
        }

        // Write data rows
        for (row_idx, row) in rows.iter().enumerate() {
            for (col_idx, value) in row.iter().enumerate() {
                if let Ok(num) = value.parse::<f64>() {
                    worksheet.write_number((row_idx + 1) as u32, col_idx as u16, num)
                        .map_err(|e| anyhow::anyhow!("Write error: {}", e))?;
                } else {
                    worksheet.write_string((row_idx + 1) as u32, col_idx as u16, value)
                        .map_err(|e| anyhow::anyhow!("Write error: {}", e))?;
                }
            }
        }
    }

    workbook.save(output_path)
        .map_err(|e| anyhow::anyhow!("Failed to save xlsx: {}", e))?;

    Ok(())
}
```

**Step 3: Verify compilation**

Run: `cargo check --features full 2>&1 | head -30`
Expected: Clean

**Step 4: Commit**

```bash
git add Cargo.toml src/engine/mod.rs
git commit -m "feat: add rust_xlsxwriter dependency and write_xlsx helper"
```

---

### Task 7: Add MCP Tool Handlers

**Files:**
- Modify: `src/mcp.rs`

**Step 1: Add parameter structs**

After `SqlQueryParams`, add:

```rust
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetMetadataParams {
    #[schemars(description = "File name (as shown in list_files). If omitted, returns metadata for all imported files.")]
    pub file_name: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSheetSampleParams {
    #[schemars(description = "File name (as shown in list_files)")]
    pub file_name: String,
    #[schemars(description = "Sheet name within the file")]
    pub sheet_name: String,
    #[schemars(description = "Number of rows to sample (default: 10)")]
    pub sample_size: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSheetDataParams {
    #[schemars(description = "File name (as shown in list_files)")]
    pub file_name: String,
    #[schemars(description = "Sheet name within the file")]
    pub sheet_name: String,
    #[schemars(description = "Start row index (0-based, inclusive). Default: 0")]
    pub start_row: Option<usize>,
    #[schemars(description = "End row index (exclusive). Default: all rows from start_row")]
    pub end_row: Option<usize>,
    #[schemars(description = "Column names to include. Default: all columns")]
    pub columns: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SaveAsParams {
    #[schemars(description = "Source file name (as shown in list_files)")]
    pub file_name: String,
    #[schemars(description = "Output file path for the new xlsx file")]
    pub output_path: String,
    #[schemars(description = "Specific sheet to export. If omitted, exports all sheets.")]
    pub sheet_name: Option<String>,
}
```

**Step 2: Add MCP response types**

After `McpSqlResult`, add:

```rust
#[derive(Debug, Serialize)]
pub struct McpSheetMetadata {
    pub sheet_name: String,
    pub row_count: usize,
    pub columns: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct McpFileMetadata {
    pub file_name: String,
    pub sheet_count: usize,
    pub sheets: Vec<McpSheetMetadata>,
}

#[derive(Debug, Serialize)]
pub struct McpMetadataResponse {
    pub files: Vec<McpFileMetadata>,
}

#[derive(Debug, Serialize)]
pub struct McpSheetData {
    pub file_name: String,
    pub sheet_name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub total_rows: usize,
    pub truncated: bool,
}
```

Add From impls:

```rust
impl From<FileMetadataInfo> for McpFileMetadata {
    fn from(m: FileMetadataInfo) -> Self {
        McpFileMetadata {
            file_name: m.file_name,
            sheet_count: m.sheet_count,
            sheets: m.sheets.into_iter().map(|s| McpSheetMetadata {
                sheet_name: s.sheet_name,
                row_count: s.row_count,
                columns: s.columns,
            }).collect(),
        }
    }
}

impl From<SheetDataResult> for McpSheetData {
    fn from(r: SheetDataResult) -> Self {
        McpSheetData {
            file_name: r.file_name,
            sheet_name: r.sheet_name,
            columns: r.columns,
            rows: r.rows,
            row_count: r.row_count,
            total_rows: r.total_rows,
            truncated: r.truncated,
        }
    }
}
```

Note: Add the new type imports to the `use crate::types::...` line at the top:
```rust
use crate::types::{FileInfo, FileMetadataInfo, SheetDataResult, SearchResult, SearchStats};
```

**Step 3: Add tool handlers to `GrepExcelServer`**

Inside `#[tool_router(server_handler)] impl GrepExcelServer`, add 4 new methods:

```rust
#[tool(description = "Get detailed metadata for imported files, including sheet names and column names. If file_name is omitted, returns metadata for all imported files.")]
pub async fn get_metadata(
    &self,
    Parameters(params): Parameters<GetMetadataParams>,
) -> Result<String, String> {
    let db = Arc::clone(&self.db);
    tokio::task::spawn_blocking(move || {
        let guard = db.read();
        if let Some(file_name) = params.file_name {
            guard.0.get_metadata(&file_name)
                .map(|m| {
                    let mcp: McpFileMetadata = m.into();
                    serde_json::to_string_pretty(&McpMetadataResponse {
                        files: vec![mcp],
                    }).unwrap_or_else(|_| "Metadata retrieved".to_string())
                })
                .map_err(|e| format!("Failed to get metadata: {}", e))
        } else {
            let files = guard.0.list_files();
            let mut all_metadata = Vec::new();
            for file in files {
                match guard.0.get_metadata(&file.name) {
                    Ok(m) => all_metadata.push(m.into()),
                    Err(_) => continue,
                }
            }
            Ok(serde_json::to_string_pretty(&McpMetadataResponse {
                files: all_metadata,
            }).unwrap_or_else(|_| "Metadata retrieved".to_string()))
        }
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}

#[tool(description = "Get a sample of rows from a specific sheet. Uses deterministic evenly-spaced sampling.")]
pub async fn get_sheet_sample(
    &self,
    Parameters(params): Parameters<GetSheetSampleParams>,
) -> Result<String, String> {
    let sample_size = params.sample_size.unwrap_or(10);
    let file_name = params.file_name;
    let sheet_name = params.sheet_name;
    let db = Arc::clone(&self.db);
    tokio::task::spawn_blocking(move || {
        let guard = db.read();
        guard.0.get_sheet_sample(&file_name, &sheet_name, sample_size)
            .map(|r| {
                let mcp: McpSheetData = r.into();
                serde_json::to_string_pretty(&mcp)
                    .unwrap_or_else(|_| "Sample retrieved".to_string())
            })
            .map_err(|e| format!("Failed to get sample: {}", e))
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}

#[tool(description = "Get rows from a specific sheet with pagination and column filtering. Supports start_row/end_row for pagination and optional column selection.")]
pub async fn get_sheet_data(
    &self,
    Parameters(params): Parameters<GetSheetDataParams>,
) -> Result<String, String> {
    let columns_ref = params.columns.as_deref();
    let db = Arc::clone(&self.db);
    let file_name = params.file_name;
    let sheet_name = params.sheet_name;
    let start_row = params.start_row;
    let end_row = params.end_row;
    let columns = params.columns;
    tokio::task::spawn_blocking(move || {
        let guard = db.read();
        guard.0.get_sheet_data(&file_name, &sheet_name, start_row, end_row, columns.as_deref())
            .map(|r| {
                let mcp: McpSheetData = r.into();
                serde_json::to_string_pretty(&mcp)
                    .unwrap_or_else(|_| "Data retrieved".to_string())
            })
            .map_err(|e| format!("Failed to get sheet data: {}", e))
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}

#[tool(description = "Save imported data to a new Excel file (Save As). Does not modify the original file.")]
pub async fn save_as(
    &self,
    Parameters(params): Parameters<SaveAsParams>,
) -> Result<String, String> {
    let db = Arc::clone(&self.db);
    let file_name = params.file_name;
    let output_path = params.output_path;
    let sheet_name = params.sheet_name;
    tokio::task::spawn_blocking(move || {
        let guard = db.read();
        if let Some(ref sheet_name) = sheet_name {
            // Save a single sheet
            let data = guard.0.get_sheet_data(&file_name, sheet_name, None, None, None)
                .map_err(|e| format!("Failed to read sheet data: {}", e))?;
            use crate::engine::write_xlsx;
            let headers = &data.columns;
            let rows = &data.rows;
            write_xlsx(&[(sheet_name.as_str(), headers.as_slice(), rows.as_slice())], std::path::Path::new(&output_path))
                .map(|_| format!("Successfully saved sheet '{}' to '{}'", sheet_name, output_path))
                .map_err(|e| format!("Failed to save: {}", e))
        } else {
            // Save all sheets
            guard.0.save_as(&file_name, std::path::Path::new(&output_path))
                .map(|_| format!("Successfully saved '{}' to '{}'", file_name, output_path))
                .map_err(|e| format!("Failed to save: {}", e))
        }
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}
```

**Step 4: Verify compilation**

Run: `cargo check --features full 2>&1 | head -30`
Expected: Clean

**Step 5: Commit**

```bash
git add src/mcp.rs
git commit -m "feat: add 4 new MCP tools (get_metadata, get_sheet_sample, get_sheet_data, save_as)"
```

---

### Task 8: Update README

**Files:**
- Modify: `README.md`

**Step 1: Add new tools to MCP tools table**

Update the MCP tools table to include the 4 new tools:

```markdown
| Tool | Description |
|------|-------------|
| `import_file` | Import an Excel/CSV file |
| `list_files` | List imported files and their sheets |
| `get_metadata` | Get detailed metadata: sheet names, column names per sheet |
| `get_sheet_sample` | Get sampled rows from a specific sheet |
| `get_sheet_data` | Get paginated row data with column filtering |
| `search` | Search with fulltext/exact/wildcard/regex |
| `execute_sql` | Execute a raw SQL `SELECT` query |
| `save_as` | Save imported data to a new Excel file |
```

**Step 2: Commit**

```bash
git add README.md
git commit -m "docs: update README with new MCP tools"
```

---

### Task 9: Build and Verify

**Step 1: Full build check**

Run: `cargo check --features full 2>&1`
Expected: Clean compilation

**Step 2: Default build check (engine-memory)**

Run: `cargo check 2>&1`
Expected: Clean compilation

**Step 3: Run existing tests**

Run: `cargo test 2>&1`
Expected: All existing tests pass (tests use `database` module which may need updating — check if tests reference old paths)

Note: The existing integration tests reference `grep_excel::database::Database` which doesn't exist anymore — they reference `grep_excel::database` module. Check if this module exists or if tests are broken already. Do NOT fix pre-existing test failures unrelated to our changes.

**Step 4: Final commit if any fixes needed**
