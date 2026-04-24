# grep-excel

TUI tool for searching Excel/CSV files with DuckDB-powered performance.

grep-excel provides a fast, interactive terminal interface for searching across multiple Excel and CSV files. It uses DuckDB as the query engine for high-performance full-text search, exact matching, wildcard pattern matching, and regex search.

## Features

- Full-text search across all imported Excel/CSV files
- Exact match mode for precise string comparisons
- Wildcard pattern matching with `%` and `_` operators
- Regex mode for multi-keyword OR search (e.g. `keyword1|keyword2`)
- Column filtering to narrow search scope
- **Raw SQL queries** — execute `SELECT` statements directly against imported data (DuckDB/SQLite engines)
- Native file picker dialog (press `o` in TUI, requires `file-dialog` feature)
- Matched cells highlighted in green in search results
- Multi-file support — import and search across multiple spreadsheets
- DuckDB backend for high-performance in-memory queries
- Cross-platform: Windows, macOS (Intel/Apple Silicon), and Linux (x64/ARM64)
- Terminal UI built with ratatui for smooth keyboard-driven navigation

## Installation

### Download Prebuilt Binaries

Download the latest release for your platform from the [GitHub Releases page](https://github.com/c2j/grep-excel/releases).

Available targets:
- Windows (x86_64)
- macOS (Intel x86_64, Apple Silicon ARM64)
- Linux (x86_64, ARM64)

### Build from Source

```bash
git clone https://github.com/c2j/grep-excel.git
cd grep-excel
cargo build --release
```

The binary will be available at `target/release/grep_excel`.

> **Note:** Set `DUCKDB_DOWNLOAD_LIB=1` when building to download pre-built DuckDB libraries instead of compiling from source, which significantly speeds up the build process.

## Usage

### Command Line

```bash
grep_excel [FILES...] [OPTIONS]
```

### CLI Options

| Flag | Short | Description |
|------|-------|-------------|
| `--query` | `-q` | Search query string |
| `--column` | `-c` | Filter to specific column name |
| `--mode` | `-m` | Search mode: `fulltext` (default), `exact`, `wildcard`, or `regex` |
| `--sql` | `-x` | Execute a SQL `SELECT` query against imported data |
| `--export` | `-e` | Export search results to a CSV file |
| `--mcp` | — | Start MCP server mode (stdio) |

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

Launch in TUI mode (no CLI arguments):
```bash
grep_excel
```

### SQL Query Examples

Run a SQL query directly from the command line:

```bash
# Basic SELECT
grep_excel data.xlsx --sql "SELECT * FROM sheet_1_0 LIMIT 10"

# Aggregation query
grep_excel data.xlsx --sql "SELECT City, COUNT(*) FROM sheet_1_0 GROUP BY City"

# Join across sheets (requires two files)
grep_excel employees.xlsx departments.xlsx --sql "SELECT e.Name, d.DeptName FROM sheet_1_0 e JOIN sheet_2_0 d ON e.DeptId = d.Id"
```

**Note:** When using the DuckDB engine, you can use DuckDB-specific functions like `ILIKE`, `regexp_matches()`, and `::` casts. When using the SQLite engine, use SQLite-compatible syntax (`LIKE`, custom `regexp()`). Incompatible queries will fail at runtime with the engine's native error message.

**Table naming:** Imported sheets are stored as `sheet_{file_id}_{sheet_index}`. In TUI, press `o` to see loaded files and their sheet indices. In MCP, use `list_files` to discover tables.

## MCP Server Mode

Start the MCP server for integration with AI assistants (e.g., Claude, Cursor):

```bash
grep_excel --mcp
```

Available MCP tools:

| Tool | Description |
|------|-------------|
| `import_file` | Import an Excel/CSV file |
| `list_files` | List imported files and their sheets |
| `get_metadata` | Get detailed metadata: sheet count, sheet names, column names per sheet |
| `get_sheet_sample` | Get sampled rows from a specific sheet |
| `get_sheet_data` | Get paginated row data with column filtering |
| `search` | Search with fulltext/exact/wildcard/regex |
| `execute_sql` | Execute a raw SQL `SELECT` query |
| `save_as` | Save imported data to a new Excel file (Save As) |

### MCP Tool Details

#### `import_file`
Import an Excel/CSV file for searching and querying.
- **Parameters:** `file_path` (string) — Absolute or relative path to the file

#### `list_files`
List all imported files with their sheet names and row counts. Returns file name, sheets, and total rows.

#### `get_metadata`
Get detailed metadata including column names for each sheet.
- **Parameters:**
  - `file_name` (string, optional) — File name as shown in `list_files`. If omitted, returns metadata for all imported files.
- **Returns:** Sheet count, sheet names, row counts, and column names per sheet.

#### `get_sheet_sample`
Get a representative sample of rows from a specific sheet using deterministic evenly-spaced sampling.
- **Parameters:**
  - `file_name` (string) — File name as shown in `list_files`
  - `sheet_name` (string) — Sheet name within the file
  - `sample_size` (integer, optional, default: 10) — Number of rows to sample

#### `get_sheet_data`
Get rows from a specific sheet with pagination and optional column filtering.
- **Parameters:**
  - `file_name` (string) — File name as shown in `list_files`
  - `sheet_name` (string) — Sheet name within the file
  - `start_row` (integer, optional, default: 0) — Start row index (0-based, inclusive)
  - `end_row` (integer, optional) — End row index (exclusive). Default: all rows from `start_row`
  - `columns` (string array, optional) — Column names to include. Default: all columns

#### `search`
Search across all imported files with fulltext/exact/wildcard/regex modes.
- **Parameters:**
  - `query` (string) — Search query string
  - `column` (string, optional) — Filter to a specific column name
  - `mode` (string, optional) — Search mode: `fulltext` (default), `exact`, `wildcard`, or `regex`
  - `limit` (integer, optional, default: 100) — Maximum results to return
  - `aggregate` (string, optional) — Column name to count distinct values in matched rows

#### `execute_sql`
Execute a SQL `SELECT` query against imported data.
- **Parameters:**
  - `sql` (string) — SQL SELECT query to execute
  - `limit` (integer, optional, default: 1000) — Maximum results to return

#### `save_as`
Save imported data to a new Excel file. Does not modify the original file.
- **Parameters:**
  - `file_name` (string) — Source file name as shown in `list_files`
  - `output_path` (string) — Output file path for the new xlsx file
  - `sheet_name` (string, optional) — Specific sheet to export. If omitted, exports all sheets.

### MCP Example Workflows

**Explore an unknown file:**
```
User: Import data.xlsx and tell me what's in it
Assistant: [calls import_file with file_path="data.xlsx"]
           → Shows file name, sheets, and row counts

Assistant: [calls get_metadata with file_name="data.xlsx"]
           → Shows each sheet's column names

Assistant: [calls get_sheet_sample with file_name="data.xlsx", sheet_name="Employees", sample_size=5]
           → Shows 5 evenly-spaced rows so you can understand the data
```

**Paginated data access:**
```
User: Show me rows 20-40 of the Orders sheet, just the Customer and Amount columns
Assistant: [calls get_sheet_data with file_name="data.xlsx", sheet_name="Orders",
            start_row=20, end_row=40, columns=["Customer", "Amount"]]
```

**Export filtered results:**
```
User: Save the Products sheet to a new file called products_backup.xlsx
Assistant: [calls save_as with file_name="data.xlsx", sheet_name="Products",
            output_path="products_backup.xlsx"]
           → "Successfully saved sheet 'Products' to 'products_backup.xlsx'"
```

**SQL analysis:**
```
User: Import data.xlsx and show me the top 5 salaries
Assistant: [calls import_file with file_path="data.xlsx"]
Assistant: [calls execute_sql with sql="SELECT * FROM sheet_1_0 ORDER BY Salary DESC LIMIT 5"]
```

## TUI Keybindings

| Key | Action |
|-----|--------|
| `q` | Quit |
| `/` | Enter search query |
| `c` | Set column filter |
| `S` | Enter **SQL query mode** — type a raw `SELECT` query and press Enter to execute |
| `Tab` | Cycle through search modes (fulltext / exact / wildcard / regex) |
| `Enter` | Execute search / open detail panel |
| `o` | Open file picker (with `file-dialog` feature) or view loaded files |
| `?` | Show help |
| `j` / `k` | Navigate results down/up |
| `g` | Jump to top of results |
| `G` | Jump to bottom of results |
| `d` | Clear all data (search results + SQL results) |
| `s` | Export current results to CSV |
| `n` | Load more results (when truncated) |
| `←` / `→` | Scroll columns left/right |
| `1`–`9` | Switch to tab |

### SQL Mode in TUI

1. Press `S` to enter SQL mode (search bar turns into a SQL input field)
2. Type your SQL query, e.g. `SELECT City, COUNT(*) FROM sheet_1_0 GROUP BY City`
3. Press `Enter` to execute
4. Results display in the results table
5. Press `d` to clear SQL results and return to normal search mode

## Supported Formats

grep-excel can read the following formats:

- `.xlsx` - Excel 2007+ (Open XML)
- `.xls` - Excel 97-2004 (BIFF8)
- `.xlsm` - Excel Macro-Enabled
- `.xlsb` - Excel Binary
- `.ods` - OpenDocument Spreadsheet
- `.csv` - Comma-Separated Values

## Build from Source

Requirements:
- Rust 1.70+ and Cargo

```bash
# Clone the repository
git clone https://github.com/c2j/grep-excel.git
cd grep-excel

# Build release binary (with native file dialog)
cargo build --release

# Build without file dialog (for headless environments)
cargo build --release --no-default-features

# Or with faster DuckDB library download
DUCKDB_DOWNLOAD_LIB=1 cargo build --release
```

> **Note:** `DUCKDB_DOWNLOAD_LIB=1` downloads pre-built DuckDB libraries instead of compiling from source, which significantly speeds up the build process.

## License

MIT License - see [LICENSE](LICENSE) for details.
