[中文版](./DeveloperGuide.zh-CN.md)

# grep-excel Developer Guide

This document is intended for developers who need to extend grep-excel's functionality, integrate MCP tools, or understand the internal architecture.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [SearchEngine Trait](#searchengine-trait)
- [Adding a New Engine Backend](#adding-a-new-engine-backend)
- [MCP Tool Development](#mcp-tool-development)
- [Type System](#type-system)
- [Internationalization (i18n)](#internationalization-i18n)
- [CLI Command Extension](#cli-command-extension)
- [TUI Component Development](#tui-component-development)
- [Data Flow](#data-flow)
- [Release Process](#release-process)

---

## Architecture Overview

grep-excel uses a layered architecture with the `SearchEngine` trait at its core:

```
┌─────────────────────────────────────────────────────────────┐
│                       Entry Layer (main.rs)                 │
│  CLI Parse → Mode Routing (CLI / TUI / MCP / Exec / SQL /  │
│                           Run / REPL)                       │
└───────┬──────────────┬──────────────┬───────────────────────┘
        │              │              │
        ▼              ▼              ▼
┌───────────┐  ┌───────────┐  ┌──────────────┐
│  CLI Mode  │  │  TUI Mode  │  │  MCP Mode    │
│ run_cli()  │  │ app/mod.rs│  │ mcp.rs + rmcp│
│ run_sql()  │  │ ratatui   │  │              │
│ run_exec() │  │           │  │              │
│run_exec_shell()│         │  │              │
└─────┬─────┘  └─────┬─────┘  └──────┬────────┘
      │              │              │
      ▼              ▼              ▼
┌─────────────────────────────────────────────────────────────┐
│                    SearchEngine trait                       │
│  import_excel / search / execute_sql / update_cell / ...   │
├──────────────────┬──────────────────┬──────────────────────┤
│  Memory Engine   │  DuckDB Engine   │  SQLite Engine       │
│  (memory.rs)     │  (duckdb.rs)     │  (sqlite.rs)         │
└──────────────────┴──────────────────┴──────────────────────┘
```

### Design Principles

1. **Engine Agnostic** — CLI/TUI/MCP depend only on the `SearchEngine` trait; they never touch concrete implementations.
2. **Compile-Time Selection** — Engines are chosen via Cargo features; the `DefaultEngine` type alias points to the active engine.
3. **Shared Types** — All data structures live in `types.rs`, shared across every layer.
4. **Async TUI** — The TUI communicates with the engine through an event channel (`event.rs`) to avoid blocking the UI thread.

### Key Dependencies

| Dependency | Purpose |
|------------|---------|
| `ratatui` + `crossterm` | Terminal UI framework |
| `calamine` | Excel file parsing (xlsx, xls, xlsb, ods) |
| `csv` | CSV file parsing |
| `scraper` | HTML DOM parsing, extracting `<table>` ( `html_table` module) |
| `encoding_rs` | Non-UTF-8 HTML/text decoding (`read_file_auto_encoding`) |
| `duckdb` (optional) | DuckDB database engine |
| `rusqlite` (optional) | SQLite database engine |
| `rmcp` (optional) | MCP protocol server implementation |
| `rust_xlsxwriter` (optional) | xlsx file writing (bundled with `mcp-server`) |
| `clap` | CLI argument parsing |
| `regex` | Regular expression matching |
| `serde` + `serde_json` | Serialization / deserialization |
| `schemars` (optional) | JSON Schema generation (for MCP tool descriptions; bundled with `mcp-server`) |

### File Import Pipeline (`crates/core`)

| Module | Responsibility |
|--------|---------------|
| `excel.rs` | Dispatches by extension: `csv` → CSV parser; `html`/`htm` → `html_table`; `txt`/`md`/`markdown` → `text_table`; everything else → calamine Excel |
| `html_table.rs` | Extracts all `<table>` elements from HTML; table names derived from id/caption/preceding heading |
| `text_table.rs` | Markdown pipe tables, dash-separated tables, alignment-heuristic tables |
| `excel::read_file_auto_encoding` | UTF-8 → HTML meta charset → CJK fallback encodings → lossy UTF-8 |

---

## SearchEngine Trait

The `SearchEngine` trait is defined in `crates/core/src/engine/mod.rs` and serves as the unified interface for all database backends.

### Sheet State

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SheetState {
    Virtual,       // VIEW on source file, no data materialized
    Materializing, // Background import in progress
    Materialized,  // TABLE with all data loaded
}
```

### Trait Definition

```rust
pub trait SearchEngine: Send {
    fn new() -> Result<Self> where Self: Sized;

    /// Open engine backed by a database file (for shared concurrent access).
    /// Default: ignore path and use in-memory (`new()`).
    fn with_path(path: &Path) -> Result<Self> where Self: Sized {
        let _ = path;
        Self::new()
    }

    // ── File import ──────────────────────────────────────────
    fn import_excel(&mut self, path: &Path, progress: &dyn Fn(usize, usize)) -> Result<FileInfo>;

    /// Import pre-parsed sheets into the engine.
    /// Used when format override (`--as` flag) bypasses auto-detection.
    fn import_sheets(
        &mut self,
        file_name: &str,
        sheets: Vec<crate::excel::SheetData>,
        progress: &dyn Fn(usize, usize),
    ) -> Result<FileInfo>;

    fn import_excel_repair(&mut self, path: &Path, progress: &dyn Fn(usize, usize)) -> Result<FileInfo>;

    // ── Search ───────────────────────────────────────────────
    fn search(&self, query: &SearchQuery) -> Result<(Vec<SearchResult>, SearchStats)>;

    // ── SQL ──────────────────────────────────────────────────
    fn execute_sql(&self, sql: &str, limit: usize) -> Result<SqlResult>;

    // ── Metadata ─────────────────────────────────────────────
    fn list_files(&self) -> Vec<FileInfo>;
    fn list_table_aliases(&self) -> Vec<TableAliasInfo>;
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
    fn get_sheet_statistics(&self, file_name: &str, sheet_name: &str, max_top_values: usize) -> Result<SheetStatistics>;

    // ── Editing ──────────────────────────────────────────────
    fn update_cell(&mut self, file_name: &str, sheet_name: &str, row: usize, column: &str, value: &str) -> Result<()>;
    fn update_cells(&mut self, file_name: &str, sheet_name: &str, updates: &[(usize, String, String)]) -> Result<usize>;
    fn insert_rows(&mut self, file_name: &str, sheet_name: &str, start_row: usize, rows: Vec<Vec<String>>) -> Result<()>;
    fn delete_rows(&mut self, file_name: &str, sheet_name: &str, start_row: usize, count: usize) -> Result<usize>;
    fn add_column(&mut self, file_name: &str, sheet_name: &str, column_name: &str, default_value: &str) -> Result<()>;
    fn rename_column(&mut self, file_name: &str, sheet_name: &str, old_name: &str, new_name: &str) -> Result<()>;

    // ── Export ───────────────────────────────────────────────
    fn save_as(&self, file_name: &str, output_path: &Path) -> Result<()>;

    // ── Cleanup ──────────────────────────────────────────────
    fn clear(&mut self) -> Result<()>;

    // ── Virtual / Materialization ─────────────────────────────
    fn register_virtual(&mut self, path: &Path, progress: &dyn Fn(usize, usize)) -> Result<FileInfo>;
    fn materialize(&mut self, file_name: &str, progress: &dyn Fn(usize, usize)) -> Result<()>;
    fn sheet_state(&self, file_name: &str, sheet_name: &str) -> Option<SheetState>;

    // ── Session temp tables ─────────────────────────────────
    fn materialize_query(
        &mut self,
        name: &str,
        sql: &str,
        replace: bool,
        max_rows: Option<usize>,
    ) -> Result<TempTableInfo>;
    fn drop_temp_table(&mut self, name: &str) -> Result<()>;
}
```

### DefaultEngine Type Alias

`DefaultEngine` is selected at compile time based on feature flags:

```rust
// src/engine/mod.rs (implemented via cfg attributes)

#[cfg(feature = "engine-duckdb")]
pub use duckdb::DuckDbEngine as DefaultEngine;

#[cfg(all(feature = "engine-sqlite", not(feature = "engine-duckdb")))]
pub use sqlite::SqliteEngine as DefaultEngine;

#[cfg(all(
    feature = "engine-memory",
    not(any(feature = "engine-duckdb", feature = "engine-sqlite"))
))]
pub use memory::MemEngine as DefaultEngine;

#[cfg(not(any(
    feature = "engine-duckdb",
    feature = "engine-sqlite",
    feature = "engine-memory"
)))]
compile_error!("Enable one engine feature: engine-duckdb, engine-sqlite, or engine-memory");
```

Priority: DuckDB > SQLite > Memory. Exactly one engine feature must be enabled.

---

## Adding a New Engine Backend

To add a new database backend (e.g. PostgreSQL, ClickHouse):

### Step 1: Create the Engine File

Create a new engine file under `crates/core/src/engine/`, e.g. `postgres.rs`.

### Step 2: Implement the SearchEngine Trait

```rust
// crates/core/src/engine/postgres.rs

use crate::engine::SearchEngine;
use crate::types::*;
use anyhow::Result;
use std::path::Path;

pub struct PostgresEngine {
    // engine state
}

impl SearchEngine for PostgresEngine {
    fn new() -> Result<Self> {
        // initialize connection
        todo!()
    }

    fn import_excel(&mut self, path: &Path, progress: &dyn Fn(usize, usize)) -> Result<FileInfo> {
        // parse Excel → import to PostgreSQL
        todo!()
    }

    fn search(&self, query: &SearchQuery) -> Result<(Vec<SearchResult>, SearchStats)> {
        // execute search
        todo!()
    }

    fn execute_sql(&self, sql: &str, limit: usize) -> Result<SqlResult> {
        // execute SQL
        todo!()
    }

    fn list_files(&self) -> Vec<FileInfo> {
        todo!()
    }

    // ... implement remaining methods
}
```

### Step 3: Register in Cargo.toml

```toml
# crates/core/Cargo.toml
[features]
engine-postgres = ["dep:postgres"]

[dependencies]
postgres = { version = "0.19", optional = true }
```

Also re-export the feature in the CLI crate:

```toml
# crates/cli/Cargo.toml
[features]
engine-postgres = ["grep-excel-core/engine-postgres"]
```

### Step 4: Update DefaultEngine Selection

Add conditional compilation in `crates/core/src/engine/mod.rs`:

```rust
#[cfg(feature = "engine-postgres")]
mod postgres;

// Update priority
#[cfg(feature = "engine-postgres")]
pub use postgres::PostgresEngine as DefaultEngine;
```

### Step 5: Update CI

Add the PostgreSQL engine build matrix entry in `.github/workflows/release.yml`.

---

## MCP Tool Development

### Architecture

The MCP server is implemented using the `rmcp` crate and runs over stdio transport.

```
AI Assistant (Claude/Cursor)
    │ stdio (JSON-RPC)
    ▼
┌─────────────────────────────┐
│  rmcp Server                │
│  serve(stdio transport)     │
└──────────┬──────────────────┘
           │
           ▼
┌─────────────────────────────┐
│  GrepExcelServer            │
│  #[tool_router]             │
│  ├── import_file            │
│  ├── search                 │
│  ├── execute_sql            │
│  ├── export_query           │
│  ├── get_sheet_statistics   │
│  ├── ... (19 tools total)   │
└──────────┬──────────────────┘
           │
           ▼
┌─────────────────────────────┐
│  SearchEngine trait         │
│  (Arc<RwLock<SyncDb>>)      │
└─────────────────────────────┘
```

### Adding a New MCP Tool

**Step 1: Define the parameter struct** (in `crates/core/src/types.rs`)

```rust
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct MyNewToolParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Parameter description"))]
    pub param1: String,
    pub param2: Option<usize>,
}
```

**Step 2: Add a method to SearchEngine trait** (if a new engine operation is needed)

```rust
// crates/core/src/engine/mod.rs
fn my_new_operation(&mut self, param1: &str, param2: Option<usize>) -> Result<String>;
```

**Step 3: Implement the method in all engine backends**

```rust
// crates/core/src/engine/memory.rs, duckdb.rs, sqlite.rs
fn my_new_operation(&mut self, param1: &str, param2: Option<usize>) -> Result<String> {
    // implementation
    todo!()
}
```

**Step 4: Add a `#[tool]` method to the MCP server** (in `crates/cli/src/mcp.rs`)

```rust
#[tool(description = "Tool description")]
pub async fn my_new_tool(
    &self,
    Parameters(params): Parameters<MyNewToolParams>,
) -> Result<String, String> {
    let db = Arc::clone(&self.db);
    tokio::task::spawn_blocking(move || {
        let mut guard = db.write();
        guard.0.my_new_operation(&params.param1, params.param2)
            .map(|result| serde_json::to_string_pretty(&result).unwrap())
            .map_err(|e| format!("Operation failed: {}", e))
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}
```

**Step 5: Add dispatch in CLI `--exec`** (in `crates/cli/src/main.rs`)

```rust
// inside exec_dispatch() function
"my_new_tool" => {
    let p: MyNewToolParams = serde_json::from_value(params.clone())?;
    let result = db.my_new_operation(&p.param1, p.param2)?;
    Ok(serde_json::to_string_pretty(&result)?)
}
```

**Step 6: Update `--exec help` output** (in the `print_exec_help()` function)

### MCP Server Thread Safety

The MCP server uses `Arc<RwLock<SyncDb>>` to safely share database state across multiple async tasks:

```rust
pub struct GrepExcelServer {
    db: Arc<RwLock<SyncDb>>,
    import_paths: Arc<RwLock<HashMap<String, String>>>,
}
```

- **Read operations** (search, execute_sql, get_metadata, etc.) use `db.read()`
- **Write operations** (update_cell, insert_rows, etc.) use `db.write()`
- **Blocking operations** run on a dedicated thread pool via `tokio::task::spawn_blocking`

### JSON Schema Generation

`schemars` automatically generates JSON Schema for MCP parameters:

```rust
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct SearchParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Search query string"))]
    pub query: String,
    // ...
}
```

`schemars` only compiles when the `mcp-server` feature is enabled, with no impact on other builds.

---

## Type System

All core types are defined in `crates/core/src/types.rs`:

### Search-Related Types

```rust
pub enum SearchMode { FullText, ExactMatch, Wildcard, Regex }

pub struct SearchQuery {
    pub text: String,
    pub column: Option<String>,
    pub mode: SearchMode,
    pub limit: usize,
    pub sheet: Option<String>,
    pub invert: bool,
}

pub struct SearchResult {
    pub file_name: String,
    pub sheet_name: String,
    pub row: Vec<String>,
    pub col_names: Vec<String>,
    pub matched_columns: Vec<usize>,
    pub col_widths: Vec<f64>,
}

pub struct SearchStats {
    pub total_rows_searched: usize,
    pub total_matches: usize,
    pub matches_per_sheet: HashMap<String, usize>,
    pub search_duration: Duration,
    pub truncated: bool,
}
```

### File and Metadata Types

```rust
pub struct FileInfo {
    pub name: String,
    pub sheets: Vec<(String, usize)>,     // (sheet name, row count)
    pub total_rows: usize,
    pub sample: Option<FileSample>,
}

pub struct FileMetadataInfo {
    pub file_name: String,
    pub sheet_count: usize,
    pub sheets: Vec<SheetMetadataInfo>,
}

pub struct SheetMetadataInfo {
    pub sheet_name: String,
    pub row_count: usize,
    pub columns: Vec<String>,
}

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

### MCP / Exec Parameter Types

All structs ending in `Params` are shared between MCP and `--exec` mode:

- `ImportFileParams`
- `SearchParams` (includes `context_lines`, `conditions: Vec<SearchCondition>`)
- `SqlQueryParams`
- `ExportQueryParams` (`sql`, `output_path`, `sheet_name`)
- `GetMetadataParams`
- `GetSheetSampleParams`
- `GetSheetDataParams`
- `GetSheetStatisticsParams` (`file_name`, `sheet_name`, `max_top_values`)
- `SaveAsParams`
- `SaveParams`
- `UpdateCellParams`
- `UpdateCellsParams`
- `InsertRowsParams`
- `DeleteRowsParams`
- `AddColumnParams`
- `RenameColumnParams`

Each condition in `SearchParams.conditions` is a `SearchCondition { column, operator, value }` supporting operators `=`, `!=`, `ILIKE`, `LIKE`, `>`, `<`, `>=`, `<=`, combined with AND logic.

These structs use `#[derive(Deserialize)]` for JSON deserialization, with `#[derive(schemars::JsonSchema)]` additionally applied in MCP builds for schema generation.

---

## Internationalization (i18n)

`crates/core/src/i18n.rs` provides bilingual (Chinese/English) text support. All user-facing strings must go through this module.

### Language Detection

Language is auto-detected from environment variables on startup:

```rust
fn detect() -> Lang {
    let locale = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .unwrap_or_default()
        .to_lowercase();
    if locale.starts_with("zh") { Lang::Zh } else { Lang::En }
}
```

### Adding New Strings

1. Add a new function in `i18n.rs`:

```rust
pub fn my_new_label() -> &'static str {
    match current() {
        Lang::Zh => "中文文案",
        Lang::En => "English text",
    }
}

// With parameter formatting:
pub fn my_new_message(param: &str) -> String {
    match current() {
        Lang::Zh => format!("中文: {}", param),
        Lang::En => format!("English: {}", param),
    }
}
```

2. Use it in code:

```rust
use grep_excel_core::i18n;

let text = i18n::my_new_label();
let message = i18n::my_new_message("value");
```

**Never** hardcode Chinese or English text directly in code — always use i18n functions.

---

## CLI Command Extension

### Adding a New CLI Option

**Step 1**: Add a field to the `Args` struct (`crates/cli/src/main.rs`):

```rust
#[derive(Parser, Debug)]
struct Args {
    // ... existing fields

    #[arg(short = 'n', long, help = "New option description")]
    my_new_option: Option<String>,
}
```

**Step 2**: Add routing logic in `main()`:

```rust
if args.my_new_option.is_some() {
    return run_my_new_command(&args);
}
```

> **Interactive SQL REPL**: `-i` / `--interactive` routes to `run_interactive_cli()`, which calls `interactive::run(db, no_history)` (powered by rustyline), looping over multi-line SQL input and executing via `execute_sql`. Command history persists across sessions to `dirs::state_dir()/grep-excel/history.txt` (disabled with `--no-history`), with incremental `save_history` after each `add_history_entry` to survive crashes mid-session. See `crates/cli/src/interactive.rs` for details.

**Step 3**: Implement the handler function:

```rust
fn run_my_new_command(args: &Args) -> Result<()> {
    let mut db = DefaultEngine::new()?;
    // ... import files
    // ... execute logic
    Ok(())
}
```

### Adding `--exec` Tool Dispatch

Add a new branch to the `match tool` block in `exec_dispatch()` (see MCP Tool Development Step 5 above).

---

## TUI Component Development

The TUI uses the ratatui framework, with code in `crates/cli/src/app/`.

### Key Components

| File | Responsibility |
|------|---------------|
| `mod.rs` | App state management, event loop |
| `handlers.rs` | Keyboard events → App state transitions |
| `render.rs` | Main rendering logic |
| `ui.rs` | Reusable UI components (tables, tabs, etc.) |
| `theme.rs` | Color definitions |

### Adding a New TUI Mode

**Step 1**: Add a new variant to the `AppMode` enum:

```rust
pub enum AppMode {
    Normal,
    EditingSearch,
    // ... existing modes
    MyNewMode,
}
```

**Step 2**: Add mode switching logic in `handlers.rs`

**Step 3**: Add rendering logic for the new mode in `render.rs`

**Step 4**: Add related i18n strings in `i18n.rs`

### TUI Async Model

The TUI communicates with the engine through an event channel to avoid blocking the UI thread:

```rust
pub enum AppEvent {
    Key(KeyEvent),           // Keyboard events
    Tick,                    // Timer events
    FileImported(Result<FileInfo>),
    SearchCompleted(Result<(Vec<SearchResult>, SearchStats)>),
    SqlCompleted(Result<SqlResult>),
    Progress(usize, usize),  // Import progress
}
```

Events flow through an `mpsc::channel`. Expensive engine operations (import, search) run on background threads and notify the UI thread via `event_tx.send()` upon completion.

---

## Data Flow

### Search Data Flow

```
User enters query (CLI / TUI / MCP)
    │
    ▼
SearchQuery { text, column, mode, limit, sheet, invert }
    │
    ▼
SearchEngine::search(&self, query)
    │
    ├── Iterate over all imported files / sheets
    │   ├── If query.column is set → search only matching column
    │   └── Otherwise → search all columns
    │
    ├── Match each row (engine/mod.rs: find_matched_columns)
    │   ├── FullText: case-insensitive substring
    │   ├── ExactMatch: case-sensitive exact match
    │   ├── Wildcard: custom like_match() implementation
    │   └── Regex: regex crate
    │
    └── Returns (Vec<SearchResult>, SearchStats)
        │
        ▼
    SearchResult { file_name, sheet_name, row, col_names, matched_columns }
    SearchStats { total_rows_searched, total_matches, duration, ... }
```

### SQL Data Flow

```
SQL string
    │
    ▼
SearchEngine::execute_sql(&self, sql, limit)
    │
    ├── Memory engine: converts data to in-memory SQLite database → executes
    ├── DuckDB: direct execution (supports window functions, :: type casts, etc.)
    └── SQLite: direct execution
    │
    └── Returns SqlResult { columns, rows, row_count, duration, ... }
```

### MCP Data Flow

```
AI Assistant → JSON-RPC (stdio) → rmcp → GrepExcelServer → SearchEngine
                                                          │
                             JSON response ← rmcp ← serde_json ←┘
```

### Exec Data Flow

```
CLI --exec JSON → serde_json parse → exec_dispatch() → SearchEngine
                       │
                   formatted as markdown/pretty/json/simple → stdout
```

### Run Shell Data Flow

```
CLI --run SHELL_CMD + (--query | --sql)
       │
       ├── Import files → execute search/SQL → get result rows
       │
       ├── For each row:
       │   ├── expand_exec_template(): ${column_name} → shell-escaped cell value
       │   │   └── shell_escape(): single-quote wrap + escape
       │   ├── sh -c <expanded_command>
       │   ├── stdout → print / --run-output-column → update_cell()
       │   └── stderr → eprintln (warning)
       │
       └── --export: save_as() to export full file
```

`expand_exec_template` is implemented in `main.rs` and supports `${column_name}` placeholders and `$$` escape sequences.

### Interactive REPL Data Flow

```
CLI -i (files...) 
    │
    ├── Import positional argument files
    │
    ├── interactive::run() (interactive.rs, rustyline)
    │   ├── $ prompt → read multi-line input (Validator: submits on ';' or '.' suffixed)
    │   ├── SQL input → execute_and_print() → SearchEngine::execute_sql()
    │   │              → Unicode-aligned table output + row count
    │   └── Dot commands (.tables/.files/.help/.history/.clear/.exit)
    │
    └── Ctrl+D / .exit → exit
```

### Excel Date Auto-Detection (Import Phase)

```
import_excel (excel.rs)
    │
    ├── Pass 1: collect calamine Data raw rows
    │
    ├── detect_date_columns_from_data()
    │   ├── Signal 1 (high confidence): column has ≥1 Data::DateTime cell → mark as date column
    │   └── Signal 2 (conservative fallback): column name matches date keywords + >50% Float values are whole integers in Excel serial range [1, 100000]
    │
    ├── Pass 2: call date_aware_to_string() / serial_to_datetime_string() on date columns
    │           convert to ISO 8601 format (YYYY-MM-DD or YYYY-MM-DD HH:MM:SS)
    │
    └── --repair path: convert_date_columns_in_place() applies the same post-processing
```

---

## Release Process

### CI/CD Configuration

Releases are managed by `.github/workflows/release.yml`, triggered when a version tag is pushed:

```
git tag vX.Y.Z
git push origin vX.Y.Z
```

### Build Matrix

CI builds the following platforms:

| Platform | Target Triple | Engine |
|----------|---------------|--------|
| Linux x86_64 | `x86_64-unknown-linux-gnu` | DuckDB |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | DuckDB |
| macOS Intel | `x86_64-apple-darwin` | DuckDB |
| macOS ARM | `aarch64-apple-darwin` | DuckDB |
| Windows x86_64 | `x86_64-pc-windows-msvc` | DuckDB |

All builds use `--features engine-duckdb,file-dialog,mcp-server,share-url,pdf-support` with `cargo zigbuild` for Linux glibc 2.31 compatibility.

### Release Artifacts

Each platform produces a zip archive containing the binary and LICENSE file:

```
grep_excel-{target}-v{version}.zip
├── grep_excel (or grep_excel.exe)
└── LICENSE
```

### Manual Release Checklist

- [ ] `cargo test --features full` passes
- [ ] `cargo clippy --all-features` produces no warnings
- [ ] `Cargo.toml` version updated
- [ ] `README.md` version updated (if applicable)
- [ ] Manual verification of platform binaries
- [ ] git tag created and pushed

---

## Related Documentation

- [README.md](../README.md) — Project overview (English)
- [README.zh-CN.md](../README.zh-CN.md) — Project overview (Chinese)
- [UserGuide.md](UserGuide.md) — End-user manual (English)
- [UserGuide.zh-CN.md](UserGuide.zh-CN.md) — End-user manual (Chinese)
- [CONTRIBUTING.md](../CONTRIBUTING.md) — Contribution guide (English)
- [CONTRIBUTING.zh-CN.md](../CONTRIBUTING.zh-CN.md) — Contribution guide (Chinese)
- [DeveloperGuide.zh-CN.md](./DeveloperGuide.zh-CN.md) — Chinese version of this document
