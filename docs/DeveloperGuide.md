# grep-excel 开发者指南

本文档面向需要扩展 grep-excel 功能、集成 MCP 工具或理解内部架构的开发者。

## 目录

- [架构概览](#架构概览)
- [SearchEngine Trait](#searchengine-trait)
- [添加新引擎后端](#添加新引擎后端)
- [MCP 工具开发](#mcp-工具开发)
- [类型系统](#类型系统)
- [国际化（i18n）](#国际化i18n)
- [CLI 命令扩展](#cli-命令扩展)
- [TUI 组件开发](#tui-组件开发)
- [数据流](#数据流)
- [发布流程](#发布流程)

---

## 架构概览

grep-excel 采用分层架构，核心抽象是 `SearchEngine` trait：

```
┌─────────────────────────────────────────────────────────────┐
│                        入口层 (main.rs)                      │
│  CLI 解析 → 模式路由（CLI / TUI / MCP / Exec / SQL / Run / REPL）    │
└───────┬──────────────┬──────────────┬───────────────────────┘
        │              │              │
        ▼              ▼              ▼
┌───────────┐  ┌───────────┐  ┌──────────────┐
│  CLI 模式  │  │  TUI 模式  │  │  MCP 模式     │
│ run_cli() │  │ app/mod.rs│  │ mcp.rs + rmcp │
│ run_sql() │  │ ratatui   │  │               │
│ run_exec()│  │           │  │               │
│ run_exec_shell()│        │  │               │
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

### 设计原则

1. **引擎无关**：CLI/TUI/MCP 只依赖 `SearchEngine` trait，不关心具体实现
2. **运行时选择**：引擎通过 Cargo features 在编译时选择，`DefaultEngine` 类型别名指向当前启用的引擎
3. **共享类型**：所有数据结构定义在 `types.rs`，被所有层共享
4. **异步 TUI**：TUI 通过事件通道（`event.rs`）与引擎交互，避免阻塞 UI 线程

### 关键依赖

| 依赖 | 用途 |
|------|------|
| `ratatui` + `crossterm` | 终端 UI 框架 |
| `calamine` | Excel 文件解析（xlsx, xls, xlsb, ods） |
| `csv` | CSV 文件解析 |
| `duckdb` (可选) | DuckDB 数据库引擎 |
| `rusqlite` (可选) | SQLite 数据库引擎 |
| `rmcp` (可选) | MCP 协议服务器实现 |
| `rust_xlsxwriter` (可选) | xlsx 文件写入 |
| `clap` | CLI 参数解析 |
| `regex` | 正则表达式匹配 |
| `serde` + `serde_json` | 序列化/反序列化 |
| `schemars` (可选) | JSON Schema 生成（用于 MCP 工具描述） |

---

## SearchEngine Trait

`SearchEngine` trait 定义在 `src/engine/mod.rs`，是所有数据库后端的统一接口。

### Trait 定义

```rust
pub trait SearchEngine: Send {
    fn new() -> Result<Self> where Self: Sized;

    // 文件导入
    fn import_excel(&mut self, path: &Path, progress: &dyn Fn(usize, usize)) -> Result<FileInfo>;
    fn import_excel_repair(&mut self, path: &Path, progress: &dyn Fn(usize, usize)) -> Result<FileInfo>;

    // 搜索
    fn search(&self, query: &SearchQuery) -> Result<(Vec<SearchResult>, SearchStats)>;

    // SQL
    fn execute_sql(&self, sql: &str, limit: usize) -> Result<SqlResult>;

    // 元数据
    fn list_files(&self) -> Vec<FileInfo>;
    fn list_table_aliases(&self) -> Vec<TableAliasInfo>;
    fn get_metadata(&self, file_name: &str) -> Result<FileMetadataInfo>;
    fn get_sheet_sample(&self, file_name: &str, sheet_name: &str, sample_size: usize) -> Result<SheetDataResult>;
    fn get_sheet_data(&self, file_name: &str, sheet_name: &str, start_row: Option<usize>, end_row: Option<usize>, columns: Option<&[String]>) -> Result<SheetDataResult>;
    fn get_sheet_statistics(&self, file_name: &str, sheet_name: &str, max_top_values: usize) -> Result<SheetStatistics>;

    // 编辑
    fn update_cell(&mut self, file_name: &str, sheet_name: &str, row: usize, column: &str, value: &str) -> Result<()>;
    fn update_cells(&mut self, file_name: &str, sheet_name: &str, updates: &[(usize, String, String)]) -> Result<usize>;
    fn insert_rows(&mut self, file_name: &str, sheet_name: &str, start_row: usize, rows: Vec<Vec<String>>) -> Result<()>;
    fn delete_rows(&mut self, file_name: &str, sheet_name: &str, start_row: usize, count: usize) -> Result<usize>;
    fn add_column(&mut self, file_name: &str, sheet_name: &str, column_name: &str, default_value: &str) -> Result<()>;
    fn rename_column(&mut self, file_name: &str, sheet_name: &str, old_name: &str, new_name: &str) -> Result<()>;

    // 导出
    fn save_as(&self, file_name: &str, output_path: &Path) -> Result<()>;

    // 清理
    fn clear(&mut self) -> Result<()>;
}
```

### DefaultEngine 类型别名

`DefaultEngine` 根据编译时 feature flags 自动选择：

```rust
// src/engine/mod.rs（实际代码中通过 cfg 实现）

#[cfg(feature = "engine-duckdb")]
pub type DefaultEngine = duckdb::DuckDbEngine;

#[cfg(all(feature = "engine-sqlite", not(feature = "engine-duckdb")))]
pub type DefaultEngine = sqlite::SqliteEngine;

#[cfg(not(any(feature = "engine-duckdb", feature = "engine-sqlite")))]
pub type DefaultEngine = memory::MemoryEngine;
```

优先级：DuckDB > SQLite > Memory。

---

## 添加新引擎后端

要添加一个新的数据库后端（例如 PostgreSQL、ClickHouse），需要：

### 步骤 1：创建引擎文件

在 `src/engine/` 下创建新的引擎文件，例如 `postgres.rs`。

### 步骤 2：实现 SearchEngine trait

```rust
// src/engine/postgres.rs

use crate::engine::SearchEngine;
use crate::types::*;
use anyhow::Result;
use std::path::Path;

pub struct PostgresEngine {
    // 引擎状态
}

impl SearchEngine for PostgresEngine {
    fn new() -> Result<Self> {
        // 初始化连接
        todo!()
    }

    fn import_excel(&mut self, path: &Path, progress: &dyn Fn(usize, usize)) -> Result<FileInfo> {
        // 解析 Excel → 导入 PostgreSQL
        todo!()
    }

    fn search(&self, query: &SearchQuery) -> Result<(Vec<SearchResult>, SearchStats)> {
        // 执行搜索
        todo!()
    }

    fn execute_sql(&self, sql: &str, limit: usize) -> Result<SqlResult> {
        // 执行 SQL
        todo!()
    }

    fn list_files(&self) -> Vec<FileInfo> {
        todo!()
    }

    // ... 实现其余方法
}
```

### 步骤 3：注册到 Cargo.toml

```toml
[features]
engine-postgres = ["dep:postgres"]

[dependencies]
postgres = { version = "0.19", optional = true }
```

### 步骤 4：更新 DefaultEngine 选择逻辑

在 `src/engine/mod.rs` 中添加条件编译：

```rust
#[cfg(feature = "engine-postgres")]
mod postgres;

// 更新优先级
#[cfg(feature = "engine-postgres")]
pub type DefaultEngine = postgres::PostgresEngine;
```

### 步骤 5：更新 CI

在 `.github/workflows/release.yml` 中添加 PostgreSQL 引擎的构建矩阵项。

---

## MCP 工具开发

### 架构

MCP 服务器使用 `rmcp` crate 实现，运行在 stdio 传输上。

```
AI 助手 (Claude/Cursor)
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
│  ├── ... (17 tools total)   │
└──────────┬──────────────────┘
           │
           ▼
┌─────────────────────────────┐
│  SearchEngine trait         │
│  (Arc<RwLock<SyncDb>>)      │
└─────────────────────────────┘
```

### 添加新 MCP 工具

**步骤 1：定义参数结构体**（在 `src/types.rs`）

```rust
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct MyNewToolParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "参数说明"))]
    pub param1: String,
    pub param2: Option<usize>,
}
```

**步骤 2：在 SearchEngine trait 中添加方法**（如果涉及新引擎功能）

```rust
// src/engine/mod.rs
fn my_new_operation(&mut self, param1: &str, param2: Option<usize>) -> Result<String>;
```

**步骤 3：在所有引擎实现中添加方法**

```rust
// src/engine/memory.rs, duckdb.rs, sqlite.rs
fn my_new_operation(&mut self, param1: &str, param2: Option<usize>) -> Result<String> {
    // 实现
    todo!()
}
```

**步骤 4：在 MCP 服务器中添加 `#[tool]` 方法**（在 `src/mcp.rs`）

```rust
#[tool(description = "工具描述")]
pub async fn my_new_tool(
    &self,
    Parameters(params): Parameters<MyNewToolParams>,
) -> Result<String, String> {
    let db = Arc::clone(&self.db);
    tokio::task::spawn_blocking(move || {
        let mut guard = db.write();
        guard.0.my_new_operation(&params.param1, params.param2)
            .map(|result| serde_json::to_string_pretty(&result).unwrap())
            .map_err(|e| format!("操作失败: {}", e))
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}
```

**步骤 5：在 CLI `--exec` 中添加 dispatch**（在 `src/main.rs`）

```rust
// exec_dispatch() 函数中
"my_new_tool" => {
    let p: MyNewToolParams = serde_json::from_value(params.clone())?;
    let result = db.my_new_operation(&p.param1, p.param2)?;
    Ok(serde_json::to_string_pretty(&result)?)
}
```

**步骤 6：更新 `--exec help` 输出**（在 `print_exec_help()` 函数中）

### MCP 服务器的线程安全

MCP 服务器使用 `Arc<RwLock<SyncDb>>` 来安全地在多个异步任务间共享数据库状态：

```rust
pub struct GrepExcelServer {
    db: Arc<RwLock<SyncDb>>,
    import_paths: Arc<RwLock<HashMap<String, String>>>,
}
```

- **读操作**（search, execute_sql, get_metadata 等）使用 `db.read()`
- **写操作**（update_cell, insert_rows 等）使用 `db.write()`
- **阻塞操作**通过 `tokio::task::spawn_blocking` 在专用线程池执行

### JSON Schema 生成

使用 `schemars` 自动为 MCP 参数生成 JSON Schema：

```rust
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct SearchParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "搜索查询字符串"))]
    pub query: String,
    // ...
}
```

`schemars` 仅在 `mcp-server` feature 启用时编译，不影响其他构建。

---

## 类型系统

所有核心类型定义在 `src/types.rs`：

### 搜索相关

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

### 文件与元数据

```rust
pub struct FileInfo {
    pub name: String,
    pub sheets: Vec<(String, usize)>,     // (sheet名, 行数)
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

### MCP / Exec 参数类型

所有以 `Params` 结尾的结构体同时用于 MCP 和 `--exec` 模式：

- `ImportFileParams`
- `SearchParams`（含 `context_lines`、`conditions: Vec<SearchCondition>`）
- `SqlQueryParams`
- `ExportQueryParams`（`sql`, `output_path`, `sheet_name`）
- `GetMetadataParams`
- `GetSheetSampleParams`
- `GetSheetDataParams`
- `GetSheetStatisticsParams`（`file_name`, `sheet_name`, `max_top_values`）
- `SaveAsParams`
- `SaveParams`
- `UpdateCellParams`
- `UpdateCellsParams`
- `InsertRowsParams`
- `DeleteRowsParams`
- `AddColumnParams`
- `RenameColumnParams`

`SearchParams.conditions` 中的每个条件是 `SearchCondition { column, operator, value }`，支持操作符 `=`, `!=`, `ILIKE`, `LIKE`, `>`, `<`, `>=`, `<=`，条件间为 AND 关系。

这些结构体使用 `#[derive(Deserialize)]` 从 JSON 反序列化，MCP 构建时额外使用 `#[derive(schemars::JsonSchema)]` 生成 Schema。

---

## 国际化（i18n）

`src/i18n.rs` 提供中英双语文案支持。所有面向用户的文本必须通过此模块。

### 语言检测

启动时自动从环境变量检测语言：

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

### 添加新文案

1. 在 `i18n.rs` 中添加一个新的函数：

```rust
pub fn my_new_label() -> &'static str {
    match current() {
        Lang::Zh => "中文文案",
        Lang::En => "English text",
    }
}

// 如果需要格式化参数：
pub fn my_new_message(param: &str) -> String {
    match current() {
        Lang::Zh => format!("中文: {}", param),
        Lang::En => format!("English: {}", param),
    }
}
```

2. 在代码中使用：

```rust
use grep_excel::i18n;

let text = i18n::my_new_label();
let message = i18n::my_new_message("value");
```

**禁止**在代码中直接硬编码中/英文文案，必须通过 `i18n` 函数。

---

## CLI 命令扩展

### 添加新 CLI 选项

**步骤 1**：在 `Args` 结构体中添加字段（`src/main.rs`）：

```rust
#[derive(Parser, Debug)]
struct Args {
    // ... 现有字段

    #[arg(short = 'n', long, help = "新选项说明")]
    my_new_option: Option<String>,
}
```

**步骤 2**：在 `main()` 中添加路由逻辑：

```rust
if args.my_new_option.is_some() {
    return run_my_new_command(&args);
}
```

> **交互式 SQL REPL**：`-i` / `--interactive` 路由到 `run_interactive_cli()`，调用 `interactive::run(db, no_history)`（基于 rustyline），循环读取多行 SQL 并通过 `execute_sql` 执行。命令历史跨会话持久化到 `dirs::state_dir()/grep-excel/history.txt`（`--no-history` 关闭），每次 `add_history_entry` 后增量 `save_history` 以抵御执行期崩溃。详见 `crates/cli/src/interactive.rs`。

**步骤 3**：实现处理函数：

```rust
fn run_my_new_command(args: &Args) -> Result<()> {
    let mut db = DefaultEngine::new()?;
    // ... 导入文件
    // ... 执行逻辑
    Ok(())
}
```

### 添加 `--exec` 工具 dispatch

在 `exec_dispatch()` 函数的 `match tool` 中添加新分支（参见上文 MCP 工具开发步骤 5）。

---

## TUI 组件开发

TUI 使用 ratatui 框架，代码在 `src/app/` 目录。

### 关键组件

| 文件 | 职责 |
|------|------|
| `mod.rs` | App 状态管理、事件循环 |
| `handlers.rs` | 键盘事件 → App 状态变更 |
| `render.rs` | 主渲染逻辑 |
| `ui.rs` | 可复用 UI 组件（表格、标签等） |
| `theme.rs` | 颜色定义 |

### 添加新的 TUI 模式

**步骤 1**：在 `AppMode` 枚举中添加新模式：

```rust
pub enum AppMode {
    Normal,
    EditingSearch,
    // ... 现有模式
    MyNewMode,
}
```

**步骤 2**：在 `handlers.rs` 中添加模式切换逻辑

**步骤 3**：在 `render.rs` 中添加新模式下的渲染逻辑

**步骤 4**：在 `i18n.rs` 中添加相关文案

### TUI 异步模型

TUI 通过事件通道与引擎交互，避免阻塞 UI 线程：

```rust
pub enum AppEvent {
    Key(KeyEvent),           // 键盘事件
    Tick,                    // 定时器事件
    FileImported(Result<FileInfo>),
    SearchCompleted(Result<(Vec<SearchResult>, SearchStats)>),
    SqlCompleted(Result<SqlResult>),
    Progress(usize, usize),  // 导入进度
}
```

事件通过 `mpsc::channel` 传递。引擎的耗时操作（导入、搜索）在后台线程执行，完成后通过 `event_tx.send()` 通知 UI 线程。

---

## 数据流

### 搜索数据流

```
用户输入查询 (CLI/TUI/MCP)
    │
    ▼
SearchQuery { text, column, mode, limit, sheet, invert }
    │
    ▼
SearchEngine::search(&self, query)
    │
    ├── 遍历所有导入的文件/工作表
    │   ├── 如果 query.column 存在 → 仅搜索匹配列
    │   └── 否则 → 搜索所有列
    │
    ├── 对每行执行匹配 (engine/mod.rs: find_matched_columns)
    │   ├── FullText: 不区分大小写子串
    │   ├── ExactMatch: 区分大小写完全匹配
    │   ├── Wildcard: like_match() 自定义实现
    │   └── Regex: regex crate
    │
    └── 返回 (Vec<SearchResult>, SearchStats)
        │
        ▼
    SearchResult { file_name, sheet_name, row, col_names, matched_columns }
    SearchStats { total_rows_searched, total_matches, duration, ... }
```

### SQL 数据流

```
SQL 字符串
    │
    ▼
SearchEngine::execute_sql(&self, sql, limit)
    │
    ├── 内存引擎: 将数据转换为 SQLite 内存数据库 → 执行
    ├── DuckDB: 直接执行（支持窗口函数、:: 类型转换等）
    └── SQLite: 直接执行
    │
    └── 返回 SqlResult { columns, rows, row_count, duration, ... }
```

### MCP 数据流

```
AI 助手 → JSON-RPC (stdio) → rmcp → GrepExcelServer → SearchEngine
                                                          │
                              JSON 响应 ← rmcp ← serde_json ←┘
```

### Exec 数据流

```
CLI --exec JSON → serde_json 解析 → exec_dispatch() → SearchEngine
                       │
                  格式化为 markdown/pretty/json/simple → stdout
```

### Run Shell 数据流

```
CLI --run SHELL_CMD + (--query | --sql)
       │
       ├── 导入文件 → 执行搜索/SQL → 获取结果行
       │
       ├── 遍历每行:
       │   ├── expand_exec_template(): ${列名} → shell-escaped cell value
       │   │   └── shell_escape(): 单引号包裹 + 转义
       │   ├── sh -c <expanded_command>
       │   ├── stdout → print / --run-output-column → update_cell()
       │   └── stderr → eprintln (warning)
       │
       └── --export: save_as() 导出完整文件
```

`expand_exec_template` 在 `main.rs` 中实现，支持 `${column_name}` 占位符和 `$$` 转义。

### 交互式 REPL 数据流

```
CLI -i (files...) 
    │
    ├── 导入位置参数文件
    │
    ├── interactive::run() (interactive.rs, rustyline)
    │   ├── $ 提示符 → 读取多行输入（Validator: ';' 或 '.' 结尾才提交）
    │   ├── SQL 输入 → execute_and_print() → SearchEngine::execute_sql()
    │   │              → Unicode 对齐表格输出 + 行数统计
    │   └── 点命令 (.tables/.files/.help/.history/.clear/.exit)
    │
    └── Ctrl+D / .exit → 退出
```

### Excel 日期自动检测（导入阶段）

```
import_excel (excel.rs)
    │
    ├── 第一遍：收集 calamine Data 原始行
    │
    ├── detect_date_columns_from_data()
    │   ├── 信号 1（高置信度）：列含 ≥1 个 Data::DateTime 单元格 → 标记为日期列
    │   └── 信号 2（兜底）：列名匹配日期关键词 + >50% Float 值在 Excel 序列号范围 [1, 100000]
    │
    ├── 第二遍：对日期列调用 as_datetime() / excel_serial_to_date_string()
    │           转换为 YYYYMMDD 字符串
    │
    └── --repair 路径：convert_date_columns_in_place() 执行同样的后处理
```

---

## 发布流程

### CI/CD 配置

发布由 `.github/workflows/release.yml` 管理，当推送版本 tag 时触发：

```
git tag vX.Y.Z
git push origin vX.Y.Z
```

### 构建矩阵

CI 构建以下平台：

| 平台 | 目标三元组 | 引擎 |
|------|-----------|------|
| Linux x86_64 | `x86_64-unknown-linux-gnu` | DuckDB |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | DuckDB |
| macOS Intel | `x86_64-apple-darwin` | DuckDB |
| macOS ARM | `aarch64-apple-darwin` | DuckDB |
| Windows x86_64 | `x86_64-pc-windows-msvc` | DuckDB |

所有构建启用 `--features engine-duckdb,file-dialog`，使用 `cargo zigbuild` 确保 Linux glibc 2.31 兼容性。

### 发布产物

每个平台生成一个 zip 包，包含二进制文件和 LICENSE：

```
grep_excel-{target}-v{version}.zip
├── grep_excel (或 grep_excel.exe)
└── LICENSE
```

### 手动发布检查清单

- [ ] `cargo test --features full` 全部通过
- [ ] `cargo clippy --all-features` 无警告
- [ ] `Cargo.toml` version 已更新
- [ ] `README.md` 版本号已更新（如有）
- [ ] 手动验证各平台二进制功能正常
- [ ] git tag 已创建并推送

---

## 相关文档

- [README.md](../README.md) — 项目说明（中英双语）
- [UserGuide.md](UserGuide.md) — 最终用户手册
- [CONTRIBUTION.md](../CONTRIBUTION.md) — 贡献指南
