[中文版](./UserGuide.zh-CN.md)

# grep-excel User Guide

grep-excel is a high-performance, multi-format tabular file search tool built on DuckDB. It handles Excel / CSV / TSV / HTML / text / Markdown / Word / PowerPoint / DBF / XML and offers several ways to work: an interactive terminal UI (TUI), one-shot command-line queries (CLI), an MCP server for AI assistant integration, and batch `--exec` pipelines.

## Table of Contents

- [Quick Start](#quick-start)
- [Supported File Formats](#supported-file-formats)
- [CLI Mode](#cli-mode)
- [Archives & Cloud Share URL Import](#archives--cloud-share-url-import)
- [Interactive SQL REPL (`-i`)](#interactive-sql-repl-i)
- [TUI Interactive Mode](#tui-interactive-mode)
- [MCP Server Mode](#mcp-server-mode)
- [`--exec` Execution Mode](#-exec-execution-mode)
- [`--run` Shell Command Mode](#-run-shell-command-mode)
- [Search Modes Explained](#search-modes-explained)
- [SQL Query Guide](#sql-query-guide)
- [File Editing](#file-editing)
- [Output Formats](#output-formats)
- [FAQ](#faq)
- [Related Documentation](#related-documentation)

---

## Quick Start

### Option 1: Download a prebuilt binary

Grab the archive for your platform from the [GitHub Releases](https://github.com/c2j/grep-excel/releases) page:

- `grep_excel-windows-x86_64.zip` — Windows
- `grep_excel-macos-x86_64.zip` — macOS (Intel)
- `grep_excel-macos-aarch64.zip` — macOS (Apple Silicon)
- `grep_excel-linux-x86_64.zip` — Linux x86_64
- `grep_excel-linux-aarch64.zip` — Linux ARM64

Extract and run:

```bash
# Linux / macOS
chmod +x grep_excel
./grep_excel --help

# Windows
grep_excel.exe --help
```

### Option 2: Build from source

```bash
git clone https://github.com/c2j/grep-excel.git
cd grep-excel

# Quick build (in-memory engine)
cargo build --release

# Recommended: full-featured build
cargo build --release --features full
```

The binary is placed at `target/release/grep_excel`.

---

## Supported File Formats

| Extension | Description |
|-----------|-------------|
| `.xlsx` / `.xls` / `.xlsm` / `.xlsb` / `.ods` | Excel / OpenDocument spreadsheets |
| `.csv` | Comma-separated values |
| `.tsv` / `.tab` | Tab-separated values |
| `.html` / `.htm` | HTML tables; each `<table>` is imported as a sheet; encoding auto-detected (UTF-8 / `<meta charset>` / CJK fallback) |
| `.txt` | Plain-text tables (extracted via section / dash-separator / column-alignment heuristics) |
| `.md` / `.markdown` | GFM Markdown pipe tables (`\| col \|`) |
| `.dbf` | dBase database files |
| `.xml` | XML data files (flat convention: repeated sibling elements under the root become rows; their child tags become columns) |
| `.docx` | Word documents; tables extracted from `word/document.xml`, sheet names derived from the heading paragraph preceding each table, merged cells forward-filled automatically; **read-only**, no editing |
| `.pptx` | PowerPoint presentations; each slide's tables are imported as a sheet, merged cells auto-filled; **read-only**, no editing |
| `.zip` / `.tar` / `.tar.gz` / `.tgz` / `.tar.bz2` / `.tar.xz` / `.tar.zst` | Archives; all recognizable table files inside are extracted and imported (see [Archives & Cloud Share URL Import](#archives--cloud-share-url-import)) |
| `.zip.001` / `.zip.002` | Multi-volume split ZIP archives |

All other formats work the same way as Excel:

```bash
# Search an HTML report (e.g. openGauss WDR)
grep_excel report.html -q "CPU"

# Run SQL against a Markdown table
grep_excel awr.md -x "SELECT * FROM sheet_1_0 LIMIT 10"

# List tables extracted from a text file
grep_excel data.txt -t

# Tables inside a Word document
grep_excel report.docx -q "预算"

# PowerPoint: one sheet per slide
grep_excel slides.pptx -t

# dBase / XML data files
grep_excel legacy.dbf -q "Smith"
grep_excel data.xml -x "SELECT * FROM data LIMIT 10"
```

> Run `grep_excel --help` to see the format list and option descriptions for your installed version (the output switches between Chinese and English based on your `LANG` setting).

---

## CLI Mode

### Basic Search

```bash
# Full-text search (case-insensitive substring match)
grep_excel data.xlsx -q "关键词"

# Exact search (case-sensitive, full-cell match)
grep_excel data.xlsx -q "Engineering" -m exact

# Wildcard search (% matches any characters, _ matches a single character)
grep_excel data.xlsx -q "张%" -m wildcard

# Regex search (supports multi-keyword OR with |)
grep_excel data.xlsx -q "张三|李四" -m regex
```

### Search Filters

```bash
# Search only a specific column
grep_excel data.xlsx -q "关键词" -c "姓名"

# Search only a specific sheet
grep_excel data.xlsx -q "关键词" -s "Sheet1"

# Invert match: find rows that do NOT contain the query
grep_excel data.xlsx -q "已删除" -v
```

### SQL Queries

```bash
# Basic query
grep_excel data.xlsx -x "SELECT * FROM sheet_1_0 LIMIT 10"

# Aggregation
grep_excel data.xlsx -x "SELECT 部门, COUNT(*) as 人数 FROM sheet_1_0 GROUP BY 部门"

# Use a friendly alias (run --list-tables first to see available names)
grep_excel data.xlsx -t
grep_excel data.xlsx -x "SELECT * FROM data.Sheet1 WHERE 年龄 > 30"
```

### Aggregate Statistics

```bash
# Count distinct value distribution in a column among matched rows
grep_excel data.xlsx -q "工程部" -g 职位

# Example output:
# Aggregate column '职位': 工程师 (15), 经理 (3), 实习生 (8)
```

### Exporting Results

```bash
# Export to CSV
grep_excel data.xlsx -q "关键词" -e results.csv

# Specify an output format
grep_excel data.xlsx -q "关键词" -f json
grep_excel data.xlsx -q "关键词" -f pretty
grep_excel data.xlsx -q "关键词" -f simple  # TSV format
```

### Repairing Corrupted Files

```bash
# Auto-repair and import
grep_excel corrupted.xlsx -q "数据" -r
```

### Listing Imported Tables

```bash
# List all tables and their columns
grep_excel data.xlsx employees.xlsx -t

# Example output:
# Available tables:
#   data.Sheet1 → sheet_1_0 (150 rows) [姓名, 部门, 职位]
#   employees.Sheet1 → sheet_2_0 (200 rows) [姓名, 工号, 薪资]
```

### Forcing a Format (`--as`)

When a file extension is missing or misleading, use `--as` to explicitly specify the parsing format. `--as` is a **sticky** option: it applies to every file that follows on the command line until the next `--as`. Files not preceded by any `--as` are still auto-detected by extension.

```bash
# Parse access.log as CSV, dump.dat as Excel
grep_excel --as csv access.log --as excel dump.dat -t

# File with no extension
grep_excel --as tsv exported_data -q "关键词"
```

Valid values: `csv`, `tsv`, `html`, `txt`, `md`, `dbf`, `xml`, `excel`, `docx`, `pptx`, `pdf`, `parquet`.

---

## Archives & Cloud Share URL Import

### Archive Files

Pass an archive file directly and grep-excel automatically extracts every recognizable table file inside, importing each one as a separate entry named `archive::path/filename`:

```bash
# Search table files inside a ZIP
grep_excel audit_2026.zip -q "异常交易"

# Query a CSV inside a tar.gz
grep_excel db_dump.tar.gz -x "SELECT * FROM sheet_1_0 LIMIT 10"

# Multi-volume ZIP (pass the first volume)
grep_excel big_data.zip.001 -t
```

Supported archive formats: `.zip`, `.tar`, `.tar.gz` / `.tgz`, `.tar.bz2`, `.tar.xz`, `.tar.zst`, and split `.zip.001` / `.zip.002`.

> Requires the `archive-support` feature (included in `--features full`).

### Cloud Share URL Import

Pass a Kingsoft Docs / WPS (`kdocs.cn`) share link directly; grep-excel downloads the file using your session cookie:

```bash
export KDOCS_COOKIE='wps_sid=...; ...'
grep_excel 'https://www.kdocs.cn/l/xxxx' -q "关键词"

# Or pass the cookie inline
grep_excel --kdocs-cookie "$KDOCS_COOKIE" 'https://www.kdocs.cn/l/xxxx' -t
```

For enterprise domains: set the `SHARE_HOSTS` environment variable (comma-separated), or use the `--share-hosts` option.

> Requires the `share-url` feature (included in `--features full`).

---

## Interactive SQL REPL (`-i`)

The `-i` / `--interactive` flag launches a rustyline-backed multi-line SQL shell — ideal when you need to run several queries in succession.

### Launching

```bash
# Pre-import files, then enter the REPL
grep_excel data.xlsx employees.xlsx -i

# You can also start with no files and use --exec or .tables to inspect state
grep_excel -i
```

### Basic Usage

The REPL uses `$` as the primary prompt. Type a SQL statement and end it with `;` to execute:

```
$ SELECT * FROM sheet_1_0 LIMIT 5;
$ SELECT 部门, COUNT(*) FROM sheet_1_0 GROUP BY 部门;
```

**Multi-line input**: any input not terminated by `;` automatically enters continuation mode (the prompt changes to `>`), letting you spread a long query across multiple lines:

```
$ SELECT 姓名, 部门
> FROM sheet_1_0
> WHERE 薪资 > 10000
> ORDER BY 薪资 DESC;
```

### Dot-Commands

The REPL supports the following meta-commands, all prefixed with `.`:

| Command | Action |
|---------|--------|
| `.tables` / `.schema` | List imported tables with their friendly aliases and columns |
| `.files` | List imported files |
| `.output <file>` | Redirect all subsequent SQL results to a file (CSV); the terminal no longer prints tables |
| `.output` | Close redirection and restore terminal output |
| `.save <file> [fmt]` | Save the **last** SQL result to a file; `fmt` can be `csv` (default), `json`, `tsv`, or `table` |
| `.let <name> AS <sql>` | Materialize a SQL query as a named session temp table (e.g. `.let t AS SELECT City, COUNT(*) FROM sheet_1_0 GROUP BY City`) |
| `.drop <name>` | Drop a temp table previously created via `.let` |
| `.help` | Show help |
| `.history` | Show command history |
| `.clear` / `.cls` | Clear the screen |
| `.exit` / `.quit` | Exit the REPL |

Export examples:

```
$ SELECT * FROM sheet_1_0 WHERE 部门 = '工程部';
$ .save eng.csv
$ .save eng.json json
$ .output full.csv
$ SELECT * FROM sheet_1_0;
$ .output
```

Temp table example:

```
$ .let t AS SELECT 部门, COUNT(*) AS n FROM sheet_1_0 GROUP BY 部门
$ SELECT * FROM t WHERE n > 5;
$ .drop t
```

> If the last result was truncated for terminal display, `.save` will suggest using `.output <file>` instead to export the full data set (file output is never row-limited).

### Exiting

- Type `.exit` or `.quit`
- Press `Ctrl+D` (EOF)
- Pressing `Ctrl+C` does **not** exit — it only cancels the current input line

### Result Display

Query results render as Unicode-aligned tables with auto-fit column widths (truncated at 40 characters max) and a row-count summary:

```
  姓名   │ 部门    │ 薪资
  ────────┼─────────┼──────────
  张三    │ 工程部  │ 15000
  李四    │ 市场部  │ 12000

  2 rows (3ms)
```

> **Tip**: The REPL displays at most ~1000 rows per query in the terminal; use `.output` to export the complete result set. Command history persists across sessions at `~/.local/state/grep-excel/history.txt` (macOS: `~/Library/Application Support/grep-excel/history.txt`), capped at 500 entries. Use the up/down arrow keys to recall inputs from previous sessions. Pass `--no-history` to disable persistence for the current session.

---

## TUI Interactive Mode

Run `grep_excel` without any query arguments to enter the TUI:

```bash
grep_excel
```

Launch with files:

```bash
grep_excel data.xlsx employees.xlsx
```

### Interface Overview

```
┌─────────────────────────────────────────────────┐
│ grep-excel │ [Normal] │ 2 files                 │ ← Title bar
├─────────────────────────────────────────────────┤
│ All(2) │ data:员工 │ emp:Sheet1                 │ ← Tabs (file:sheet when multi-file)
├─────────────────────────────────────────────────┤
│ [search________] [fulltext] [column___] [agg__] │ ← Search bar
├─────────────────────────────────────────────────┤
│ │ Source      │ Name │ Dept    │ Title  │       │ ← Results (All tab uses a Source column)
│ │ data:员工   │ 张三 │ 工程部  │ 工程师 │       │
│ │ data:员工   │ 李四 │ 工程部  │ 经理   │       │
├─────────────────────────────────────────────────┤
│ 2 matches / 150 rows, 0.05s                     │ ← Status bar
├─────────────────────────────────────────────────┤
│ /search c:column Tab:mode o:open ?:help s:export│ ← Hint bar
└─────────────────────────────────────────────────┘
```

### Auto-browse

When you launch with files (or import via `o`), the TUI **automatically loads and displays the first sheet's data** — no need to type a search term first. You can browse, scroll, then press `/` to search or `S` to run SQL.

### Keybindings

#### Navigation

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down one row |
| `k` / `↑` | Move up one row |
| `g` | Jump to top |
| `G` | Jump to bottom |
| `←` / `→` | Scroll columns left / right |
| `H` / `L` | Scroll columns left / right (vim-style) |
| `[` / `]` | Previous / next sheet (browse mode; across files) |
| `Ctrl+←` / `Ctrl+→` | Switch sheet within the current file (works in browse / flat / table views) |
| `Ctrl+↑` / `Ctrl+↓` | Switch file |
| `1`–`9` | Switch tab (in browse mode, jump to the Nth sheet) |

#### Search

| Key | Action |
|-----|--------|
| `/` or `e` | Enter a search query |
| `c` | Set a column filter |
| `a` | Set an aggregate column |
| `Tab` | Cycle search modes (fulltext → exact → wildcard → regex) |
| `Enter` | Execute search |
| `n` | Load more results (when search is truncated; in browse mode loads 500 more rows) |

#### Files & Data

| Key | Action |
|-----|--------|
| `o` | Open the file picker / view loaded files |
| `s` | Export current results to CSV |
| `d` | Clear all data |
| `v` | Toggle flat / table view |
| `Enter` | Open / close the detail panel |

#### SQL Mode

| Key | Action |
|-----|--------|
| `S` | Enter SQL query mode |

#### Other

| Key | Action |
|-----|--------|
| `?` | Show help (includes Ctrl+arrow explanations) |
| `q` | Quit |

### Running SQL (inside the TUI)

1. Press `S` to enter SQL mode (the search bar becomes a SQL input field)
2. Type a query, for example:
   ```sql
   SELECT 部门, COUNT(*) as 人数 FROM sheet_1_0 GROUP BY 部门
   ```
3. Press `Enter` to execute
4. Results appear in the table view
5. Press `d` to clear SQL results and return to normal search mode

### Flat View and the Source Column

Press `v` to toggle between table view and flat view.

- **Table view**: results are shown in a table; the **All** tab uses a single **Source** column (`file:sheet`), while single-sheet tabs omit the redundant file/sheet columns
- **Flat view**: each worksheet is rendered as an independent block with a source header; use `Ctrl+arrows` to switch between files and sheets

---

## MCP Server Mode

MCP (Model Context Protocol) lets AI assistants (such as Claude and Cursor) call grep-excel's capabilities directly.

### Starting the MCP Server

```bash
grep_excel --mcp
```

### Configuring Your AI Assistant

#### Claude Desktop

Edit `claude_desktop_config.json` (location is described in the [Claude documentation](https://docs.anthropic.com/en/docs/claude-desktop)):

```json
{
  "mcpServers": {
    "grep-excel": {
      "command": "/usr/local/bin/grep_excel",
      "args": ["--mcp"]
    }
  }
}
```

#### Cursor

In Cursor, go to Settings → MCP and add:

```json
{
  "mcpServers": {
    "grep-excel": {
      "command": "/usr/local/bin/grep_excel",
      "args": ["--mcp"]
    }
  }
}
```

### Available Tools

| Tool | Description | Key Parameters |
|------|-------------|----------------|
| `import_file` | Import a tabular file (Excel/CSV/HTML/text/Markdown) | `file_path` |
| `list_files` | List imported files | none |
| `get_metadata` | Get file metadata (column names, etc.) | `file_name` (optional) |
| `get_sheet_sample` | Uniformly sample rows from a sheet | `file_name`, `sheet_name`, `sample_size` |
| `get_sheet_data` | Fetch rows with pagination | `file_name`, `sheet_name`, `start_row`, `end_row`, `columns` |
| `search` | Search (supports context lines, multi-condition filtering) | `query`, `column`, `sheet`, `mode`, `limit`, `aggregate`, `invert`, `context_lines`, `conditions` |
| `execute_sql` | Execute a SQL query | `sql`, `limit` |
| `export_query` | Execute SQL and export results to a .xlsx file | `sql`, `output_path`, `sheet_name` |
| `get_sheet_statistics` | Per-column statistics (nulls / distinct count / top values) | `file_name`, `sheet_name`, `max_top_values` |
| `materialize_query` | Save a read-only SQL result as a named session temp table for reuse in later `execute_sql` calls | `name`, `sql` |
| `drop_temp_table` | Drop a session temp table created by `materialize_query` | `name` |
| `save_as` | Save imported data to a new file | `file_name`, `output_path`, `sheet_name` |
| `save` | Overwrite the original file with current data | `file_name`, `sheet_name` |
| `update_cell` | Update a single cell | `file_name`, `sheet_name`, `row`, `column`, `value` |
| `update_cells` | Batch-update cells | `file_name`, `sheet_name`, `updates` |
| `insert_rows` | Insert rows | `file_name`, `sheet_name`, `start_row`, `rows` |
| `delete_rows` | Delete rows | `file_name`, `sheet_name`, `start_row`, `count` |
| `add_column` | Add a column | `file_name`, `sheet_name`, `column_name`, `default_value` |
| `rename_column` | Rename a column | `file_name`, `sheet_name`, `old_name`, `new_name` |

### Typical MCP Workflows

#### Exploring an unknown file

```
You: Import data.xlsx and tell me what's in it
AI:  → import_file → shows file name, sheet count, and row counts
AI:  → get_metadata → shows each sheet's column names
AI:  → get_sheet_sample → shows 5 evenly-spaced sample rows
```

#### Data analysis

```
You: Analyze the sales data — break down revenue by region
AI:  → import_file → imports sales.xlsx
AI:  → execute_sql → SELECT 地区, SUM(销售额) FROM sales.Sheet1 GROUP BY 地区
```

#### Data profiling (get_sheet_statistics)

```
You: How's the data quality in this file? Any nulls?
AI:  → import_file → imports data.xlsx
AI:  → get_sheet_statistics → null counts, distinct counts, and top 5 frequent values per column
     → "薪资 column: 3 of 200 rows null, 45 distinct values, most frequent: 10000 (15×)"
```

#### Multi-condition search with context (enhanced search)

```
You: Find records where salary > 12000 and department is Engineering, with 2 rows of context above and below
AI:  → search → query="工程部", conditions=[{column:"薪资", operator:">", value:"12000"}], context_lines=2
     → each match also returns the 2 preceding and 2 following rows
```

#### Filtered export (export_query)

```
You: Export Beijing-based high-earning employees to a new file
AI:  → execute_sql → SELECT * FROM data.Sheet1 WHERE 城市='北京' AND 薪资 > 15000
AI:  → export_query → sql="SELECT * FROM data.Sheet1 WHERE 城市='北京' AND 薪资 > 15000", output_path="beijing_high.xlsx"
     → generates a .xlsx file directly
```

#### Session temp tables for multi-step analysis

```
You: Group the employees by department, then show only departments with more than 10 people
AI:  → materialize_query → name="dept_summary", sql="SELECT 部门, COUNT(*) AS n FROM data.Sheet1 GROUP BY 部门"
AI:  → execute_sql → SELECT * FROM dept_summary WHERE n > 10
AI:  → drop_temp_table → name="dept_summary"
```

#### Edit and save

```
You: Change row 3's department from "工程部" to "研发部"
AI:  → update_cell → modifies the cell

You: Save
AI:  → save → overwrites the original file
```

---

## `--exec` Execution Mode

`--exec` lets you run MCP tools directly from the command line without starting an MCP server. It supports single commands and multi-step pipelines.

### Single Command

```bash
# Show help
grep_excel --exec help

# Import and inspect a file
grep_excel data.xlsx --exec '{"tool":"import_file","params":{"file_path":"data.xlsx"}}'

# Search
grep_excel data.xlsx --exec '{"tool":"search","params":{"query":"张三","mode":"exact"}}'

# Search with aggregation
grep_excel data.xlsx --exec '{"tool":"search","params":{"query":"工程部","aggregate":"职位"}}'

# Execute SQL
grep_excel data.xlsx --exec '{"tool":"execute_sql","params":{"sql":"SELECT * FROM sheet_1_0 LIMIT 5"}}'
```

### Multi-Step Pipeline

Pass a JSON array; commands run in order with shared state:

```bash
grep_excel --exec '[
  {"tool":"import_file","params":{"file_path":"data.xlsx"}},
  {"tool":"get_metadata","params":{}},
  {"tool":"search","params":{"query":"张三","mode":"exact"}},
  {"tool":"update_cell","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","row":0,"column":"姓名","value":"李四"}},
  {"tool":"save","params":{"file_name":"data.xlsx"}}
]'
```

### Output Format

```bash
# JSON format (default)
grep_excel data.xlsx --exec '{"tool":"list_files","params":{}}'

# Markdown table
grep_excel data.xlsx --exec '{"tool":"list_files","params":{}}' -f markdown

# Simple TSV format
grep_excel data.xlsx --exec '{"tool":"list_files","params":{}}' -f simple
```

---

## `--run` Shell Command Mode

`--run` executes an external shell command for each matching row, using `${column_name}` placeholders to reference cell values.

### Basic Usage

```bash
# Run a command for each matching row
grep_excel <file> -q <query> --run '<command>'
```

Inside the command template, `${column_name}` placeholders are automatically shell-escaped (wrapped in single quotes). `$$` produces a literal `$`.

### Examples

```bash
# Run an external analysis tool for each match
grep_excel data.xlsx -q "ERROR" -c "级别" --run './analyzer "${内容}"'

# Write command output to a new column, then export
grep_excel data.xlsx -q "TODO" -c "类型" --run './classifier "${标题}"' --run-output-column "分类" -e output.xlsx

# Combine with a SQL query
grep_excel data.xlsx --sql "SELECT 姓名, SQL FROM sheet_1_0 WHERE 类型='旧版'" --run './formatter "${SQL}"'
```

### Related Options

| Option | Description |
|--------|-------------|
| `--run-output-column` | Write the command's stdout to this column (created automatically if it doesn't exist) |
| `--export` | Export the full Excel file after processing completes (requires the `mcp-server` feature) |

> **Note**: `--run` must be combined with `--query` (`-q`) or `--sql` (`-x`). Commands execute via `sh -c`.

---

## Search Modes Explained

### fulltext — Default

Case-insensitive substring matching. Best suited for general-purpose searches.

```
Query: "john"
Matches: "John Smith", "Johnson", "JOHN", "john@example.com"
No match: "Jon"
```

### exact

Case-sensitive full-cell match. The entire cell content must equal the query exactly.

```
Query: "Engineering"
Match: "Engineering" (exact)
No match: "engineering", "Engineering Dept"
```

### wildcard

SQL `LIKE`-style pattern matching. Case-insensitive.

| Wildcard | Meaning |
|----------|---------|
| `%` | Matches any sequence of characters (including the empty string) |
| `_` | Matches exactly one character |

```
Query: "San%"  → matches "San Francisco", "San Jose", "San"
Query: "A__"   → matches "ABC", "Amy" (exactly 3 characters)
Query: "%公司"  → matches "科技有限公司", "贸易公司"
```

### regex

Regular expression matching. Case-insensitive. Supports the full Rust regex syntax.

```
Query: "张三|李四"                    → matches cells containing either keyword
Query: "\d{4}-\d{2}-\d{2}"           → matches dates like 2024-01-15
Query: "^[A-Z]{3}-\d{3}$"            → matches IDs like ABC-123
Query: "(error|warning|critical)"    → matches log levels
```

---

## SQL Query Guide

### Table Naming

Imported worksheets are stored in the database using the following conventions:

- **Internal table name**: `sheet_{file_id}_{sheet_index}` (e.g. `sheet_1_0`, `sheet_2_0`)
- **Friendly alias**: `filename.sheetname` (e.g. `data.Sheet1`, `employees.员工表`)

Use `--list-tables` or the MCP `list_files` tool to see available table names first.

### Basic Queries

```sql
-- View the first 10 rows
SELECT * FROM sheet_1_0 LIMIT 10

-- Select specific columns
SELECT 姓名, 部门 FROM sheet_1_0

-- Conditional filter
SELECT * FROM sheet_1_0 WHERE 年龄 > 30

-- Sort
SELECT * FROM sheet_1_0 ORDER BY 薪资 DESC
```

### Aggregation

```sql
-- Count
SELECT 部门, COUNT(*) as 人数 FROM sheet_1_0 GROUP BY 部门

-- Sum
SELECT 部门, SUM(薪资) as 总薪资 FROM sheet_1_0 GROUP BY 部门

-- Average
SELECT 部门, AVG(薪资) as 平均薪资 FROM sheet_1_0 GROUP BY 部门
```

### Cross-Table Queries (JOIN)

```sql
-- Import both files first, then query using friendly aliases
SELECT e.姓名, e.部门, d.部门负责人
FROM employees.Sheet1 e
JOIN departments.Sheet1 d ON e.部门 = d.部门名称
```

### DuckDB-Specific Functions

When using the DuckDB engine (`--features engine-duckdb`), you can take advantage of:

```sql
-- Case-insensitive matching
SELECT * FROM sheet_1_0 WHERE 姓名 ILIKE '%张%'

-- Regex matching
SELECT * FROM sheet_1_0 WHERE regexp_matches(邮箱, '.*@company\.com')

-- Type casting
SELECT 姓名, 薪资::INTEGER * 12 as 年薪 FROM sheet_1_0

-- Window functions
SELECT 姓名, 薪资, RANK() OVER (PARTITION BY 部门 ORDER BY 薪资 DESC) as 排名
FROM sheet_1_0
```

---

## File Editing

grep-excel can edit Excel files via MCP or `--exec` mode.

### Cell Editing

```bash
# Update a single cell
grep_excel data.xlsx --exec '{"tool":"update_cell","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","row":0,"column":"姓名","value":"张三"}}'

# Batch update
grep_excel data.xlsx --exec '{"tool":"update_cells","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","updates":[{"row":0,"column":"姓名","value":"张三"},{"row":1,"column":"部门","value":"研发部"}]}}'
```

### Row & Column Operations

```bash
# Insert a new row before row 5
grep_excel data.xlsx --exec '{"tool":"insert_rows","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","start_row":4,"rows":[["新员工","研发部","工程师"]]}}'

# Delete rows 3–5 (3 rows total)
grep_excel data.xlsx --exec '{"tool":"delete_rows","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","start_row":2,"count":3}}'

# Add a new column
grep_excel data.xlsx --exec '{"tool":"add_column","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","column_name":"状态","default_value":"在职"}}'

# Rename a column
grep_excel data.xlsx --exec '{"tool":"rename_column","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","old_name":"部门","new_name":"所属部门"}}'
```

### Saving

```bash
# Overwrite the original file
grep_excel data.xlsx --exec '{"tool":"save","params":{"file_name":"data.xlsx"}}'

# Save as a new file
grep_excel data.xlsx --exec '{"tool":"save_as","params":{"file_name":"data.xlsx","output_path":"data_modified.xlsx"}}'
```

> **Note**: `save` overwrites the original file — consider using `save_as` to keep a backup first. The `save` / `save_as` functionality requires the `mcp-server` feature flag (which includes xlsx write support).

> **Read-only formats**: `.docx` and `.pptx` support only import, search, and query export (`export_query` / `--export`). All editing and save tools (`update_cell`, `update_cells`, `insert_rows`, `delete_rows`, `add_column`, `rename_column`, `save`, `save_as`) are rejected with an error.

---

## Output Formats

The `--format` / `-f` option supports four output formats:

### markdown (default)

```
| 文件 | Sheet | 姓名 | 部门 |
| --- | --- | --- | --- |
| data | Sheet1 | 张三 | 工程部 |
| data | Sheet1 | 李四 | 市场部 |
```

### pretty

Aligned, human-readable formatting — ideal for reading directly in the terminal.

### json

Full JSON format, suitable for programmatic processing.

```json
{
  "results": [
    {
      "file_name": "data.xlsx",
      "sheet_name": "Sheet1",
      "row": ["张三", "工程部"],
      "col_names": ["姓名", "部门"]
    }
  ],
  "stats": {
    "total_matches": 2,
    "total_rows_searched": 150,
    "search_duration_ms": 12
  }
}
```

### simple

TSV (tab-separated values) format, suitable for piping into other tools.

```
文件	Sheet	姓名	部门
data	Sheet1	张三	工程部
```

---

## FAQ

### Q: What file formats are supported?

A: `.xlsx`, `.xls`, `.xlsm`, `.xlsb`, `.ods`, `.csv`, `.tsv`/`.tab`, `.html`/`.htm`, `.txt`, `.md`/`.markdown`, `.dbf`, `.xml`, `.docx`, `.pptx`, plus `.zip` / `.tar` archive families and `.zip.001` split archives. HTML and text files have encoding auto-detected; when an extension is missing or misleading, use `--as` to force a format. See [Supported File Formats](#supported-file-formats) for the full list.

### Q: How do I speed up searching large files?

A: Use the DuckDB engine (build with `--features engine-duckdb` or `--features full`). DuckDB is purpose-built for OLAP queries and handles millions of rows with sub-second response times.

### Q: How do I edit files in MCP mode?

A: After modifying data with `update_cell` / `update_cells`, you must call `save` or `save_as` to persist the changes. The `save` functionality requires the `mcp-server` feature flag.

### Q: How do I repair a corrupted xlsx file?

A: Use the `--repair` / `-r` option. grep-excel attempts to recover data at the ZIP/XML level. If a normal import fails, repair mode is triggered automatically.

### Q: Does it support Chinese (CJK) search?

A: Fully supported. grep-excel is built on UTF-8, so Chinese search works as smoothly as English. The TUI and CLI help text auto-detect your system language and display Chinese or English accordingly.

### Q: How do I switch to the Chinese interface?

A: Set the `LANG=zh_CN.UTF-8` environment variable. grep-excel detects it automatically at startup.

### Q: Does it work on Windows?

A: Yes, Windows 7 and later are supported. You need a terminal with ANSI escape-sequence support (such as Windows Terminal; the legacy `cmd.exe` is not supported).

### Q: How do I run SQL in the TUI?

A: Press `S` to enter SQL mode, type your query, and press `Enter`. Press `o` first to see available table names. After import the first sheet auto-browses; use `Ctrl+←/→` to switch sheets within a file and `Ctrl+↑/↓` to switch files.

### Q: How do I export query results from the REPL?

A: Use `.save <file> [csv|json|tsv|table]` to save the last result, or `.output <file>` to continuously write subsequent query results to CSV, then call `.output` again to restore terminal output.

### Q: What's the difference between `--exec` and `--mcp`?

A: `--exec` is a one-shot CLI execution mode (it exits after running), while `--mcp` starts a persistent MCP server that waits for an AI assistant to connect.

### Q: DuckDB engine or SQLite engine — which should I choose?

A: DuckDB is optimized for analytical queries and is ideal for aggregations, JOINs, and window functions. SQLite is lighter and fine for simple queries. The default in-memory engine has no external dependencies and is great for basic search.

### Q: How are Excel dates displayed?

A: Excel stores dates internally as serial numbers (e.g. `46188` = 2026-06-15). grep-excel **auto-detects date columns** and converts serial numbers to ISO 8601 format strings:

- Pure date columns display as `2026-06-15`
- Columns containing time display as `2026-06-15 14:30:00`
- You can search on date fragments directly — e.g. `-q 06-15` matches June 15th, `-q 2026-06` matches June 2026
- `--repair` mode applies the same date conversion

Detection works in two layers: (1) if a column already contains `Data::DateTime`-typed cells (high confidence); (2) if the column name contains date-related keywords and more than 50% of values are plain integers within the Excel serial-number range (a conservative fallback). Numeric columns (such as salary) are unaffected.

---

## Related Documentation

- [README (Chinese)](../README.zh-CN.md) — project overview and feature summary
- [User Guide (Chinese)](./UserGuide.zh-CN.md) — this guide in Chinese
- [Developer Guide (Chinese)](./DeveloperGuide.zh-CN.md) — architecture, `SearchEngine` trait, and MCP development
- [Contributing (Chinese)](../CONTRIBUTING.zh-CN.md) — development process, features, and PR checklist
