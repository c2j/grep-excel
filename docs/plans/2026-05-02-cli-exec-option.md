# CLI `--exec` Option Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `--exec` CLI option that accepts JSON-formatted MCP tool commands, enabling all MCP tools to run from the command line in a single process invocation.

**Architecture:** Reuse existing `DefaultEngine` methods directly from CLI (Strategy A). The `--exec` flag accepts either a single JSON object `{"tool":"...","params":{...}}` or a JSON array of such objects. Files passed as positional args are auto-imported before exec runs. A dispatch function routes tool names to the appropriate engine method.

**Tech Stack:** Rust, clap (already in use), serde + serde_json (currently optional via mcp-server, will become non-optional)

---

### Task 1: Make serde/serde_json non-optional

The `--exec` dispatch needs JSON parsing regardless of whether mcp-server is enabled.

**Files:**
- Modify: `Cargo.toml`

**Step 1: Update Cargo.toml**

Change serde and serde_json from optional to always-on:

```toml
# BEFORE (optional, only for mcp-server):
serde = { version = "1", features = ["derive"], optional = true }
serde_json = { version = "1", optional = true }

# AFTER (always available):
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Keep serde in mcp-server feature list (it's a no-op if already present, but harmless):
```toml
mcp-server = ["dep:rmcp", "dep:tokio", "dep:schemars", "dep:rust_xlsxwriter"]
```

Note: remove `dep:serde` and `dep:serde_json` from the mcp-server feature since they're no longer optional deps.

**Step 2: Verify build**

Run: `cargo check`
Expected: Compiles without errors. The `types.rs` types can now derive Serialize/Deserialize.

---

### Task 2: Remove `#[cfg(feature = "mcp-server")]` from SearchEngine trait methods

All methods that `--exec` needs must be unconditionally available.

**Files:**
- Modify: `src/engine/mod.rs` — remove cfg gates from trait method signatures
- Modify: `src/engine/memory.rs` — remove cfg gates from implementations

**Step 1: Update trait in `src/engine/mod.rs`**

Remove all 9 `#[cfg(feature = "mcp-server")]` annotations from:
- `get_metadata`
- `get_sheet_sample`
- `get_sheet_data`
- `save_as`
- `update_cell`
- `update_cells`
- `insert_rows`
- `delete_rows`
- `add_column`
- `rename_column`

Also remove `#[cfg(feature = "mcp-server")]` from the `write_xlsx` function.

**Step 2: Update implementation in `src/engine/memory.rs`**

Remove all 9 `#[cfg(feature = "mcp-server")]` annotations from the `impl SearchEngine for MemEngine` block.

**Step 3: Verify build**

Run: `cargo check`
Expected: Compiles. The engine methods are now always available.

---

### Task 3: Create shared parameter types in `types.rs`

Move param structs from `mcp.rs` to `types.rs` so both CLI and MCP can use them.

**Files:**
- Modify: `src/types.rs` — add all param structs
- Modify: `src/mcp.rs` — remove param struct definitions, import from types

**Step 1: Add param structs to `src/types.rs`**

Add `use serde::Deserialize;` at top. Then add all param structs:

```rust
use serde::Deserialize;

// --- Exec/MCP parameter types ---

#[derive(Debug, Deserialize)]
pub struct ImportFileParams {
    pub file_path: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub query: String,
    pub column: Option<String>,
    pub sheet: Option<String>,
    pub mode: Option<String>,
    pub limit: Option<usize>,
    pub aggregate: Option<String>,
    pub invert: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SqlQueryParams {
    pub sql: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct GetMetadataParams {
    pub file_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GetSheetSampleParams {
    pub file_name: String,
    pub sheet_name: String,
    pub sample_size: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct GetSheetDataParams {
    pub file_name: String,
    pub sheet_name: String,
    pub start_row: Option<usize>,
    pub end_row: Option<usize>,
    pub columns: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct SaveAsParams {
    pub file_name: String,
    pub output_path: String,
    pub sheet_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SaveParams {
    pub file_name: String,
    pub sheet_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCellParams {
    pub file_name: String,
    pub sheet_name: String,
    pub row: usize,
    pub column: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCellsParams {
    pub file_name: String,
    pub sheet_name: String,
    pub updates: Vec<CellUpdate>,
}

#[derive(Debug, Deserialize)]
pub struct CellUpdate {
    pub row: usize,
    pub column: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct InsertRowsParams {
    pub file_name: String,
    pub sheet_name: String,
    pub start_row: usize,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteRowsParams {
    pub file_name: String,
    pub sheet_name: String,
    pub start_row: usize,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
pub struct AddColumnParams {
    pub file_name: String,
    pub sheet_name: String,
    pub column_name: String,
    pub default_value: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RenameColumnParams {
    pub file_name: String,
    pub sheet_name: String,
    pub old_name: String,
    pub new_name: String,
}
```

Do NOT add `schemars::JsonSchema` derives — those are MCP-specific. They will be handled via a newtype wrapper in mcp.rs if needed (see Task 4).

**Step 2: Update `src/mcp.rs`**

Remove all param struct definitions (lines 18-178). Replace with imports from types:

```rust
use crate::types::{
    ImportFileParams, SearchParams, SqlQueryParams, GetMetadataParams,
    GetSheetSampleParams, GetSheetDataParams, SaveAsParams, SaveParams,
    UpdateCellParams, UpdateCellsParams, CellUpdate,
    InsertRowsParams, DeleteRowsParams, AddColumnParams, RenameColumnParams,
    // existing imports...
    FileInfo, FileMetadataInfo, SheetDataResult, SearchResult, SearchStats,
};
```

For the MCP tool params that need `schemars::JsonSchema`, create thin wrapper structs with both derives:

```rust
// In mcp.rs — MCP-specific wrappers with JsonSchema
macro_rules! mcp_params {
    ($name:ident, $inner:ty) => {
        #[derive(Debug, Deserialize, schemars::JsonSchema)]
        pub struct $name(pub $inner);
        impl std::ops::Deref for $name {
            type Target = $inner;
            fn deref(&self) -> &Self::Target { &self.0 }
        }
    };
}
```

**Alternative (simpler):** Just keep the schemars derive on the structs in types.rs behind a cfg:

```rust
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct SearchParams { ... }
```

This is the simplest approach. Go with this.

**Step 3: Update mcp.rs to import params from types**

Remove all param struct definitions. Add `use crate::types::*;` which already imports them.
Keep the MCP response types (McpSheetInfo, McpSearchResult, etc.) in mcp.rs since they're MCP-specific.
Remove `use rmcp::schemars;` import since schemars derives are now on the types themselves.

**Step 4: Verify build**

Run: `cargo check`
Expected: Compiles. mcp.rs uses shared param types from types.rs.

---

### Task 4: Add `--exec` CLI argument and dispatch logic

**Files:**
- Modify: `src/main.rs` — add `--exec` arg, create dispatch function, wire into main()

**Step 1: Add exec arg to Args struct**

```rust
#[arg(short = 'E', long, help = "Execute MCP tool command(s) as JSON. Single: '{\"tool\":\"...\",\"params\":{...}}' or array: '[{...},{...}]'")]
exec: Option<String>,
```

**Step 2: Create the ExecCommand struct and dispatch function**

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ExecCommand {
    tool: String,
    params: serde_json::Value,
}

fn run_exec(args: &Args) -> Result<()> {
    let exec_json = args.exec.as_ref().unwrap();
    let commands: Vec<ExecCommand> = if exec_json.trim_start().starts_with('[') {
        serde_json::from_str(exec_json)?
    } else {
        vec![serde_json::from_str(exec_json)?]
    };

    let mut db = DefaultEngine::new()?;
    let mut import_paths: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    // Auto-import positional files first
    for file in &args.files {
        if !file.exists() {
            eprintln!("File not found: {}", file.display());
            continue;
        }
        let canonical = std::fs::canonicalize(file)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| file.display().to_string());
        match db.import_excel(file, &|_, _| {}) {
            Ok(info) => {
                import_paths.insert(info.name.clone(), canonical);
                eprintln!("Imported: {} ({} sheets, {} rows)", info.name, info.sheets.len(), info.total_rows);
            }
            Err(e) => eprintln!("Failed to import '{}': {}", file.display(), e),
        }
    }

    // Execute commands sequentially
    for (i, cmd) in commands.iter().enumerate() {
        if commands.len() > 1 {
            eprintln!("\n--- Step {} ---", i + 1);
        }
        let result = exec_dispatch(&mut db, &mut import_paths, &cmd.tool, &cmd.params);
        match result {
            Ok(output) => println!("{}", output),
            Err(e) => {
                eprintln!("Error in step {} ({}): {}", i + 1, cmd.tool, e);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
```

**Step 3: Implement `exec_dispatch`**

This is the core routing function. It deserializes params into the correct struct and calls the engine method:

```rust
fn exec_dispatch(
    db: &mut dyn SearchEngine,  // Use &mut DefaultEngine directly
    import_paths: &mut std::collections::HashMap<String, String>,
    tool: &str,
    params: &serde_json::Value,
) -> Result<String> {
    match tool {
        "import_file" => {
            let p: ImportFileParams = serde_json::from_value(params.clone())?;
            let path = std::path::PathBuf::from(&p.file_path);
            let canonical = std::fs::canonicalize(&path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| p.file_path.clone());
            let info = db.import_excel(&path, &|_, _| {})?;
            import_paths.insert(info.name.clone(), canonical);
            Ok(serde_json::to_string_pretty(&info)?)
        }
        "search" => {
            let p: SearchParams = serde_json::from_value(params.clone())?;
            let mode = p.mode.as_deref().map(parse_search_mode).unwrap_or(SearchMode::FullText);
            let query = SearchQuery {
                text: p.query,
                column: p.column,
                mode,
                limit: p.limit.unwrap_or(100),
                sheet: p.sheet,
                invert: p.invert.unwrap_or(false),
            };
            let aggregate_col = p.aggregate;
            let (results, stats) = db.search(&query)?;
            // Build response (reuse McpSearchResponse or simpler struct)
            let response = build_search_response(results, stats, aggregate_col);
            Ok(serde_json::to_string_pretty(&response)?)
        }
        "execute_sql" => {
            let p: SqlQueryParams = serde_json::from_value(params.clone())?;
            let result = db.execute_sql(&p.sql, p.limit.unwrap_or(1000))?;
            Ok(serde_json::to_string_pretty(&serde_json::json!({
                "columns": result.columns,
                "rows": result.rows,
                "row_count": result.row_count,
                "truncated": result.truncated,
                "duration_ms": result.duration.as_millis(),
            }))?)
        }
        "list_files" => {
            let files: Vec<FileInfo> = db.list_files();
            Ok(serde_json::to_string_pretty(&serde_json::json!({ "files": files }))?)
        }
        "get_metadata" => {
            let p: GetMetadataParams = serde_json::from_value(params.clone())?;
            if let Some(file_name) = p.file_name {
                let m = db.get_metadata(&file_name)?;
                Ok(serde_json::to_string_pretty(&m)?)
            } else {
                let files = db.list_files();
                let mut all = Vec::new();
                for f in files {
                    if let Ok(m) = db.get_metadata(&f.name) {
                        all.push(m);
                    }
                }
                Ok(serde_json::to_string_pretty(&serde_json::json!({ "files": all }))?)
            }
        }
        "get_sheet_sample" => {
            let p: GetSheetSampleParams = serde_json::from_value(params.clone())?;
            let r = db.get_sheet_sample(&p.file_name, &p.sheet_name, p.sample_size.unwrap_or(10))?;
            Ok(serde_json::to_string_pretty(&r)?)
        }
        "get_sheet_data" => {
            let p: GetSheetDataParams = serde_json::from_value(params.clone())?;
            let r = db.get_sheet_data(&p.file_name, &p.sheet_name, p.start_row, p.end_row, p.columns.as_deref())?;
            Ok(serde_json::to_string_pretty(&r)?)
        }
        "update_cell" => {
            let p: UpdateCellParams = serde_json::from_value(params.clone())?;
            db.update_cell(&p.file_name, &p.sheet_name, p.row, &p.column, &p.value)?;
            Ok(format!("Updated cell at row {}, column '{}' to '{}'", p.row, p.column, p.value))
        }
        "update_cells" => {
            let p: UpdateCellsParams = serde_json::from_value(params.clone())?;
            let updates: Vec<(usize, String, String)> = p.updates.into_iter().map(|u| (u.row, u.column, u.value)).collect();
            let total = updates.len();
            let count = db.update_cells(&p.file_name, &p.sheet_name, &updates)?;
            Ok(format!("Updated {}/{} cells", count, total))
        }
        "insert_rows" => {
            let p: InsertRowsParams = serde_json::from_value(params.clone())?;
            let count = p.rows.len();
            db.insert_rows(&p.file_name, &p.sheet_name, p.start_row, p.rows)?;
            Ok(format!("Inserted {} rows at position {}", count, p.start_row))
        }
        "delete_rows" => {
            let p: DeleteRowsParams = serde_json::from_value(params.clone())?;
            let actual = db.delete_rows(&p.file_name, &p.sheet_name, p.start_row, p.count)?;
            Ok(format!("Deleted {} rows starting at row {}", actual, p.start_row))
        }
        "add_column" => {
            let p: AddColumnParams = serde_json::from_value(params.clone())?;
            let default = p.default_value.unwrap_or_default();
            db.add_column(&p.file_name, &p.sheet_name, &p.column_name, &default)?;
            Ok(format!("Added column '{}' with default value '{}'", p.column_name, default))
        }
        "rename_column" => {
            let p: RenameColumnParams = serde_json::from_value(params.clone())?;
            db.rename_column(&p.file_name, &p.sheet_name, &p.old_name, &p.new_name)?;
            Ok(format!("Renamed column '{}' to '{}'", p.old_name, p.new_name))
        }
        "save_as" => {
            let p: SaveAsParams = serde_json::from_value(params.clone())?;
            if let Some(ref sheet_name) = p.sheet_name {
                let data = db.get_sheet_data(&p.file_name, sheet_name, None, None, None)?;
                use grep_excel::engine::write_xlsx;
                write_xlsx(
                    &[(sheet_name.as_str(), &data.columns, &data.rows)],
                    std::path::Path::new(&p.output_path),
                )?;
            } else {
                db.save_as(&p.file_name, std::path::Path::new(&p.output_path))?;
            }
            Ok(format!("Successfully saved to '{}'", p.output_path))
        }
        "save" => {
            let p: SaveParams = serde_json::from_value(params.clone())?;
            let original_path = import_paths.get(&p.file_name).cloned()
                .ok_or_else(|| anyhow::anyhow!("Original path for '{}' not found", p.file_name))?;
            if let Some(ref sheet_name) = p.sheet_name {
                let data = db.get_sheet_data(&p.file_name, sheet_name, None, None, None)?;
                use grep_excel::engine::write_xlsx;
                write_xlsx(
                    &[(sheet_name.as_str(), &data.columns, &data.rows)],
                    std::path::Path::new(&original_path),
                )?;
            } else {
                db.save_as(&p.file_name, std::path::Path::new(&original_path))?;
            }
            Ok(format!("Saved to '{}'", original_path))
        }
        _ => anyhow::bail!("Unknown tool: '{}'. Available tools: import_file, list_files, get_metadata, get_sheet_sample, get_sheet_data, search, execute_sql, save_as, save, update_cell, update_cells, insert_rows, delete_rows, add_column, rename_column", tool),
    }
}
```

**Step 4: Wire `--exec` into `main()`**

Add before the `--mcp` check:

```rust
if args.exec.is_some() {
    return run_exec(&args);
}
```

**Step 5: Verify build and test**

Run: `cargo build --release`
Test:
```bash
# Single command
./target/release/grep_excel test_data.xlsx --exec '{"tool":"list_files","params":{}}'
./target/release/grep_excel test_data.xlsx --exec '{"tool":"search","params":{"query":"张三"}}'
./target/release/grep_excel test_data.xlsx --exec '{"tool":"get_metadata","params":{}}'

# Multi-step
./target/release/grep_excel --exec '[
  {"tool":"import_file","params":{"file_path":"test_data.xlsx"}},
  {"tool":"search","params":{"query":"张三","mode":"exact"}},
  {"tool":"get_sheet_sample","params":{"file_name":"test_data.xlsx","sheet_name":"Sheet1","sample_size":3}}
]'
```

---

### Task 5: Verify MCP server still works

**Step 1: Build with mcp-server feature**

Run: `cargo build --release --features mcp-server`

**Step 2: Quick smoke test**

Run: `echo '{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"capabilities":{}}}' | ./target/release/grep_excel --mcp`

Expected: MCP server responds with initialization. The param type changes didn't break MCP.

---

### Task 6: Update help text and README

**Files:**
- Modify: `README.md`

**Step 1: Add --exec to CLI Options table**

```markdown
| `--exec` | `-E` | Execute MCP tool command(s) as JSON |
```

**Step 2: Add --exec examples section**

```markdown
### CLI Exec Examples

**Single command:**
```bash
grep_excel data.xlsx --exec '{"tool":"search","params":{"query":"张三","mode":"exact"}}'
```

**Multi-step pipeline:**
```bash
grep_excel --exec '[
  {"tool":"import_file","params":{"file_path":"data.xlsx"}},
  {"tool":"update_cell","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","row":0,"column":"Name","value":"fixed"}},
  {"tool":"save","params":{"file_name":"data.xlsx"}}
]'
```
```

---

## Implementation Order

Tasks 1-3 are prerequisite refactoring (must be sequential).
Task 4 is the core feature (depends on 1-3).
Task 5 is validation (depends on 4).
Task 6 is documentation (depends on 4).

Total estimated effort: ~30-40 minutes for a skilled Rust developer.
