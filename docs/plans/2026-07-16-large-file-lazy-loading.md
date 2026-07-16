# Large File Virtual/Lazy Loading Optimization Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce "open file" latency for large CSV/HTML/Text files from >60s to <2s by decoupling metadata discovery from data materialization, using DuckDB virtual views (CSV) and lazy materialization (Excel/HTML/Text).

**Architecture:** Extend `SearchEngine` trait with `register_virtual` method that creates DuckDB VIEWs (CSV) or empty table shells (other formats) instantly. Data materialization happens lazily on first write operation, eagerly in background for interactive modes (`-i`, TUI) via shared DuckDB connections. Mode-specific strategies: `-t`/`-x`/`-q`/`-E` use pure virtual (never materialize), `-i`/TUI eagerly materialize in background after instant open without blocking queries.

**Tech Stack:** Rust 1.70+, DuckDB 1.10501, calamine 0.26, csv crate

---

## Phase 1: Fix HTML/Text Metadata Memory Waste (P0)

**Impact:** `-t` for HTML/Text files currently materializes ALL row data just to get `.len()`. Fix by adding streaming metadata extraction.

### Task 1.1: Add `extract_table_metadata` to HTML module

**Files:**
- Modify: `crates/core/src/html_table.rs`

**Step 1: Add `TableMetadata` struct**

At top of file, add the struct alongside existing `HtmlTable`:

```rust
/// Lightweight table metadata — no row data materialized.
/// Used by `-t` mode for fast schema discovery on large HTML files.
pub struct TableMetadata {
    pub name: String,
    pub headers: Vec<String>,
    pub row_count: usize,
}
```

**Step 2: Add `extract_table_metadata` function**

Add a new public function that mirrors `extract_tables` but counts rows instead of storing values. Use the **existing inline selector pattern** from `extract_tables` (do NOT introduce new helper functions):

```rust
pub fn extract_table_metadata(html: &str) -> Result<Vec<TableMetadata>> {
    let document = Html::parse_document(html);
    let mut tables = Vec::new();

    for (table_idx, table_element) in document
        .select(&Selector::parse("table").unwrap())
        .enumerate()
    {
        let name = /* same name-resolution logic as extract_tables */;

        let mut row_iter = table_element
            .select(&Selector::parse("tr, thead tr, tbody tr, tfoot tr").unwrap());

        // Extract headers from first row
        let header_cells: Vec<String> = match row_iter.next() {
            Some(header_row) => header_row
                .select(&Selector::parse("th, td").unwrap())
                .map(|el| {
                    el.text()
                        .collect::<String>()
                        .trim()
                        .to_string()
                })
                .collect(),
            None => continue,
        };

        // Count remaining rows (data rows) without storing values
        let mut row_count = 0usize;
        for _row in row_iter {
            row_count += 1;
        }

        if !header_cells.is_empty() && row_count > 0 {
            tables.push(TableMetadata {
                name,
                headers: header_cells,
                row_count,
            });
        }
    }
    Ok(tables)
}
```

> **Important:** Copy the exact name-resolution logic from the existing `extract_tables` function (using `summary` attr and heading context). Do NOT introduce `table_selector()`, `extract_table_name()` or other new helper functions — use the same inline selectors and name logic as the existing code. This avoids duplication while keeping the code familiar.

**Step 3: Verify existing `extract_tables` behavior unchanged**

```bash
cargo test -p grep-excel-core -- html_table
```

**Step 4: Unit test for `extract_table_metadata`**

Add a test that verifies metadata is correct without materializing rows:

```rust
#[test]
fn test_extract_table_metadata_lightweight() {
    let html = "<table><tr><th>Name</th><th>Age</th></tr><tr><td>Alice</td><td>30</td></tr><tr><td>Bob</td><td>25</td></tr></table>";
    let tables = extract_table_metadata(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, vec!["Name", "Age"]);
    assert_eq!(tables[0].row_count, 2);
}
```

**Step 5: Commit**

```bash
git add crates/core/src/html_table.rs
git commit -m "feat(core): add extract_table_metadata for lightweight HTML metadata"
```

### Task 1.2: Add streaming metadata extraction for Text/Markdown

**Files:**
- Modify: `crates/core/src/text_table.rs`

**Step 1: Add `TextTableMetadata` struct and `extract_tables_metadata` function**

Add a metadata-only function that takes `(path: &Path, content: &str)` matching the existing `extract_tables` signature:

```rust
/// Lightweight metadata extraction — no row data materialized.
pub struct TextTableMetadata {
    pub name: String,
    pub headers: Vec<String>,
    pub row_count: usize,
}

/// Extract table metadata from text/markdown content.
/// Takes (path, content) — same signature as `extract_tables` for caller consistency.
pub fn extract_tables_metadata(path: &Path, content: &str) -> Result<Vec<TextTableMetadata>> {
    // Same section-detection / header-detection logic as extract_tables,
    // but for each detected row, increment a counter instead of pushing to a Vec.
    // This avoids storing full row data for large text/markdown files.
}
```

**Step 2: Verify existing text parsing unchanged**

```bash
cargo test -p grep-excel-core -- text_table
```

**Step 3: Unit test**

Add a test verifying row count accuracy without data materialization:

```rust
#[test]
fn test_extract_tables_metadata_lightweight() {
    let content = "# People\n\n| Name | Age |\n|------|-----|\n| Alice | 30 |\n| Bob | 25 |";
    let tables = extract_tables_metadata(Path::new("test.md"), content).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, vec!["Name", "Age"]);
    assert_eq!(tables[0].row_count, 2);
}
```

**Step 4: Commit**

```bash
git add crates/core/src/text_table.rs
git commit -m "feat(core): add extract_tables_metadata for lightweight text/md metadata"
```

### Task 1.3: Update `parse_file_metadata` to use lightweight paths

**Files:**
- Modify: `crates/core/src/excel.rs:572-587` (parse_html_metadata)
- Modify: `crates/core/src/excel.rs:589-599` (parse_text_metadata)

**Step 1: Replace `parse_html_metadata` implementation**

```rust
fn parse_html_metadata(path: &Path) -> Result<Vec<SheetMetadata>> {
    let content = read_file_auto_encoding(path)?;
    let tables = html_table::extract_table_metadata(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse HTML '{}': {}", path.display(), e))?;
    Ok(tables
        .into_iter()
        .map(|t| SheetMetadata {
            name: t.name,
            headers: t.headers,
            row_count: t.row_count,
        })
        .collect())
}
```

**Step 2: Replace `parse_text_metadata` implementation**

Call the new `extract_tables_metadata` with both path and content:

```rust
fn parse_text_metadata(path: &Path) -> Result<Vec<SheetMetadata>> {
    let content = read_file_auto_encoding(path)?;
    let tables = text_table::extract_tables_metadata(path, &content)
        .map_err(|e| anyhow::anyhow!("Failed to parse text file '{}': {}", path.display(), e))?;
    Ok(tables
        .into_iter()
        .map(|t| SheetMetadata {
            name: t.name,
            headers: t.headers,
            row_count: t.row_count,
        })
        .collect())
}
```

**Step 3: Verify with integration test**

```bash
cargo test -p grep-excel-core -- excel::parse_file_metadata
```

**Step 4: Commit**

```bash
git add crates/core/src/excel.rs
git commit -m "perf(core): use lightweight metadata extraction for HTML/text in -t mode"
```

---

## Phase 2: CSV Import Optimization + Virtual Registration (P1)

**Impact:** CSV import 20-25% faster + foundation for virtual table support.

### Task 2.1: Optimize `read_csv_auto` parameters with malformed-CSV fallback

**Files:**
- Modify: `crates/core/src/engine/duckdb.rs:910-914`

**Step 1: Update CSV import SQL with graceful fallback**

```rust
// Before (line 910-914):
let create_sql = format!(
    "CREATE TABLE {} AS SELECT * FROM read_csv_auto('{}', header=true, all_varchar=true, ignore_errors=true)",
    quote_ident(&table_name),
    path_str.replace('\'', "''")
);

// After — try without ignore_errors first, fall back if needed:
let base_args = format!("header=true, all_varchar=true, sample_size=-1");
let create_sql_try = format!(
    "CREATE TABLE {} AS SELECT * FROM read_csv_auto('{}', {})",
    quote_ident(&table_name), path_str.replace('\'', "''"), base_args
);
match self.conn.execute(&create_sql_try, []) {
    Ok(_) => {} // success without ignore_errors
    Err(_) => {
        // Fall back to ignore_errors for malformed CSVs
        let create_sql_fallback = format!(
            "CREATE TABLE {} AS SELECT * FROM read_csv_auto('{}', {}, ignore_errors=true)",
            quote_ident(&table_name), path_str.replace('\'', "''"), base_args
        );
        self.conn.execute(&create_sql_fallback, [])?;
    }
}
```

Changes:
- Remove `ignore_errors=true` by default (saves per-field try/catch overhead for well-formed CSV)
- Add `sample_size=-1` (scan entire file once for schema, avoid two-pass read)
- Fall back to `ignore_errors=true` if first attempt fails (handles malformed CSV gracefully)

**Step 2: Verify**

```bash
cargo build -p grep-excel-core --features engine-duckdb
cargo test -p grep-excel-core -- engine
```

**Step 3: Commit**

```bash
git add crates/core/src/engine/duckdb.rs
git commit -m "perf(duckdb): optimize read_csv_auto params (sample_size=-1, opt-in ignore_errors)"
```

### Task 2.2a: Add `SheetState` enum and trait methods to `SearchEngine`

**Files:**
- Modify: `crates/core/src/engine/mod.rs`

**Step 1: Add `SheetState` enum**

```rust
/// Materialization state of a sheet in the engine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SheetState {
    Virtual,       // VIEW on source file, no data materialized
    Materializing, // Background import in progress
    Materialized,  // TABLE with all data loaded
}
```

**Step 2: Add trait methods**

```rust
pub trait SearchEngine: Send {
    // ... existing methods ...

    /// Register a file as a virtual source (metadata only).
    /// For CSV: creates DuckDB VIEW on read_csv_auto.
    /// For Excel/HTML/Text: creates empty table + stores source_path for later materialization.
    fn register_virtual(
        &mut self,
        path: &Path,
        progress: &dyn Fn(usize, usize),
    ) -> Result<FileInfo>;

    /// Materialize a previously registered virtual file into a table.
    fn materialize(
        &mut self,
        file_name: &str,
        progress: &dyn Fn(usize, usize),
    ) -> Result<()>;

    /// Check materialization state of a sheet.
    fn sheet_state(&self, file_name: &str, sheet_name: &str) -> Option<SheetState>;
}
```

**Step 3: Add stubs to memory and sqlite engines**

In `crates/core/src/engine/memory.rs`:

```rust
fn register_virtual(&mut self, path: &Path, progress: &dyn Fn(usize, usize)) -> Result<FileInfo> {
    self.import_excel(path, progress) // fall back to full import
}
fn materialize(&mut self, _file_name: &str, _progress: &dyn Fn(usize, usize)) -> Result<()> {
    Ok(()) // already materialized by register_virtual fallback
}
fn sheet_state(&self, _file_name: &str, _sheet_name: &str) -> Option<SheetState> {
    Some(SheetState::Materialized)
}
```

Same stubs in `crates/core/src/engine/sqlite.rs`.

**Step 4: Commit**

```bash
git add crates/core/src/engine/mod.rs crates/core/src/engine/memory.rs crates/core/src/engine/sqlite.rs
git commit -m "feat(core): add SheetState enum and register_virtual/materialize trait methods"
```

### Task 2.2b: Add `state` and `source_path` columns to DuckDB schema + pragma tuning

**Files:**
- Modify: `crates/core/src/engine/duckdb.rs:28-49`

**Step 1: Update `DuckDbEngine::new()` schema**

```rust
fn new() -> Result<Self> {
    let conn = Connection::open_in_memory()?;

    conn.execute_batch(
        "CREATE SEQUENCE IF NOT EXISTS file_id_seq START 1;
        CREATE SEQUENCE IF NOT EXISTS sheet_id_seq START 1;
        CREATE TABLE IF NOT EXISTS files (
            file_id INTEGER DEFAULT nextval('file_id_seq') PRIMARY KEY,
            file_name TEXT NOT NULL,
            imported_at TIMESTAMP DEFAULT current_timestamp
        );
        CREATE TABLE IF NOT EXISTS sheets (
            sheet_id INTEGER DEFAULT nextval('sheet_id_seq') PRIMARY KEY,
            file_id INTEGER NOT NULL REFERENCES files(file_id),
            sheet_name TEXT NOT NULL,
            table_name TEXT NOT NULL,
            row_count INTEGER DEFAULT 0,
            col_names TEXT DEFAULT '',
            col_widths TEXT DEFAULT '',
            state TEXT DEFAULT 'materialized',
            source_path TEXT DEFAULT ''
        );
        SET preserve_insertion_order = false;
        SET enable_progress_bar = false;",
    )?;

    Ok(DuckDbEngine { conn })
}
```

New columns:
- `state`: `'virtual'` | `'materializing'` | `'materialized'` — tracks materialization progress
- `source_path`: stores original file path for lazy materialization of non-CSV files

Since DuckDB is in-memory (created fresh each run), no migration needed.

**Step 2: Commit**

```bash
git add crates/core/src/engine/duckdb.rs
git commit -m "feat(duckdb): add state/source_path columns + pragma tuning"
```

### Task 2.2c: Add `get_view_columns` helper

**Files:**
- Modify: `crates/core/src/engine/duckdb.rs`

**Step 1: Extract existing column-query pattern into a method**

The existing code at duckdb.rs:923-931 queries `information_schema.columns` for column names. Extract this into a reusable `get_view_columns` method on `DuckDbEngine`:

```rust
impl DuckDbEngine {
    /// Get column names for a table or view via information_schema.
    fn get_view_columns(&self, table_name: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT column_name FROM information_schema.columns WHERE table_name = ? ORDER BY ordinal_position"
        )?;
        let mapped = stmt.query_map(params![table_name], |row: &::duckdb::Row| {
            row.get::<_, String>(0)
        })?;
        mapped.collect::<Result<Vec<_>, _>>()
    }
}
```

**Step 2: Replace inline column queries with new method**

In `import_csv_direct` (line ~923-931), replace the inline `information_schema.columns` query with `self.get_view_columns(&table_name)?`. This deduplicates and makes the column lookup available to `register_virtual` and `materialize`.

**Step 3: Commit**

```bash
git add crates/core/src/engine/duckdb.rs
git commit -m "refactor(duckdb): extract get_view_columns helper from inline column query"
```

### Task 2.2d: Implement `register_virtual` with `register_csv_virtual`

**Files:**
- Modify: `crates/core/src/engine/duckdb.rs`

**Step 1: Implement `register_virtual` trait method (dispatch)**

```rust
fn register_virtual(
    &mut self,
    path: &Path,
    progress_callback: &dyn Fn(usize, usize),
) -> Result<FileInfo> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "csv" {
        self.register_csv_virtual(path, progress_callback)
    } else {
        // Excel, HTML, Text, etc.: create empty table shell for lazy materialization
        self.register_lazy_virtual(path, progress_callback)
    }
}
```

**Step 2: Implement `register_csv_virtual`**

```rust
impl DuckDbEngine {
    fn register_csv_virtual(
        &mut self,
        path: &Path,
        progress_callback: &dyn Fn(usize, usize),
    ) -> Result<FileInfo> {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
        let sheet_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("csv").to_string();
        let path_str = path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid path encoding"))?;
        let escaped_path = path_str.replace('\'', "''");

        // Insert file record
        self.conn.execute("INSERT INTO files (file_name) VALUES (?)", params![&file_name])?;
        let file_id: i64 = self.conn.query_row(
            "SELECT currval('file_id_seq')", [], |row| row.get::<_, i64>(0))?;

        let table_name = format!("sheet_{}_0", file_id);

        // Create VIEW (not TABLE) — no data materialized
        let create_view = format!(
            "CREATE VIEW {} AS SELECT * FROM read_csv_auto('{}', header=true, all_varchar=true, sample_size=-1)",
            quote_ident(&table_name), escaped_path
        );
        self.conn.execute(&create_view, [])?;

        // Get row count via parallel DuckDB scan (5-10s for 11GB, vs 20-25s csv crate)
        let row_count: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM {}", quote_ident(&table_name)), [],
            |row| row.get::<_, i64>(0))?;
        progress_callback(row_count as usize, row_count as usize);

        // Get column names
        let col_names = self.get_view_columns(&table_name)?;

        // Store metadata (state=virtual)
        let col_names_str = col_names.join("\x1f");
        self.conn.execute(
            "INSERT INTO sheets (file_id, sheet_name, table_name, row_count, col_names, col_widths, state) VALUES (?, ?, ?, ?, ?, ?, 'virtual')",
            params![file_id, &sheet_name, &table_name, row_count as i32, &col_names_str, ""],
        )?;

        // Create friendly aliases
        let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
        let safe_schema = sanitize_schema_name(&file_stem);
        self.conn.execute(&format!("CREATE SCHEMA IF NOT EXISTS {}", quote_ident(&safe_schema)), [])?;
        self.conn.execute(&format!(
            "CREATE VIEW IF NOT EXISTS {}.{} AS SELECT * FROM {}",
            quote_ident(&safe_schema), quote_ident(&sheet_name), quote_ident(&table_name)
        ), [])?;
        let dotted_alias = format!("{}.{}", file_stem, sheet_name);
        self.conn.execute(&format!(
            "CREATE VIEW IF NOT EXISTS {} AS SELECT * FROM {}",
            quote_ident(&dotted_alias), quote_ident(&table_name)
        ), [])?;

        let sample_rows = self.get_sample_rows(&table_name, &col_names, 3)?;

        Ok(FileInfo {
            name: file_name,
            sheets: vec![(sheet_name.clone(), row_count as usize)],
            total_rows: row_count as usize,
            sample: if sample_rows.is_empty() { None } else {
                Some(FileSample { sheet_name, headers: col_names, rows: sample_rows })
            },
        })
    }
}
```

**Step 3: Verify build**

```bash
cargo build -p grep-excel-core --features engine-duckdb
```

**Step 4: Commit**

```bash
git add crates/core/src/engine/duckdb.rs
git commit -m "feat(duckdb): implement register_csv_virtual with DuckDB VIEW on read_csv_auto"
```

### Task 2.2e: Implement `register_lazy_virtual` for non-CSV formats

**Files:**
- Modify: `crates/core/src/engine/duckdb.rs`

**Step 1: Implement `register_lazy_virtual`**

For Excel/HTML/Text files, use `parse_file_metadata` (from Phase 1) to get column names and row counts, then create an empty DuckDB table with `CREATE TABLE ... (col1 TEXT, col2 TEXT, ...)`. Store the source file path for later `materialize`:

```rust
impl DuckDbEngine {
    fn register_lazy_virtual(
        &mut self,
        path: &Path,
        progress_callback: &dyn Fn(usize, usize),
    ) -> Result<FileInfo> {
        use crate::excel::parse_file_metadata;

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
        let path_str = path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid path encoding"))?;

        // Use lightweight metadata extraction (Phase 1 — no data materialized)
        let sheets_meta = parse_file_metadata(path)?;
        if sheets_meta.is_empty() {
            anyhow::bail!("No data found in file: {}", file_name);
        }

        // Insert file record
        self.conn.execute("INSERT INTO files (file_name) VALUES (?)", params![&file_name])?;
        let file_id: i64 = self.conn.query_row(
            "SELECT currval('file_id_seq')", [], |row| row.get::<_, i64>(0))?;

        let mut total_rows: usize = 0;
        let mut sheet_info: Vec<(String, usize)> = Vec::new();
        let mut sample: Option<FileSample> = None;

        for (sheet_idx, meta) in sheets_meta.iter().enumerate() {
            let table_name = format!("sheet_{}_{}", file_id, sheet_idx);
            let col_names = sanitize_col_names(&meta.headers);

            // Create empty table shell with TEXT columns
            let col_defs: Vec<String> = col_names
                .iter()
                .map(|c| format!("{} TEXT", quote_ident(c)))
                .collect();
            self.conn.execute(&format!(
                "CREATE TABLE {} ({})",
                quote_ident(&table_name), col_defs.join(", ")
            ), [])?;

            // Store metadata with state=virtual and source_path
            let col_names_str = col_names.join("\x1f");
            self.conn.execute(
                "INSERT INTO sheets (file_id, sheet_name, table_name, row_count, col_names, col_widths, state, source_path) \
                 VALUES (?, ?, ?, ?, ?, '', 'virtual', ?)",
                params![file_id, &meta.name, &table_name, meta.row_count as i32, &col_names_str, path_str],
            )?;

            // Create friendly aliases
            let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
            let safe_schema = sanitize_schema_name(&file_stem);
            self.conn.execute(&format!("CREATE SCHEMA IF NOT EXISTS {}", quote_ident(&safe_schema)), [])?;
            self.conn.execute(&format!(
                "CREATE VIEW IF NOT EXISTS {}.{} AS SELECT * FROM {}",
                quote_ident(&safe_schema), quote_ident(&meta.name), quote_ident(&table_name)
            ), [])?;
            let dotted_alias = format!("{}.{}", file_stem, meta.name);
            self.conn.execute(&format!(
                "CREATE VIEW IF NOT EXISTS {} AS SELECT * FROM {}",
                quote_ident(&dotted_alias), quote_ident(&table_name)
            ), [])?;

            total_rows += meta.row_count;
            sheet_info.push((meta.name.clone(), meta.row_count));

            if sample.is_none() {
                sample = Some(FileSample {
                    sheet_name: meta.name.clone(),
                    headers: meta.headers.clone(),
                    rows: Vec::new(), // empty — no data yet
                });
            }
        }

        progress_callback(total_rows, total_rows);

        Ok(FileInfo {
            name: file_name,
            sheets: sheet_info,
            total_rows,
            sample,
        })
    }
}
```

Key design decisions:
- Table shell is created with `CREATE TABLE (col TEXT, ...)` — correct column names ready for queries, but zero rows
- `source_path` stores the file path so `materialize` knows where to read from
- No data is loaded — just metadata registration (<1s for Excel, streaming for HTML/Text)
- The empty table will appear to have `row_count` > 0 in metadata, but queries return no rows until `materialize` is called

**Step 2: Update `materialize` to handle non-CSV via source_path**

```rust
fn materialize(
    &mut self,
    file_name: &str,
    progress: &dyn Fn(usize, usize),
) -> Result<()> {
    // Get all virtual sheets for this file with their source_path
    let sheets: Vec<(String, String, Option<String>)> = {
        let mut stmt = self.conn.prepare(
            "SELECT s.sheet_name, s.table_name, s.source_path FROM sheets s
             JOIN files f ON s.file_id = f.file_id
             WHERE f.file_name = ? AND s.state = 'virtual'"
        )?;
        stmt.query_map(params![file_name], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?))
        })?.collect::<Result<Vec<_>,_>>()?
    };

    for (sheet_name, table_name, source_path) in &sheets {
        // Mark as materializing
        self.conn.execute(
            "UPDATE sheets SET state = 'materializing' WHERE table_name = ?",
            params![table_name]
        )?;

        if let Some(src_path) = source_path {
            // Non-CSV: full import from source file into the empty table shell
            let path = Path::new(src_path);
            match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
                "csv" => {
                    // CSV was already registered as VIEW; materialize = CREATE TABLE AS SELECT * FROM VIEW
                    // This case shouldn't reach here (CSV uses register_csv_virtual which has no source_path)
                    // But handle gracefully:
                    let temp_name = format!("{}_mat", table_name);
                    self.conn.execute(&format!(
                        "CREATE TABLE {} AS SELECT * FROM {}", quote_ident(&temp_name), quote_ident(table_name)
                    ), [])?;
                    self.conn.execute(&format!("DROP VIEW IF EXISTS {}", quote_ident(table_name)), [])?;
                    self.conn.execute(&format!(
                        "ALTER TABLE {} RENAME TO {}", quote_ident(&temp_name), quote_ident(table_name)
                    ), [])?;
                }
                _ => {
                    // Excel/HTML/Text: use existing import_excel flow, targeting the existing table shell
                    // Drop the empty shell, do full import via existing import_excel_sheets
                    self.conn.execute(&format!("DROP TABLE IF EXISTS {}", quote_ident(table_name)), [])?;
                    // Note: import_excel_sheets creates the table with correct name; we need to
                    // override the file_id/sheet_idx to match. Alternatively, materialize into temp
                    // and swap. For simplicity: drop + re-import via import_excel API.
                    // The file/sheet records already exist; we update row_count after import.
                    // See detailed implementation note in Step 3.
                }
            }
        } else {
            // CSV VIEW: CREATE TABLE AS SELECT * FROM VIEW (only path with no source_path)
            let temp_name = format!("{}_mat", table_name);
            self.conn.execute(&format!(
                "CREATE TABLE {} AS SELECT * FROM {}", quote_ident(&temp_name), quote_ident(table_name)
            ), [])?;
            // Atomically swap: wrap in transaction
            let tx = self.conn.transaction()?;
            tx.execute(&format!("DROP VIEW IF EXISTS {}", quote_ident(table_name)), [])?;
            tx.execute(&format!(
                "ALTER TABLE {} RENAME TO {}", quote_ident(&temp_name), quote_ident(table_name)
            ), [])?;
            tx.commit()?;
        }

        // Create indexes (same as import_excel_sheets)
        let col_names = self.get_view_columns(table_name)?;
        for col_name in &col_names {
            let safe_name = col_name.replace(|c: char| !c.is_alphanumeric() && c != '_', "_");
            let index_name = format!("idx_{}_{}", table_name, safe_name);
            let _ = self.conn.execute(&format!(
                "CREATE INDEX IF NOT EXISTS \"{}\" ON {} ({})",
                index_name, quote_ident(table_name), quote_ident(col_name)
            ), []);
        }

        // Mark as materialized
        self.conn.execute(
            "UPDATE sheets SET state = 'materialized' WHERE table_name = ?",
            params![table_name]
        )?;
    }
    Ok(())
}
```

**Step 3: Non-CSV materialize implementation note**

For Excel/HTML/Text lazy materialization, the cleanest approach is to mark the existing table shell for re-import. The actual implementation should:
1. Drop the empty table shell
2. Call the existing `import_excel_sheets` (or `import_csv_direct` dispatching) but with the SAME `file_id` and `sheet_idx` so the table name matches
3. This requires refactoring `import_excel_sheets` to accept an optional `file_id` parameter rather than always inserting a new file record

Alternatively (simpler): create a temporary table via existing import, then `INSERT INTO shell SELECT * FROM temp`, drop temp.

The developer should evaluate which approach is cleaner during implementation. The key contract is: after `materialize`, the sheet is a fully populated TABLE with indexes.

**Step 4: Verify**

```bash
cargo build --features full
cargo test -p grep-excel-core -- engine
```

**Step 5: Commit**

```bash
git add crates/core/src/engine/duckdb.rs
git commit -m "feat(duckdb): implement register_lazy_virtual + materialize with transaction safety"
```

---

## Phase 3: Mode-Specific Loading Strategy (P2)

**Impact:** `-i` and TUI open instantly with non-blocking background materialization via shared DuckDB connections. `-x`/`-q`/`-t`/`-E` use virtual tables exclusively.

**Threading model:** For `-i` and TUI modes where background materialization is needed, use a **file-backed DuckDB database** instead of in-memory:

```rust
// Instead of Connection::open_in_memory():
let db_path = std::env::temp_dir().join(format!("grep_excel_{}.duckdb", std::process::id()));
let conn = Connection::open(&db_path)?;
```

This allows two connections to share the same database concurrently:
- **Connection A** (REPL/TUI main thread): handles user queries
- **Connection B** (background materialize thread): runs `materialize` without blocking Connection A

DuckDB's MVCC handles concurrent read/write transparently. The temp file is cleaned up on exit.

> **If file-backed DB is not feasible** (e.g., slower than in-memory), fall back to **in-memory + chunked materialization**: the background thread acquires the lock, materializes one sheet, releases the lock, repeats. Between chunks, the main thread can service queries.

### Task 3.1: CLI `-t`: DuckDB parallel COUNT for CSV + metadata cache

**Files:**
- Modify: `crates/cli/src/main.rs:680-921` (run_list_tables_cli)

**Step 1: For CSV files, use `register_virtual` for faster parallel row counting**

```rust
// In the file loop within run_list_tables_cli:
if ext == "csv" {
    // DuckDB parallel COUNT via register_virtual: 5-10s vs 20-25s csv crate scan
    let mut engine = DefaultEngine::new()?;
    match engine.register_virtual(&file, &|_, _| {}) {
        Ok(info) => {
            for alias in engine.list_table_aliases() {
                tables.push(TableInfo {
                    alias: alias.alias,
                    table_name: alias.table_name,
                    row_count: alias.row_count,
                    columns: alias.columns,
                });
            }
        }
        Err(e) => {
            // Fall back to parse_file_metadata for malformed CSVs
            match parse_file_metadata(&file) {
                Ok(sheets) => { /* existing logic */ }
                Err(_) => eprintln!("Failed to read '{}': {}", file.display(), e),
            }
        }
    }
} else {
    // Use parse_file_metadata for non-CSV (already fast: Excel <0.5s, HTML/Text streaming)
    // ... existing code ...
}
```

**Step 2: Identify metadata cache opportunity**

The DuckDB parallel COUNT (5-10s) is better than csv crate (20-25s) but still requires a full file scan. Future optimization: persist `{size, mtime, row_count, headers_hash}` to a local cache file (e.g., `~/.local/share/grep-excel/metadata_cache.json`). On subsequent `-t` calls, if `(size, mtime)` matches, return cached row count instantly. First run pays the 5-10s scan cost. Note this as a TODO comment in the code — not implementing in this plan.

**Step 3: Verify**

```bash
cargo build --features full
cargo run --features full -- test_data.csv -t
```

**Step 4: Commit**

```bash
git add crates/cli/src/main.rs
git commit -m "feat(cli): use DuckDB parallel COUNT for CSV in -t mode (5-10s vs 20-25s)"
```

### Task 3.2: CLI `-x` / `-q` / `-E`: use virtual registration for CSV

**Files:**
- Modify: `crates/cli/src/main.rs` (import_file_with_repair call sites for `-x`, `-q`, `-E` paths)

**Step 1: For CSV files in `-x`, `-q`, `-E` mode, use `register_virtual` instead of full import**

In each mode's entry point where `import_file_with_repair` is called, detect CSV and use `register_virtual`:

```rust
// Generic helper for one-shot modes (x, q, E):
fn quick_register(db: &mut DefaultEngine, path: &Path, repair: bool) -> Result<FileInfo> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "csv" && !repair {
        // Virtual registration — no full import needed for one-shot queries
        db.register_virtual(path, &|_, _| {})
    } else {
        // Full import for non-CSV or repair mode
        import_file_with_repair(db, path, repair)
    }
}
```

Apply this to the `-x` path (line ~1000+), `-q` paths (line ~330+), and `-E` path (line ~1340+).

**Step 2: Verify**

```bash
cargo run --features full -- large.csv -x "SELECT * FROM sheet_1_0 LIMIT 10"
# Expected: <3s (was >60s)
cargo run --features full -- large.csv -q "keyword"
# Expected: 10-20s (was >60s import + search)
```

**Step 3: Commit**

```bash
git add crates/cli/src/main.rs
git commit -m "feat(cli): use register_virtual for CSV in -x/-q/-E one-shot modes"
```

### Task 3.3: CLI `-i`: non-blocking background materialization via shared DB

**Files:**
- Modify: `crates/cli/src/main.rs:923-988` (run_interactive_cli)
- Modify: `crates/cli/src/interactive.rs` (add materialization status display)

**Step 1: Change `run_interactive_cli` to use file-backed DuckDB + background materialize**

```rust
fn run_interactive_cli(args: &Args) -> Result<()> {
    // Use file-backed DB for shared concurrent access
    let db_path = std::env::temp_dir()
        .join(format!("grep_excel_rep_{}.duckdb", std::process::id()));
    let db = Arc::new(Mutex::new(DefaultEngine::with_path(&db_path)?));
    // ... share-url setup ...

    let mut registered_files: Vec<String> = Vec::new();

    // Phase 1: Quick registration (main thread)
    for input in &args.files {
        // ... resolve path ...
        let mut db_guard = db.lock();
        match db_guard.0.register_virtual(&path, &|_, _| {}) {
            Ok(info) => {
                eprintln!("Registered: {} ({} rows)", info.name, info.total_rows);
                registered_files.push(info.name.clone());
            }
            Err(e) => eprintln!("Failed: {}", e),
        }
        drop(db_guard);
    }

    // Phase 2: Spawn background materialization thread with own connection
    if !registered_files.is_empty() {
        let (mat_tx, mat_rx) = std::sync::mpsc::channel::<MaterializeProgress>();
        let mat_db_path = db_path.clone();
        let files = registered_files.clone();

        std::thread::spawn(move || {
            // Open a SECOND connection to the SAME file-backed DB
            let mut mat_engine = match DefaultEngine::with_path(&mat_db_path) {
                Ok(e) => e,
                Err(e) => { let _ = mat_tx.send(MaterializeProgress::Error("".into(), e.to_string())); return; }
            };
            for file_name in &files {
                let start = Instant::now();
                match mat_engine.materialize(file_name, &|current, total| {
                    let _ = mat_tx.send(MaterializeProgress::Progress(file_name.clone(), current, total));
                }) {
                    Ok(()) => {
                        let _ = mat_tx.send(MaterializeProgress::Done(file_name.clone(), start.elapsed()));
                    }
                    Err(e) => {
                        let _ = mat_tx.send(MaterializeProgress::Error(file_name.clone(), e.to_string()));
                    }
                }
            }
        });

        // Phase 3: Enter REPL with materialization events
        interactive::run_with_progress(&db, mat_rx, args.no_history)?;
    } else {
        // No files to materialize; enter REPL directly
        let db_guard = db.lock();
        interactive::run_with_progress(&db, /* empty rx */ std::sync::mpsc::channel().1, args.no_history)?;
        drop(db_guard);
    }

    // Cleanup temp DB
    let _ = std::fs::remove_file(&db_path);
    Ok(())
}
```

**Step 2: Add `MaterializeProgress` enum**

In `interactive.rs` or `main.rs`:

```rust
enum MaterializeProgress {
    Progress(String, usize, usize),  // (file_name, current, total)
    Done(String, Duration),          // (file_name, elapsed)
    Error(String, String),           // (file_name, error_message)
}
```

**Step 3: Add progress display to REPL**

In `interactive.rs`, modify the REPL loop to check `mat_rx.try_recv()` before each prompt and update a status line:

```
[Importing 11gb.csv ████████░░ 78%]
$ SELECT * FROM sheet_1_0 LIMIT 3   ← queries work immediately (own connection)
...
Import complete: 11gb.csv (48.3s)
$
```

**Step 4: Add `DefaultEngine::with_path` factory**

Since `DuckDbEngine::new()` hardcodes `Connection::open_in_memory()`, add a constructor that takes a path:

```rust
impl DuckDbEngine {
    pub fn with_path(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        // Same schema setup as new()
        conn.execute_batch(/* same schema DDL */)?;
        Ok(DuckDbEngine { conn })
    }
}
```

For memory/sqlite engines, `with_path` delegates to `new()`.

**Step 5: Commit**

```bash
git add crates/cli/src/main.rs crates/cli/src/interactive.rs \
        crates/core/src/engine/duckdb.rs crates/core/src/engine/mod.rs
git commit -m "feat(cli): non-blocking background materialization for -i via shared file-backed DB"
```

### Task 3.4: TUI: non-blocking background materialization

**Files:**
- Modify: `crates/cli/src/app/mod.rs:180-197` (import_file)
- Modify: `crates/cli/src/app/ui.rs` (add progress indicator)
- Modify: `crates/cli/src/event.rs` (add materialization events)

**Step 1: Switch TUI DuckDB to file-backed for shared access**

In the TUI initialization (where `DefaultEngine::new()` is called), use `DefaultEngine::with_path(&temp_path)?` if the DuckDB engine is active. This is the same pattern as Task 3.3.

**Step 2: Change `App::import_file` to register virtual + spawn background materialize**

```rust
pub fn import_file(&mut self, path: PathBuf) {
    self.loading = true;
    self.status_message = crate::i18n::status_importing(&path);
    let db = Arc::clone(&self.database);
    let tx = self.event_tx.clone();
    let path_clone = path.clone();
    let db_path = self.db_path.clone(); // stored on App

    std::thread::spawn(move || {
        // Phase 1: Virtual registration (hold lock briefly)
        let register_result = {
            let mut db_guard = db.lock();
            db_guard.0.register_virtual(&path_clone, &|_, _| {})
        };

        match register_result {
            Ok(info) => {
                let file_name = info.name.clone();
                let _ = tx.send(AppEvent::FileImported(Ok(info)));

                // Phase 2: Background materialization via separate connection
                if let Ok(mut mat_engine) = DefaultEngine::with_path(&db_path) {
                    let result = mat_engine.materialize(&file_name, &|current, total| {
                        let _ = tx.send(AppEvent::MaterializeProgress(file_name.clone(), current, total));
                    });
                    let _ = tx.send(AppEvent::MaterializeComplete(file_name, result));
                }
            }
            Err(e) => {
                let _ = tx.send(AppEvent::FileImported(Err(e)));
            }
        }
    });
}
```

> **Key difference from original plan:** The materialize runs on a **separate DuckDB connection** to the same file-backed DB. The main TUI thread's connection is NOT blocked. Queries during materialization work normally — DuckDB handles MVCC.

**Step 3: Add `AppEvent` variants**

In `crates/cli/src/event.rs`:

```rust
pub enum AppEvent {
    // ... existing ...
    MaterializeProgress(String, usize, usize),  // (file_name, current, total)
    MaterializeComplete(String, Result<()>),     // (file_name, result)
}
```

**Step 4: Display materialization progress in TUI status bar**

In `ui.rs` render method:

```rust
if !app.mat_progress.is_empty() {
    let (ref name, current, total) = app.mat_progress[0];
    let pct = if total > 0 { current * 100 / total } else { 0 };
    status_line = format!("Importing {} [{:>3}%]", name, pct);
}
```

**Step 5: Commit**

```bash
git add crates/cli/src/app/mod.rs crates/cli/src/app/ui.rs crates/cli/src/event.rs
git commit -m "feat(tui): non-blocking background materialization via shared file-backed DB"
```

### Task 3.5: MCP `import_file`: use virtual registration for CSV

**Files:**
- Modify: `crates/cli/src/mcp.rs` (import_file handler)

**Step 1: Detect CSV and use `register_virtual`**

In the `import_file` MCP tool handler, detect CSV files:

```rust
pub async fn import_file(
    db: &Arc<Mutex<DefaultEngine>>,
    params: ImportFileParams,
) -> Result<Value, String> {
    let path = PathBuf::from(&params.file_path);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    let result = if ext == "csv" {
        let mut db_guard = db.lock().map_err(|e| e.to_string())?;
        db_guard.0.register_virtual(&path, &|_, _| {}).map_err(|e| e.to_string())
    } else {
        let mut db_guard = db.lock().map_err(|e| e.to_string())?;
        db_guard.0.import_excel(&path, &|_, _| {}).map_err(|e| e.to_string())
    };

    // ... format result ...
}
```

> Note: MCP write operations (update_cell, save, etc.) will trigger lazy materialization if the sheet is virtual (Task 3.6). For read-only MCP usage (search, execute_sql, get_sheet_data), virtual VIEWs work transparently.

**Step 2: Commit**

```bash
git add crates/cli/src/mcp.rs
git commit -m "feat(mcp): use register_virtual for CSV in import_file"
```

### Task 3.6: Protect write operations with lazy materialization trigger

**Files:**
- Modify: `crates/core/src/engine/duckdb.rs` (update_cell, insert_rows, delete_rows, add_column, rename_column, save, save_as)

**Step 1: Add pre-check that triggers materialization if virtual**

Instead of blocking writes, automatically trigger materialization:

```rust
fn update_cell(&mut self, file_name: &str, sheet_name: &str, row: usize, column: &str, value: &str) -> Result<()> {
    let meta = self.get_sheet_metadata_query(file_name, sheet_name)?;

    let state: String = self.conn.query_row(
        "SELECT state FROM sheets WHERE table_name = ?",
        params![&meta.table_name], |row| row.get(0)
    ).unwrap_or_else(|_| "materialized".to_string());

    if state == "virtual" || state == "materializing" {
        if state == "virtual" {
            // Auto-materialize before write
            self.materialize(file_name, &|_, _| {})?;
        } else {
            anyhow::bail!(
                "File '{}' is currently being imported. Please wait for import to complete.",
                file_name
            );
        }
    }

    // ... existing update logic ...
}
```

Apply the same pattern to: `update_cells`, `insert_rows`, `delete_rows`, `add_column`, `rename_column`, `save`, `save_as`.

**Step 2: Commit**

```bash
git add crates/core/src/engine/duckdb.rs
git commit -m "feat(core): auto-materialize on first write to virtual sheet"
```

---

## Phase 4: Verification and Integration Testing

### Task 4.1: End-to-end test — `-t` mode

```bash
# Metadata for all formats — should be fast
grep_excel test_data.csv -t
# Expected: 5-10s (DuckDB parallel COUNT, down from 20-25s csv crate scan)

grep_excel test_data.xlsx -t
# Expected: <0.5s (already fast via calamine metadata)

grep_excel test_data.html -t
# Expected: <1s (streaming metadata, no more full row materialization)

grep_excel test_data.md -t
# Expected: <1s (streaming metadata)
```

### Task 4.2: End-to-end test — `-x` mode with virtual CSV

```bash
grep_excel large.csv -x "SELECT * FROM sheet_1_0 LIMIT 10"
# Expected: <3s total (register virtual + query), no 60s import

grep_excel large.csv -x "SELECT COUNT(*), col FROM sheet_1_0 GROUP BY col"
# Expected: 10-20s (full scan of 11GB CSV via VIEW)
```

### Task 4.3: End-to-end test — `-q` mode

```bash
grep_excel large.csv -q "rare_keyword"
# Expected: 10-20s (VIEW scan, down from >60s import + search)
```

### Task 4.4: End-to-end test — `-i` mode

```bash
grep_excel large.csv -i
# Expected: <3s to REPL prompt, with "[Importing...]" progress
# .tables — works immediately
# SELECT * FROM sheet_1_0 LIMIT 10 — works immediately (separate connection for materialize)
# After import: status line shows "Import complete (48s)"
# Subsequent queries are millisecond-fast
```

### Task 4.5: End-to-end test — TUI mode

```bash
grep_excel large.csv
# Expected: <3s to TUI with browse data, progress bar during background import
# Browse works immediately (LIMIT 500 via VIEW)
# Search works during import (slightly slower until materialized)
```

### Task 4.6: Regression — existing `import_excel` path still works

```bash
cargo test -p grep-excel-core -- engine::duckdb
cargo test -p grep-excel-core -- engine::memory
cargo test -p grep-excel-core -- engine::sqlite

# MCP server mode
grep_excel --mcp  # then test import_file, search, execute_sql
```

### Task 4.7: Commit

```bash
git add docs/plans/2026-07-16-large-file-lazy-loading.md
git commit -m "docs: add large file lazy loading optimization plan v2"
```

---

## Performance Summary

| Scenario | Before | After | Gain |
|----------|--------|-------|------|
| `grep_excel 11gb.csv -t` | ~25s (Rust csv sequential scan) | **5-10s** (DuckDB parallel COUNT) | **60-80%** |
| `grep_excel 11gb.csv -x "SELECT ... LIMIT 10"` | >60s (full import) | **<3s** (virtual + pushdown) | **95%+** |
| `grep_excel 11gb.csv -q "keyword"` | >60s (full import + search) | **10-20s** (virtual scan) | **65-80%** |
| `grep_excel 11gb.csv -i` (to prompt) | >60s | **<3s** | **95%+** |
| `grep_excel 11gb.csv -i` (queries during import) | N/A (blocked) | **works** (separate connection) | new |
| `grep_excel 11gb.csv -i` (queries after import) | <1s | <1s (same) | same |
| `grep_excel 11gb.csv` (TUI browse) | >60s | **<3s** (LIMIT 500 via VIEW) | **95%+** |
| `grep_excel big.html -t` | OOM risk (full materialize) | **<1s** (streaming count) | **99%+** |
| `grep_excel 11gb.csv` MCP import_file | >60s | **<3s** (virtual) | **95%+** |
| CSV full import speed (when needed) | ~60s | ~45s (20-25%) | 25% |

## Threading Model Summary

```
┌─ REPL / TUI Main Thread ─────────────────────┐
│  Connection A (file-backed DB)                │
│  ┌─────────────────────────────────────┐      │
│  │ register_virtual: CREATE VIEW ...   │      │
│  │ user queries: SELECT * FROM VIEW    │      │
│  │ browse: SELECT ... LIMIT 500        │      │
│  │ .tables, .files, .save             │      │
│  └─────────────────────────────────────┘      │
└───────────────────────────────────────────────┘
          ▲                    ▲
          │ shared file-backed │ DuckDB (MVCC)
          ▼                    ▼
┌─ Background Materialize Thread ───────────────┐
│  Connection B (same file-backed DB)           │
│  ┌─────────────────────────────────────┐      │
│  │ materialize:                        │      │
│  │   CREATE TABLE _mat AS SELECT *     │      │
│  │   FROM VIEW                         │      │
│  │   → DROP VIEW → RENAME TABLE        │      │
│  │   → CREATE INDEX                    │      │
│  │   → UPDATE state = 'materialized'   │      │
│  └─────────────────────────────────────┘      │
│  Queries on Connection A work normally        │
│  during entire materialize process            │
└───────────────────────────────────────────────┘
```
