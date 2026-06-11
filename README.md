# grep-excel

[English](#english) | [中文](#chinese)

---

<a id="english"></a>

TUI tool for searching Excel/CSV files with DuckDB-powered performance.

grep-excel provides a fast, interactive terminal interface for searching across multiple Excel and CSV files. It uses DuckDB as the query engine for high-performance full-text search, exact matching, wildcard pattern matching, regex search, and SQL queries. Supports CLI one-shot queries, TUI interactive mode, MCP server integration with AI assistants, and batch `--exec` command pipelines.

## Features

- **Multiple Search Modes** — Full-text (case-insensitive substring), Exact (case-sensitive), Wildcard (SQL LIKE `%` / `_`), Regex (multi-keyword OR with `|`)
- **SQL Queries** — Execute `SELECT` statements directly against imported data with DuckDB analytical functions
- **Multi-Engine Backend** — DuckDB (high-performance OLAP), SQLite, or pure in-memory engine; select via feature flags
- **TUI Interactive Mode** — Keyboard-driven terminal interface with ratatui, tabbed results, detail panel, flat/table views
- **MCP Server Mode** — Integrate with AI assistants (Claude, Cursor) via 14 MCP tools for search, data exploration, editing, and export
- **CLI `--exec` Pipeline** — Execute MCP tools from the command line as single commands or multi-step JSON arrays
- **File Editing** — Update cells, insert/delete rows, add/rename columns, save back to original or export as new file
- **Aggregate Statistics** — Count distinct value distributions in matched rows by column
- **Repair Damaged Files** — Recover data from corrupted `.xlsx` files at the ZIP/XML level (`--repair`)
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
| `full` | Everything: memory engine + file dialog + MCP server |

## Usage

```bash
grep_excel [FILES...] [OPTIONS]
```

### CLI Options

| Flag | Short | Description |
|------|-------|-------------|
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
| `--help` | `-h` | Show help (auto-detects language: Chinese or English) |

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

Export results with specific format:
```bash
grep_excel data.xlsx -q "keyword" -e results.csv -f json
```

Launch in TUI mode (no CLI arguments):
```bash
grep_excel
```

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

Available tools for `--exec`: `import_file`, `list_files`, `get_metadata`, `get_sheet_sample`, `get_sheet_data`, `search`, `execute_sql`, `save_as`, `save`, `update_cell`, `update_cells`, `insert_rows`, `delete_rows`, `add_column`, `rename_column`.

## MCP Server Mode

Start the MCP server for integration with AI assistants (e.g., Claude, Cursor):

```bash
grep_excel --mcp
```

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `import_file` | Import an Excel/CSV file |
| `list_files` | List imported files and their sheets |
| `get_metadata` | Get detailed metadata: sheet names, columns per sheet |
| `get_sheet_sample` | Get evenly-spaced sampled rows from a sheet |
| `get_sheet_data` | Get paginated row data with column filtering |
| `search` | Search with fulltext/exact/wildcard/regex + aggregation |
| `execute_sql` | Execute a raw SQL `SELECT` query |
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

## TUI Keybindings

| Key | Action |
|-----|--------|
| `q` | Quit |
| `/` | Enter search query |
| `c` | Set column filter |
| `g` | Set aggregate column |
| `S` | Enter **SQL query mode** — type a raw `SELECT` query |
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
| `v` | Toggle flat/table view |
| `←` / `→` | Scroll columns left/right |
| `1`–`9` | Switch to tab |

## Supported Formats

- `.xlsx` — Excel 2007+ (Open XML)
- `.xls` — Excel 97-2004 (BIFF8)
- `.xlsm` — Excel Macro-Enabled
- `.xlsb` — Excel Binary
- `.ods` — OpenDocument Spreadsheet
- `.csv` — Comma-Separated Values

## License

MIT License — see [LICENSE](LICENSE) for details.

---

<a id="chinese"></a>

# grep-excel（中文说明）

基于 DuckDB 的 Excel/CSV 文件搜索 TUI 工具。

grep-excel 提供快速的交互式终端界面，用于在多个 Excel 和 CSV 文件中进行搜索。使用 DuckDB 作为查询引擎，支持全文搜索、精确匹配、通配符模式匹配、正则搜索和 SQL 查询。支持 CLI 命令行查询、TUI 交互模式、MCP 服务器与 AI 助手集成，以及批量 `--exec` 命令流水线。

## 功能特性

- **多种搜索模式** — 全文（不区分大小写子串）、精确（区分大小写）、通配符（SQL LIKE `%` / `_`）、正则（多关键词 OR 用 `|`）
- **SQL 查询** — 直接对导入的数据执行 `SELECT` 语句，支持 DuckDB 分析函数
- **多引擎后端** — DuckDB（高性能 OLAP）、SQLite 或纯内存引擎；通过 feature flag 选择
- **TUI 交互模式** — 键盘驱动的终端界面，支持选项卡结果、详情面板、平铺/表格视图
- **MCP 服务器模式** — 通过 14 个 MCP 工具与 AI 助手（Claude、Cursor）集成，支持搜索、数据探索、编辑和导出
- **CLI `--exec` 流水线** — 在命令行中以单条命令或多步 JSON 数组执行 MCP 工具
- **文件编辑** — 更新单元格、插入/删除行、添加/重命名列、保存回原文件或导出为新文件
- **聚合统计** — 对匹配结果按列统计不同值的分布
- **修复损坏文件** — 在 ZIP/XML 层面从损坏的 `.xlsx` 文件中恢复数据（`--repair`）
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
| `full` | 全部功能：内存引擎 + 文件对话框 + MCP 服务器 |

## 使用方式

```bash
grep_excel [文件...] [选项]
```

### CLI 选项

| 选项 | 缩写 | 说明 |
|------|------|------|
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
| `--help` | `-h` | 显示帮助信息（自动检测中/英文） |

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

指定格式导出：
```bash
grep_excel data.xlsx -q "关键词" -e results.csv -f json
```

启动 TUI 模式（不带 CLI 参数）：
```bash
grep_excel
```

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

## 许可证

MIT License — 详见 [LICENSE](LICENSE)
