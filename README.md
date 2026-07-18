# grep-excel

[English](#english) | [中文](#chinese)

---

<a id="english"></a>

TUI tool for searching tabular data files (Excel/CSV/TSV/HTML/text/Markdown/Word/PowerPoint/DBF/XML) with DuckDB-powered performance.

grep-excel provides a fast, interactive terminal interface for searching across multiple spreadsheet and table files. It uses DuckDB as the query engine for high-performance full-text search, exact matching, wildcard pattern matching, regex search, and SQL queries. Supports CLI one-shot queries, TUI interactive mode, MCP server integration with AI assistants, and batch `--exec` command pipelines.

## Features

- **Multiple Search Modes** — Full-text (case-insensitive substring), Exact (case-sensitive), Wildcard (SQL LIKE `%` / `_`), Regex (multi-keyword OR with `|`)
- **SQL Queries** — Execute `SELECT` statements directly against imported data with DuckDB analytical functions
- **Multi-Engine Backend** — DuckDB (high-performance OLAP), SQLite, or pure in-memory engine; select via feature flags
- **TUI Interactive Mode** — Keyboard-driven terminal interface with ratatui, auto-browse on import, tabbed results, detail panel, flat/table views, Ctrl+arrow file/sheet navigation
- **HTML / Text / Markdown Tables** — Import HTML reports (e.g. WDR/AWR), plain-text tables, and GFM Markdown pipe tables as queryable sheets; encoding auto-detected (UTF-8 / meta charset / CJK fallback)
- **Word / PowerPoint / DBF / XML** — Extract tables from `.docx` documents (sheet names derived from heading paragraphs, merged cells forward-filled) and `.pptx` slides (one sheet per slide); import dBase `.dbf` and flat `.xml` files as queryable sheets; use `--as` to force a format when the extension is missing or misleading
- **MCP Server Mode** — Integrate with AI assistants (Claude, Cursor) via 17 MCP tools for search, data exploration, statistics, editing, and export
- **Interactive SQL REPL** — Multi-line SQL shell with `;`-terminated input, command history, and dot-commands (`.tables`, `.files`, `.output`, `.save`, `.help`); launch with `-i`
- **CLI `--exec` Pipeline** — Execute MCP tools from the command line as single commands or multi-step JSON arrays
- **File Editing** — Update cells, insert/delete rows, add/rename columns, save back to original or export as new file (spreadsheet formats; `.docx`/`.pptx` are read-only)
- **Aggregate Statistics** — Count distinct value distributions in matched rows by column
- **Repair Damaged Files** — Recover data from corrupted `.xlsx` files at the ZIP/XML level (`--repair`)
- **Cloud Share URL Import** — Pass Kingsoft Docs / WPS (`kdocs.cn`) share links directly; downloads via session cookie. Use `--kdocs-cookie` or `KDOCS_COOKIE` env var. For enterprise domains, set `SHARE_HOSTS` env var
- **Archive Support** — Import table files directly from `.zip`, `.tar`, `.tar.gz`, `.tar.bz2`, `.tar.xz` archives and `.zip.001` split volumes without manual decompression
- **Excel Date Auto-Detection** — Detects Excel date serial numbers and converts them to readable `YYYYMMDD` strings, so date-based searches (`-q 0615`) work correctly
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
| `mcp-server` | MCP server mode for AI assistant integration |
| `rust_xlsxwriter` | xlsx write support for `save_as` / `save` tools |
| `archive-support` | Archive support: `.zip`, `.tar`, `.tar.gz`, `.tar.bz2`, `.tar.xz`, split `.zip.001` |
| `full` | Everything: memory engine + file dialog + MCP server + share URL + archive support |

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
| `--as` | — | Sticky per-file format override: applies to all following files until the next `--as`. Valid: `csv`, `tsv`, `html`, `txt`, `md`, `dbf`, `xml`, `excel`, `docx`, `pptx` |

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

Available tools for `--exec`: `import_file`, `list_files`, `get_metadata`, `get_sheet_sample`, `get_sheet_data`, `get_sheet_statistics`, `search`, `execute_sql`, `export_query`, `save_as`, `save`, `update_cell`, `update_cells`, `insert_rows`, `delete_rows`, `add_column`, `rename_column`.

## MCP Server Mode

Start the MCP server for integration with AI assistants (e.g., Claude, Cursor):

```bash
grep_excel --mcp
```

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `import_file` | Import a tabular file (Excel/CSV/TSV/HTML/text/Markdown/docx/pptx/DBF/XML) |
| `list_files` | List imported files and their sheets |
| `get_metadata` | Get detailed metadata: sheet names, columns per sheet |
| `get_sheet_sample` | **Preview** a sheet: get N evenly-spaced rows (default 10). Fastest way to understand structure without loading all rows |
| `get_sheet_data` | Get rows from a sheet with pagination (`start_row`/`end_row` as numbers) and column filtering |
| `search` | Search with fulltext/exact/wildcard/regex + aggregation + context lines + multi-condition AND filtering |
| `execute_sql` | Execute a raw SQL `SELECT` query |
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
- `.zip` — ZIP archives; table files inside are automatically extracted and imported
- `.tar` / `.tar.gz` / `.tgz` / `.tar.bz2` / `.tar.xz` / `.tar.zst` — TAR archives (compressed or uncompressed)
- `.zip.001` / `.zip.002` — Multi-volume split ZIP archives

Archives are handled transparently: pass a `.zip` or `.tar.gz` directly and all recognizable table files inside are extracted and imported as separate entries with `archive::path/file.xlsx` naming.

## License

MIT License — see [LICENSE](LICENSE) for details.

---

<a id="chinese"></a>

# grep-excel（中文说明）

基于 DuckDB 的多格式表格文件搜索 TUI 工具（Excel/CSV/TSV/HTML/文本/Markdown/Word/PowerPoint/DBF/XML）。

grep-excel 提供快速的交互式终端界面，用于在多个电子表格与表格文件中进行搜索。使用 DuckDB 作为查询引擎，支持全文搜索、精确匹配、通配符模式匹配、正则搜索和 SQL 查询。支持 CLI 命令行查询、TUI 交互模式、MCP 服务器与 AI 助手集成，以及批量 `--exec` 命令流水线。

## 功能特性

- **多种搜索模式** — 全文（不区分大小写子串）、精确（区分大小写）、通配符（SQL LIKE `%` / `_`）、正则（多关键词 OR 用 `|`）
- **SQL 查询** — 直接对导入的数据执行 `SELECT` 语句，支持 DuckDB 分析函数
- **多引擎后端** — DuckDB（高性能 OLAP）、SQLite 或纯内存引擎；通过 feature flag 选择
- **TUI 交互模式** — 键盘驱动的终端界面，导入后自动浏览、选项卡结果、详情面板、平铺/表格视图，支持 Ctrl+方向键切换文件/Sheet
- **HTML / 文本 / Markdown 表格** — 导入 HTML 报告（如 WDR/AWR）、纯文本表格与 GFM Markdown 管道表为可查询 sheet；自动检测编码（UTF-8 / meta charset / CJK 回退）
- **Word / PowerPoint / DBF / XML** — 提取 `.docx` 文档表格（sheet 名取自标题段落，合并单元格自动前向填充）与 `.pptx` 幻灯片表格（每页一个 sheet）；导入 dBase `.dbf` 与扁平 `.xml` 数据为可查询 sheet；扩展名缺失或误导时可用 `--as` 强制指定格式
- **MCP 服务器模式** — 通过 17 个 MCP 工具与 AI 助手（Claude、Cursor）集成，支持搜索、数据探索、统计分析、编辑和导出
- **交互式 SQL REPL** — 多行 SQL 交互式 shell，支持 `;` 结束输入、命令历史和点命令（`.tables`、`.files`、`.output`、`.save`、`.help`）；用 `-i` 启动
- **CLI `--exec` 流水线** — 在命令行中以单条命令或多步 JSON 数组执行 MCP 工具
- **文件编辑** — 更新单元格、插入/删除行、添加/重命名列、保存回原文件或导出为新文件（电子表格格式；`.docx`/`.pptx` 为只读）
- **聚合统计** — 对匹配结果按列统计不同值的分布
- **修复损坏文件** — 在 ZIP/XML 层面从损坏的 `.xlsx` 文件中恢复数据（`--repair`）
- **云文档链接导入** — 直接传入金山文档 (kdocs.cn) 分享链接；通过登录 Cookie 下载。使用 `--kdocs-cookie` 或 `KDOCS_COOKIE` 环境变量。企业版域名请设置 `SHARE_HOSTS` 环境变量
- **归档文件支持** — 直接导入 `.zip`、`.tar`、`.tar.gz`、`.tar.bz2`、`.tar.xz` 归档文件及 `.zip.001` 分卷压缩包，无需手动解压
- **Excel 日期自动识别** — 自动检测 Excel 日期序列号并转换为可读的 `YYYYMMDD` 字符串，确保基于日期的搜索（`-q 0615`）能正确命中
- **多种输出格式** — Markdown 表格、美化打印、JSON 和简单 TSV（`--format`）
- **CSV 导出** — 将搜索或 SQL 结果导出为 CSV 文件
- **友好表别名** — 在 SQL 中使用 `文件名.工作表名` 语法替代内部 `sheet_N_M` 名称
- **国际化 / 中文支持** — 从 `LANG`/`LC_ALL` 环境变量自动检测语言；TUI、CLI 和帮助文本均支持中英双语
- **跨平台** — Windows（x86_64，Win7+）、macOS（Intel x86_64、Apple Silicon ARM64）、Linux（x86_64 glibc 2.31+、ARM64）

## 安装

### 下载预编译二进制文件

从 [GitHub Releases 页面](https://github.com/c2j/grep-excel/releases) 下载适用于您平台的最新版本。

支持的平台：
- Windows (x86_64)
- macOS (Intel x86_64, Apple Silicon ARM64)
- Linux (x86_64, ARM64)

### 从源码构建

要求：Rust 1.70+ 和 Cargo

```bash
git clone https://github.com/c2j/grep-excel.git
cd grep-excel

# 默认构建（内存引擎 + 文件对话框）
cargo build --release

# 使用 DuckDB 引擎（推荐用于生产环境）
cargo build --release --features full

# DuckDB 捆绑构建（独立运行，无需系统 DuckDB）
cargo build --release --features duckdb-bundled

# 使用 SQLite 引擎
cargo build --release --features engine-sqlite

# 无头构建（无文件对话框）
cargo build --release --no-default-features --features engine-memory
```

> **提示：** 使用 DuckDB 功能构建时，设置 `DUCKDB_DOWNLOAD_LIB=1` 可下载预编译库而非从源码编译，大幅加快构建速度。

### Feature Flags（功能标志）

| 功能标志 | 说明 |
|---------|------|
| `engine-memory` | 内存引擎（默认） |
| `engine-duckdb` | DuckDB 引擎（高性能） |
| `engine-sqlite` | SQLite 引擎 |
| `duckdb-bundled` | DuckDB 捆绑 C 库（独立运行） |
| `file-dialog` | 原生文件选择对话框（默认） |
| `mcp-server` | MCP 服务器模式（AI 助手集成） |
| `rust_xlsxwriter` | xlsx 写入支持（用于 `save_as` / `save` 工具） |
| `archive-support` | 归档支持：`.zip`、`.tar`、`.tar.gz`、`.tar.bz2`、`.tar.xz`、分卷 `.zip.001` |
| `full` | 全部功能：内存引擎 + 文件对话框 + MCP 服务器 + 分享链接 + 归档支持 |

## 使用方式

```bash
grep_excel [文件...] [选项]
```

### CLI 选项

| 选项 | 缩写 | 说明 |
|------|------|------|
| `--interactive` | `-i` | 启动交互式 SQL REPL：`$` 提示符，多行输入（`;` 执行），上下方向键历史，点命令 |
| `--no-history` | — | 禁用跨会话 SQL 历史持久化（默认保存） |
| `--query` | `-q` | 搜索查询字符串 |
| `--column` | `-c` | 筛选指定列名 |
| `--sheet` | `-s` | 筛选指定工作表名称 |
| `--mode` | `-m` | 搜索模式：`fulltext`（默认）、`exact`、`wildcard`、`regex` |
| `--invert` | `-v` | 反向匹配：显示不匹配的行 |
| `--sql` | `-x` | 对导入数据执行 SQL SELECT 查询 |
| `--export` | `-e` | 将搜索结果导出为 CSV 文件 |
| `--exec` | `-E` | 以 JSON 执行 MCP 工具命令 |
| `--mcp` | — | 启动 MCP 服务器模式（stdio） |
| `--aggregate` | `-g` | 按列统计匹配行中不同值的分布 |
| `--list-tables` | `-t` | 列出已导入表及其友好名称和列名 |
| `--format` | `-f` | 输出格式：`markdown`（默认）、`pretty`、`json`、`simple`（TSV） |
| `--repair` | `-r` | 导入前尝试修复损坏的 xlsx 文件（ZIP/XML 层面） |
| `--run` | `-X` | 对每个匹配行执行 Shell 命令，`${列名}` 引用单元格值 |
| `--run-output-column` | — | 将 `--run` 命令 stdout 写入指定列（列不存在则自动创建） |
| `--help` | `-h` | 显示帮助（含支持的文件格式；自动检测中/英文） |
| `--kdocs-cookie` | — | 金山文档 (kdocs.cn) 分享链接下载专用 Cookie |
| `--share-hosts` | — | 企业版云文档分享链接的额外域名（逗号分隔） |
| `--as` | — | 按文件覆盖格式（粘性）：对后续所有文件生效，直到下一个 `--as`。可选：`csv`、`tsv`、`html`、`txt`、`md`、`dbf`、`xml`、`excel`、`docx`、`pptx` |

### 示例

搜索单个文件：
```bash
grep_excel data.xlsx -q "搜索关键词"
```

多文件搜索并筛选列：
```bash
grep_excel a.xlsx b.xlsx -c "姓名" -m exact
```

通配符搜索（`%` 匹配任意字符，`_` 匹配单个字符）：
```bash
grep_excel data.xlsx -q "张%" -m wildcard
```

正则多关键词搜索（用 `|` 表示 OR）：
```bash
grep_excel data.xlsx -q "张三|李四" -m regex
```

仅搜索指定工作表：
```bash
grep_excel data.xlsx -q "工程部" -s 员工表
```

反向匹配 — 查找不包含查询词的行：
```bash
grep_excel data.xlsx -q "工程部" -v
```

聚合统计 — 统计不同值分布：
```bash
grep_excel data.xlsx -q "工程部" -g 部门
```

列出表别名：
```bash
grep_excel data.xlsx employees.xlsx -t
```

修复并导入损坏的 xlsx：
```bash
grep_excel corrupted.xlsx -q "数据" -r
```

从金山文档分享链接导入：
```bash
export KDOCS_COOKIE='wps_sid=...; ...'
grep_excel 'https://www.kdocs.cn/l/xxxx' -q "关键词"

# 或直接传入 Cookie
grep_excel --kdocs-cookie "$KDOCS_COOKIE" 'https://www.kdocs.cn/l/xxxx' -t
```

搜索归档文件中的表格：
```bash
# 搜索 ZIP 中的 xlsx 文件
grep_excel audit_2026.zip -q "异常交易"

# 搜索 tar.gz 中的 CSV
grep_excel db_dump.tar.gz -x "SELECT * FROM sheet_1_0 LIMIT 10"

# 分卷 ZIP
grep_excel big_data.zip.001 -t
```

指定格式导出：
```bash
grep_excel data.xlsx -q "关键词" -e results.csv -f json
```

启动 TUI 模式（不带 CLI 参数）：
```bash
grep_excel
```

启动交互式 SQL REPL：
```bash
# 预导入文件后启动 REPL
grep_excel data.xlsx employees.xlsx -i

# REPL 内操作：
#   $ SELECT * FROM sheet_1_0 LIMIT 5;
#   $ .tables          # 列出已导入表
#   $ .files           # 列出已导入文件
#   $ .output out.csv  # 将后续 SQL 结果持续重定向到 CSV
#   $ .output          # 恢复终端输出
#   $ .save out.json json  # 保存上次 SQL 结果 (csv|json|tsv|table)
#   $ .help            # 显示点命令
#   $ .exit            # 退出（Ctrl+D 也可退出）
```

REPL 中输入的 SQL 和点命令会保存到历史文件（Linux：
`~/.local/state/grep-excel/history.txt`，macOS：
`~/Library/Application Support/grep-excel/history.txt`），下次启动可用上下方向键跨会话召回。传入 `--no-history` 可在本次会话中关闭。

HTML / 文本 / Markdown 表格与 Excel 用法相同：
```bash
grep_excel report.html -q "CPU"
grep_excel awr.md -x "SELECT * FROM \"Host CPU\" LIMIT 10"
grep_excel data.txt -t
```

Word、PowerPoint、DBF、XML 文件用法相同：
```bash
grep_excel report.docx -q "预算"          # 提取 word/document.xml 中的表格
grep_excel slides.pptx -t                 # 每页幻灯片一个 sheet
grep_excel legacy.dbf -q "Smith"          # dBase 数据库
grep_excel data.xml -x "SELECT * FROM data LIMIT 10"
```

扩展名缺失或误导时强制指定格式（`--as` 为粘性选项，对后续所有文件生效直到下一个 `--as`）：
```bash
grep_excel --as csv access.log --as excel dump.dat -t
```

对每个匹配行执行 Shell 命令（`--run` / `-X`）：
```bash
# 对每个匹配行执行外部工具，使用 ${列名} 替代
grep_excel data.xlsx -q "ERROR" -c "等级" --run './analyzer "${消息}"'

# 将命令输出写入新列，再导出
grep_excel data.xlsx -q "TODO" -c "类型" --run './classifier "${标题}"' --run-output-column "分类" -e output.xlsx

# 配合 --sql 使用
grep_excel data.xlsx --sql "SELECT 姓名, SQL FROM sheet_1_0 WHERE 类型='旧版'" --run './formatter "${SQL}"'
```

> `--run` 对每个匹配行执行 `sh -c`，`${列名}` 引用单元格值（自动 shell 转义），`$$` 表示字面 `$`。

### MCP 配置

在 AI 助手的 MCP 配置中添加（如 `claude_desktop_config.json` 或 Cursor MCP 设置）：

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

## 支持的文件格式

- `.xlsx` — Excel 2007+（Open XML）
- `.xls` — Excel 97-2004（BIFF8）
- `.xlsm` — Excel 启用宏
- `.xlsb` — Excel 二进制
- `.ods` — OpenDocument 电子表格
- `.csv` — 逗号分隔值
- `.tsv` / `.tab` — 制表符分隔值
- `.html` / `.htm` — HTML 表格（自动检测编码；每个 `<table>` 作为一个 sheet）
- `.txt` — 纯文本表格（章节 / 短横线分隔 / 对齐启发式）
- `.md` / `.markdown` — GFM Markdown 管道表
- `.dbf` — dBase 数据库文件
- `.xml` — XML 数据文件（扁平约定：根元素下重复的同名子元素作为行，其子标签作为列）
- `.docx` — Word 文档（从 word/document.xml 提取表格；只读，不支持编辑）
- `.pptx` — PowerPoint 演示文稿（从每张幻灯片提取表格；只读，不支持编辑）
- `.zip` — ZIP 归档文件，内部表格文件自动提取导入
- `.tar` / `.tar.gz` / `.tgz` / `.tar.bz2` / `.tar.xz` / `.tar.zst` — TAR 归档（压缩或未压缩）
- `.zip.001` / `.zip.002` — 分卷压缩 ZIP

归档文件透明处理：直接传入 `.zip` 或 `.tar.gz`，内部所有可识别的表格文件自动提取并以 `archive::path/file.xlsx` 命名导入。

## TUI 快捷键（摘要）

| 按键 | 功能 |
|------|------|
| `Ctrl+←` / `Ctrl+→` | 在同一文件内切换 Sheet |
| `Ctrl+↑` / `Ctrl+↓` | 切换文件 |
| `v` | 切换平铺/表格视图 |
| `S` | SQL 查询模式 |
| `?` | 帮助 |

导入文件后 TUI **自动浏览**首个 sheet（无需先搜索）。多文件时标签显示 `文件:sheet`；「全部」标签使用单一 **来源** 列（`文件:sheet`）。

## 许可证

MIT License — 详见 [LICENSE](LICENSE)
