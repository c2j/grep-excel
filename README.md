[中文版](README.zh-CN.md)

# grep-excel

TUI tool for searching tabular data files (Excel/CSV/TSV/HTML/text/Markdown/Word/PowerPoint/DBF/XML) with DuckDB-powered performance.

grep-excel provides a fast, interactive terminal interface for searching across multiple spreadsheet and table files. It uses DuckDB as the query engine for high-performance full-text search, exact matching, wildcard pattern matching, regex search, and SQL queries. Supports CLI one-shot queries, TUI interactive mode, MCP server integration with AI assistants, and batch `--exec` command pipelines.

## Features

- **Multiple Search Modes** — Full-text (case-insensitive substring), Exact (case-sensitive), Wildcard (SQL LIKE `%` / `_`), Regex (multi-keyword OR with `|`)
- **SQL Queries** — Execute `SELECT` statements directly against imported data with DuckDB analytical functions
- **Multi-Engine Backend** — DuckDB (high-performance OLAP), SQLite, or pure in-memory engine; select via feature flags
- **TUI Interactive Mode** — Keyboard-driven terminal interface with ratatui, auto-browse on import, tabbed results, detail panel, flat/table views, Ctrl+arrow file/sheet navigation
- **HTML / Text / Markdown Tables** — Import HTML reports (e.g. WDR/AWR), plain-text tables, and GFM Markdown pipe tables as queryable sheets; encoding auto-detected (UTF-8 / meta charset / CJK fallback)
- **Word / PowerPoint / DBF / XML** — Extract tables from `.docx` documents (sheet names derived from heading paragraphs, merged cells forward-filled) and `.pptx` slides (one sheet per slide); import dBase `.dbf` and flat `.xml` files as queryable sheets; use `--as` to force a format when the extension is missing or misleading
- **MCP Server Mode** — Integrate with AI assistants (Claude, Cursor) via 19 MCP tools for search, data exploration, statistics, editing, and export
- **Interactive SQL REPL** — Multi-line SQL shell with `;`-terminated input, command history, and dot-commands (`.tables`, `.files`, `.output`, `.save`, `.help`); launch with `-i`
- **CLI `--exec` Pipeline** — Execute MCP tools from the command line as single commands or multi-step JSON arrays
- **File Editing** — Update cells, insert/delete rows, add/rename columns, save back to original or export as new file (spreadsheet formats; `.docx`/`.pptx` are read-only)
- **Aggregate Statistics** — Count distinct value distributions in matched rows by column
- **Repair Damaged Files** — Recover data from corrupted `.xlsx` files at the ZIP/XML level (`--repair`)
- **Cloud Share URL Import** — Pass Kingsoft Docs / WPS (`kdocs.cn`) share links directly; downloads via session cookie. Use `--kdocs-cookie` or `KDOCS_COOKIE` env var. For enterprise domains, set `SHARE_HOSTS` env var
- **Archive Support** — Import table files directly from `.zip`, `.tar`, `.tar.gz`, `.tar.bz2`, `.tar.xz` archives and `.zip.001` split volumes without manual decompression
- **Excel Date Auto-Detection** — Detects Excel date serial numbers and converts them to ISO 8601 format (`YYYY-MM-DD` or `YYYY-MM-DD HH:MM:SS`), preserving time information for date-range searches
- **Multiple Output Formats** — Markdown tables, pretty-printed, JSON, and simple TSV (`--format`)
- **CSV Export** — Export search or SQL results to CSV files
- **Friendly Table Aliases** — Use `filename.sheetname` syntax in SQL instead of internal `sheet_N_M` names
- **i18n / Chinese Support** — Auto-detects language from `LANG`/`LC_ALL` environment variables; TUI, CLI, and help text in both Chinese and English
- **Cross-Platform** — Windows (x86_64, Win7+), macOS (Intel x86_64, Apple Silicon ARM64), Linux (x86_64 glibc 2.31+, ARM64)

## Installation

### Download Prebuilt Binaries

Download the latest release for your platform from the [GitHub Releases page](https://github.com/c2j/grep-excel/releases).

Available targets:
- Windows (x86_64)
- macOS (Intel x86_64, Apple Silicon ARM64)
- Linux (x86_64, ARM64)

### Build from Source

Requirements: Rust 1.70+ and Cargo

```bash
git clone https://github.com/c2j/grep-excel.git
cd grep-excel

# Default build (in-memory engine + file dialog)
cargo build --release

# With DuckDB engine (recommended for production)
cargo build --release --features full

# With DuckDB bundled (self-contained, no system DuckDB required)
cargo build --release --features duckdb-bundled

# With SQLite engine
cargo build --release --features engine-sqlite

# Headless build (no file dialog)
cargo build --release --no-default-features --features engine-memory
```

> **Tip:** Set `DUCKDB_DOWNLOAD_LIB=1` when building with DuckDB features to download pre-built libraries instead of compiling from source, significantly speeding up builds.

### Feature Flags

| Feature | Description |
|---------|-------------|
| `engine-memory` | In-memory engine (default) |
| `engine-duckdb` | DuckDB engine (high-performance) |
| `engine-sqlite` | SQLite engine |
| `duckdb-bundled` | DuckDB with bundled C library (self-contained) |
| `file-dialog` | Native file picker dialog (default) |
| `mcp-server` | MCP server mode + xlsx write support for AI assistant integration (includes `save_as` / `save`) |
| `share-url` | Cloud share URL import (kdocs.cn / enterprise domains) |
| `archive-support` | Archive support: `.zip`, `.tar`, `.tar.gz`, `.tar.bz2`, `.tar.xz`, `.tar.zst`, split `.zip.001` |
| `pdf-support` | PDF document table extraction (read-only) |
| `parquet-support` | Parquet columnar format reader (read-only) |
| `full` | Everything: memory engine + file dialog + MCP server + share URL + archive support + pdf-support + parquet-support |

## Usage

```bash
grep_excel [FILES...] [OPTIONS]
```

### CLI Options

| Flag | Short | Description |
|------|-------|-------------|
| `--interactive` | `-i` | Launch interactive SQL REPL: `$` prompt, multi-line input (`;` to run), up/down history, dot-commands |
| `--no-history` | — | Disable persistent SQL history across sessions (history is saved by default) |
| `--query` | `-q` | Search query string |
| `--column` | `-c` | Filter to specific column name |
| `--sheet` | `-s` | Filter to specific sheet name |
| `--mode` | `-m` | Search mode: `fulltext` (default), `exact`, `wildcard`, `regex` |
| `--invert` | `-v` | Invert match: show rows that do NOT match |
| `--sql` | `-x` | Execute a SQL `SELECT` query against imported data |
| `--export` | `-e` | Export search results to a CSV file |
| `--exec` | `-E` | Execute MCP tool command(s) as JSON |
| `--mcp` | — | Start MCP server mode (stdio) |
| `--aggregate` | `-g` | Count distinct values in matched rows by column |
| `--list-tables` | `-t` | List imported tables with friendly names and columns |
| `--format` | `-f` | Output format: `markdown` (default), `pretty`, `json`, `simple` (TSV) |
| `--repair` | `-r` | Repair corrupted xlsx files before importing (ZIP/XML level) |
| `--run` | `-X` | Execute a shell command for each matching row. Use `${col_name}` for cell values |
| `--run-output-column` | — | Write `--run` command stdout to a column (creates if not exists) |
| `--help` | `-h` | Show help including supported formats (auto-detects Chinese or English) |
| `--kdocs-cookie` | — | Cookie for Kingsoft Docs (kdocs.cn) share URL downloads only |
| `--share-hosts` | — | Additional comma-separated hosts for enterprise cloud share URLs |
| `--as` | — | Sticky per-file format override: applies to all following files until the next `--as`. Valid: `csv`, `tsv`, `html`, `txt`, `md`, `dbf`, `xml`, `excel`, `docx`, `pptx`, `pdf`, `parquet` |

### Examples

Search a single file:
```bash
grep_excel data.xlsx -q "search term"
```

Search multiple files with column filter:
```bash
grep_excel a.xlsx b.xlsx -c "Name" -m exact
```

Wildcard search (use `%` for any characters, `_` for single character):
```bash
grep_excel data.xlsx -q "Jo%" -m wildcard
```

Regex multi-keyword search (use `|` for OR):
```bash
grep_excel data.xlsx -q "张三|李四" -m regex
```

Search a specific sheet only:
```bash
grep_excel data.xlsx -q "Engineering" -s Employees
```

Invert match — find rows that do NOT contain the query:
```bash
grep_excel data.xlsx -q "Engineering" -v
```

Aggregate statistics — count distinct values:
```bash
grep_excel data.xlsx -q "Engineering" -g Department
```

List tables with friendly aliases:
```bash
grep_excel data.xlsx employees.xlsx -t
```

Repair and import corrupted xlsx:
```bash
grep_excel corrupted.xlsx -q "data" -r
```

Import from a WPS cloud share link:
```bash
export KDOCS_COOKIE='wps_sid=...; ...'
grep_excel 'https://www.kdocs.cn/l/xxxx' -q "search term"

# Or pass cookie directly
grep_excel --kdocs-cookie "$KDOCS_COOKIE" 'https://www.kdocs.cn/l/xxxx' -t
```

Search all table files inside a ZIP archive:
```bash
grep_excel audit_2026.zip -q "异常交易"

# Search CSV files inside a tar.gz
grep_excel db_dump.tar.gz -x "SELECT * FROM sheet_1_0 LIMIT 10"

# Multi-volume split ZIP
grep_excel big_data.zip.001 -t
```

Export results with specific format:
```bash
grep_excel data.xlsx -q "keyword" -e results.csv -f json
```

Launch in TUI mode (no CLI arguments):
```bash
grep_excel
```

Launch the interactive SQL REPL:
```bash
# Start REPL with files pre-imported
grep_excel data.xlsx employees.xlsx -i

# Inside the REPL:
#   $ SELECT * FROM sheet_1_0 LIMIT 5;
#   $ .tables          # list imported tables
#   $ .files           # list imported files
#   $ .output out.csv  # redirect subsequent SQL results to CSV file
#   $ .output          # restore terminal output
#   $ .save out.json json  # save last SQL result (csv|json|tsv|table)
#   $ .let t AS SELECT City, COUNT(*) AS n FROM sheet_1_0 GROUP BY City
#                      # materialize a SQL query as a temp table "t"
#   $ SELECT * FROM t; # query the temp table by bare name
#   $ .drop t          # drop the temp table when done
#   $ .help            # show dot-commands
#   $ .exit            # quit (Ctrl+D also works)
```

SQL and dot-commands entered in the REPL are saved to a history file
(`~/.local/state/grep-excel/history.txt` on Linux,
`~/Library/Application Support/grep-excel/history.txt` on macOS) and recalled
with up/down arrows across sessions. Pass `--no-history` to opt out for a
session.

Search HTML / text / Markdown tables the same way as Excel:
```bash
grep_excel report.html -q "CPU"
grep_excel awr.md -x "SELECT * FROM \"Host CPU\" LIMIT 10"
grep_excel data.txt -t
```

Search Word, PowerPoint, DBF, and XML files the same way:
```bash
grep_excel report.docx -q "budget"        # tables from word/document.xml
grep_excel slides.pptx -t                 # one sheet per slide
grep_excel legacy.dbf -q "Smith"          # dBase database
grep_excel data.xml -x "SELECT * FROM data LIMIT 10"
```

Force a format when the extension is missing or misleading (`--as` is sticky — it applies to every file that follows until the next `--as`):
```bash
grep_excel --as csv access.log --as excel dump.dat -t
```

Execute a shell command for each matching row (`--run` / `-X`):
```bash
# Run an external tool for each match, substitute ${column_name}
grep_excel data.xlsx -q "ERROR" -c "Level" --run './analyzer "${Message}"'

# Write command output to a new column, then export
grep_excel data.xlsx -q "TODO" -c "Type" --run './classifier "${Title}"' --run-output-column "Category" -e output.xlsx

# Combine --run with --sql
grep_excel data.xlsx --sql "SELECT Name, SQL FROM sheet_1_0 WHERE Type='legacy'" --run './formatter "${SQL}"'
```

> `--run` executes `sh -c` for each matching row. Use `${column_name}` to reference cell values (values are automatically shell-escaped). `$$` produces a literal `$`.

### SQL Query Examples

```bash
# Basic SELECT
grep_excel data.xlsx --sql "SELECT * FROM sheet_1_0 LIMIT 10"

# Aggregation query
grep_excel data.xlsx --sql "SELECT City, COUNT(*) FROM sheet_1_0 GROUP BY City"

# Friendly aliases (use --list-tables to discover names)
grep_excel employees.xlsx departments.xlsx --sql "SELECT e.Name, d.DeptName FROM employees.Sheet1 e JOIN departments.Sheet1 d ON e.DeptId = d.Id"
```

> **Note:** When using the DuckDB engine, you can use DuckDB-specific functions like `ILIKE`, `regexp_matches()`, `::` casts, and window functions. With SQLite, use SQLite-compatible syntax (`LIKE`, custom `regexp()`).

### CLI Exec Examples

Execute MCP tools directly from the command line using `--exec` with JSON:

**Single command** (files auto-imported via positional args):
```bash
# List files
grep_excel data.xlsx --exec '{"tool":"list_files","params":{}}'

# Search with aggregation
grep_excel data.xlsx --exec '{"tool":"search","params":{"query":"Engineering","mode":"exact","aggregate":"City"}}'

# Get metadata (all files)
grep_excel data.xlsx --exec '{"tool":"get_metadata","params":{}}'
```

**Multi-step pipeline** (JSON array, state preserved across steps):
```bash
grep_excel --exec '[
  {"tool":"import_file","params":{"file_path":"data.xlsx"}},
  {"tool":"get_metadata","params":{}},
  {"tool":"get_sheet_sample","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","sample_size":3}},
  {"tool":"search","params":{"query":"张三","mode":"exact"}},
  {"tool":"update_cell","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","row":0,"column":"Name","value":"fixed"}},
  {"tool":"save","params":{"file_name":"data.xlsx"}}
]'
```

Available tools for `--exec`: `import_file`, `list_files`, `get_metadata`, `get_sheet_sample`, `get_sheet_data`, `get_sheet_statistics`, `search`, `execute_sql`, `materialize_query`, `drop_temp_table`, `export_query`, `save_as`, `save`, `update_cell`, `update_cells`, `insert_rows`, `delete_rows`, `add_column`, `rename_column`.

## MCP Server Mode

Start the MCP server for integration with AI assistants (e.g., Claude, Cursor):

```bash
grep_excel --mcp
```

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `import_file` | Import a tabular file (Excel/CSV/TSV/HTML/text/Markdown/docx/pptx/DBF/XML/PDF/Parquet) |
| `list_files` | List imported files and their sheets |
| `get_metadata` | Get detailed metadata: sheet names, columns per sheet |
| `get_sheet_sample` | **Preview** a sheet: get N evenly-spaced rows (default 10). Fastest way to understand structure without loading all rows |
| `get_sheet_data` | Get rows from a sheet with pagination (`start_row`/`end_row` as numbers) and column filtering |
| `search` | Search with fulltext/exact/wildcard/regex + aggregation + context lines + multi-condition AND filtering |
| `execute_sql` | Execute a raw SQL `SELECT` query |
| `materialize_query` | Materialize a read-only SQL result into a named session temp table for reuse in later `execute_sql` calls. Name: `[A-Za-z_][A-Za-z0-9_]*`. Query by bare name; aliases show as `temp.<name>` |
| `drop_temp_table` | Drop a session temp table previously created by `materialize_query`. Cannot drop imported file tables |
| `export_query` | Run a SQL SELECT and export results to a new .xlsx file |
| `get_sheet_statistics` | Get per-column statistics (null counts, distinct counts, top values) for data profiling |
| `save_as` | Save imported data to a new Excel file (Save As) |
| `save` | Overwrite the original imported file with current data |
| `update_cell` | Update a single cell value |
| `update_cells` | Batch update multiple cells |
| `insert_rows` | Insert rows at a specified position |
| `delete_rows` | Delete rows from a specified position |
| `add_column` | Add a new column with a default value |
| `rename_column` | Rename an existing column |

### MCP Configuration

Add to your AI assistant's MCP config (e.g., `claude_desktop_config.json` or Cursor's MCP settings):

```json
{
  "mcpServers": {
    "grep-excel": {
      "command": "/path/to/grep_excel",
      "args": ["--mcp"]
    }
  }
}
```

### MCP Example Workflows

**Explore an unknown file:**
```
User: Import data.xlsx and tell me what's in it
Assistant: [calls import_file with file_path="data.xlsx"]
           → Shows file name, sheets, and row counts

Assistant: [calls get_metadata with file_name="data.xlsx"]
           → Shows each sheet's column names

Assistant: [calls get_sheet_sample with file_name="data.xlsx", sheet_name="Employees", sample_size=5]
           → Shows 5 evenly-spaced rows to understand the data
```

**Edit and save:**
```
User: Fix the department name for row 3 — change "Enginering" to "Engineering"
Assistant: [calls update_cell with file_name="data.xlsx", sheet_name="Employees",
            row=2, column="Department", value="Engineering"]

User: Now save
Assistant: [calls save with file_name="data.xlsx"]
```

### Tips for Effective Use

**Preview large sheets without loading every row:**
Use `get_sheet_sample` to fetch a small set of evenly-spaced rows. This is the fastest way to understand a sheet's structure, value formats, and column semantics before running expensive searches or SQL queries.

**Use friendly table aliases in SQL — not internal names:**
Each imported sheet is exposed under both an internal name (`sheet_{file_id}_{sheet_idx}`, e.g. `sheet_1_0`) and a friendly alias (`{file_stem}.{sheet_name}`, e.g. `data.Employees`). Prefer the alias for readability:

```sql
SELECT * FROM data.Employees WHERE "Department" = 'Engineering'
```

Run `--list-tables` from the CLI, or call the `list_files` MCP tool, to discover every available alias for the current session.

**SQL already supports JOINs, window functions, and aggregations:**
There is no need for separate filter, sort, or aggregation tools — `execute_sql` passes your query straight to DuckDB (or SQLite), which supports the full analytical SQL surface:

```sql
-- JOIN across two imported files
SELECT e.Name, d.DeptName
FROM employees.Sheet1 e
JOIN departments.Sheet1 d ON e."DeptId" = d."Id"

-- Window function (DuckDB engine)
SELECT *,
       ROW_NUMBER() OVER (PARTITION BY "DeptId" ORDER BY "Salary" DESC) AS rank
FROM data.Employees

-- Aggregation with GROUP BY
SELECT "DeptId", COUNT(*) AS headcount
FROM data.Employees
GROUP BY "DeptId"
ORDER BY headcount DESC
```

**`get_sheet_data` pagination parameters are numbers, not strings:**
`start_row` and `end_row` are optional integers. Pass them as JSON numbers (e.g. `"start_row": 0`), never as strings (`"start_row": "0"`). Omit both to fetch all rows.

**Session temp tables for complex analysis:**
Use `materialize_query` (or the REPL `.let` command) to save intermediate SQL results as a named temp table. This is especially useful when a CTE doesn't fit your workflow, or when you need to query the same derived result across multiple steps without re-running expensive subqueries:

```sql
-- Materialize a filtered subset
materialize_query(name="dept_summary", sql="SELECT DeptId, COUNT(*) AS n FROM data.Employees GROUP BY DeptId")

-- Then query it like any table (by bare name)
SELECT * FROM dept_summary WHERE n > 10
```

Use `drop_temp_table` (or `.drop`) to clean up when the temp table is no longer needed. Temp tables are session-scoped and disappear when the process exits.

**Recommended exploration workflow:**
`import_file` → `get_metadata` → `get_sheet_sample` → `search` or `execute_sql`. Use `get_sheet_sample` instead of `get_sheet_data` whenever you only need to understand the data shape; it is significantly cheaper for large files.

## TUI Keybindings

| Key | Action |
|-----|--------|
| `q` | Quit |
| `/` or `e` | Enter search query |
| `c` | Set column filter |
| `a` | Set aggregate column |
| `S` | Enter **SQL query mode** — type a raw `SELECT` query |
| `Tab` | Cycle through search modes (fulltext / exact / wildcard / regex) |
| `Enter` | Execute search / open detail panel |
| `o` | Open file picker (with `file-dialog` feature) or view loaded files |
| `?` or `h` | Show help |
| `j` / `k` | Navigate results down/up |
| `g` | Jump to top of results |
| `G` | Jump to bottom of results |
| `d` | Clear all data (search results + SQL results) |
| `s` | Export current results to CSV |
| `n` | Load more results (when truncated; also loads more browse rows) |
| `v` | Toggle flat/table view |
| `←` / `→` | Scroll columns left/right |
| `H` / `L` | Scroll columns left/right (vim-style) |
| `[` / `]` | Previous / next sheet (browse mode; across all files) |
| `Ctrl+←` / `Ctrl+→` | Switch sheet within the current file (browse / flat / table views) |
| `Ctrl+↑` / `Ctrl+↓` | Switch file |
| `1`–`9` | Switch to tab (or Nth sheet in browse mode) |

Importing files in TUI **auto-browses** the first sheet (no search required). Multi-file tabs show `file:sheet` labels; the All tab uses a single **Source** column (`file:sheet`) instead of separate file/sheet columns.

## Supported Formats

- `.xlsx` — Excel 2007+ (Open XML)
- `.xls` — Excel 97-2004 (BIFF8)
- `.xlsm` — Excel Macro-Enabled
- `.xlsb` — Excel Binary
- `.ods` — OpenDocument Spreadsheet
- `.csv` — Comma-Separated Values
- `.tsv` / `.tab` — Tab-Separated Values
- `.html` / `.htm` — HTML tables (encoding auto-detected; each `<table>` becomes a sheet)
- `.txt` — Plain-text tables (section / dash-separator / alignment heuristics)
- `.md` / `.markdown` — GitHub-Flavored Markdown pipe tables
- `.dbf` — dBase database files
- `.xml` — XML data files (flat convention: repeated sibling elements under the root become rows, their child tags become columns)
- `.docx` — Word documents (tables extracted from word/document.xml; read-only, no editing)
- `.pptx` — PowerPoint presentations (tables extracted from each slide; read-only, no editing)
- `.pdf` — PDF documents (text-based, table extraction via lattice strategy; read-only, no OCR)
- `.parquet` — Parquet columnar format (read-only; native types preserved via DuckDB engine)
- `.zip` — ZIP archives; table files inside are automatically extracted and imported
- `.tar` / `.tar.gz` / `.tgz` / `.tar.bz2` / `.tar.xz` / `.tar.zst` — TAR archives (compressed or uncompressed)
- `.zip.001` / `.zip.002` — Multi-volume split ZIP archives

Archives are handled transparently: pass a `.zip` or `.tar.gz` directly and all recognizable table files inside are extracted and imported as separate entries with `archive::path/file.xlsx` naming.

## License

MIT License — see [LICENSE](LICENSE) for details.
