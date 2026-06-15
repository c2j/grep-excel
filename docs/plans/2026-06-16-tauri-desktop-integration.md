# grep-excel Tauri 桌面应用整合计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 grep-excel 搜索引擎内核接入 Desktop/ 下的 Tauri + React 骨架，形成独立桌面应用。保持 CLI/TUI/MCP 功能不变，核心代码最大化复用，逐步使项目符合 CONTRIBUTING.md 编码规范。

**Architecture:** 将当前单 crate 项目重组为 Cargo Workspace（M-ARCH-01），抽取 `crates/core` 库 crate 作为可复用引擎，保持 `crates/cli` 二进制 crate 原有行为不变，将 `Desktop/src-tauri` 作为 workspace member 接入。Tauri Rust 后端通过 `#[tauri::command]` 暴露 SearchEngine 全部能力，React 前端通过 `@tauri-apps/api` 的 `invoke` 调用。

**Tech Stack:** Rust (edition 2021), Tauri v1.0.0, React 18.2, TypeScript 5, Vite 4.4, TailwindCSS 3.3, calamine 0.26, serde 1, anyhow 1, thiserror 2

**Coding Standards:** 严格遵循 docs/CONTRIBUTING.md（M-* 强制规则）和 docs/BEST-PRATICE.md（R-* 建议规则）

---

## 现状诊断

### 规范违规（需在重构中解决）

| 规则 | 当前状态 | 违规影响 |
|------|---------|---------|
| **M-ARCH-01** | 单 crate，无 Workspace | 核心无法被多目标复用 |
| **M-ARCH-03** | main.rs(1824行), ui.rs(1763行), duckdb.rs(1348行), sqlite.rs(1180行), i18n.rs(978行), excel.rs(655行), mcp.rs(601行) 超 600 行 | 模块过大，难以维护 |
| **M-ERR-01** | `SearchEngine` trait 返回 `anyhow::Result`，库代码使用不透明错误 | 调用方无法精确处理错误类型 |
| **M-ARCH-04** | `lib.rs` 已是 9 行符合要求 ✅ | — |

### Desktop 目录状态

- Desktop/ 是独立的嵌套 git 仓库（有完整 `.git/`），**不是** git submodule
- Desktop 内部有 Tauri v1.0.0 + React 18 + Vite 4 的干净骨架
- `src-tauri/src/lib.rs` 仅定义 `pub type Result<T>`，无任何命令
- `src-tauri/src/main.rs` 仅 `tauri::Builder::default().run()`，无注册命令
- 前端 `frontend/App.tsx` 只有 demo greet 按钮

### 依赖兼容性确认

grep-excel 与 Desktop/src-tauri 的 Rust 依赖无版本冲突：
- serde 1.x ↔ 1.0 ✅
- anyhow 1.x ↔ 1.0 ✅
- 其余 grep-excel 依赖（calamine, duckdb, regex, csv 等）Desktop 不存在，可自由添加

---

## 阶段 0: Workspace 重组（基础架构）

> **目标**: 满足 M-ARCH-01，创建 Cargo Workspace，抽取 core crate。不影响 CLI/TUI/MCP 功能。
>
> **原则**: 最小改动 — 只移动文件位置，不改代码逻辑（除非是必须的路径调整）。

### Task 0.1: 创建 Workspace 清单

**Files:**
- 修改: `Cargo.toml`（根级）
- 创建: 目录 `crates/core/`, `crates/cli/`

**Step 1: 编写根 Workspace Cargo.toml**

```toml
[workspace]
members = ["crates/*"]
resolver = "2"

# Desktop 暂时不加，后续阶段再加
# "Desktop/src-tauri"

[workspace.package]
version = "0.2.10"
edition = "2021"
license = "MIT"
repository = "https://github.com/c2j/grep-excel"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "2"
regex = "1"
chrono = "0.4"
parking_lot = "0.12"
```

**Step 2: 创建目录结构**

```bash
mkdir -p crates/core/src/engine
mkdir -p crates/cli/src/app
mkdir -p crates/cli/src/bin
```

**Step 3: 验证 workspace 结构**

```bash
cargo metadata --no-deps
# 期望: workspace_members 包含 crates/core, crates/cli
```

**Step 4: Commit**

```bash
git add Cargo.toml crates/
git commit -m "chore: create cargo workspace structure"
```

---

### Task 0.2: 抽取核心 crate (`crates/core`)

**Files:**
- 创建: `crates/core/Cargo.toml`
- 创建: `crates/core/src/lib.rs`
- 移动: `src/types.rs` → `crates/core/src/types.rs`
- 移动: `src/engine/` → `crates/core/src/engine/`（mod.rs, memory.rs, duckdb.rs, sqlite.rs）
- 移动: `src/excel.rs` → `crates/core/src/excel.rs`
- 移动: `src/i18n.rs` → `crates/core/src/i18n.rs`
- 修改: 所有移动文件的 crate 内部引用路径

**Step 1: 编写 crates/core/Cargo.toml**

```toml
[package]
name = "grep-excel-core"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Core engine for grep-excel: Excel/CSV search and manipulation"

[dependencies]
calamine = { version = "0.26", features = ["chrono"] }
zip = "2"
roxmltree = "0.20"
csv = "1"
regex.workspace = true
anyhow.workspace = true
thiserror.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
parking_lot.workspace = true
unicode-width = "0.2"

# 可选引擎
duckdb = { version = "1.10501", optional = true }
rusqlite = { version = "0.32", features = ["bundled", "functions"], optional = true }

# 可选导出
rust_xlsxwriter = { version = "0.82", optional = true }

[features]
default = ["engine-memory"]
engine-memory = []
engine-duckdb = ["dep:duckdb"]
engine-sqlite = ["dep:rusqlite"]
duckdb-bundled = ["engine-duckdb", "duckdb/bundled"]
rust_xlsxwriter = ["dep:rust_xlsxwriter"]
```

**Step 2: 编写 crates/core/src/lib.rs**

```rust
pub mod types;
pub mod engine;
pub mod excel;
pub mod i18n;
// 注意: app/ mcp/ 留在 cli crate 中
```

**Step 3: 移动文件并修正路径**

移动后需要修正以下引用：
- `engine/mod.rs` 中的 `crate::types::*` → 保持（已在同 crate）
- `engine/` 子模块中的 `use crate::types::*` → 保持
- `excel.rs` 中的引用 → 保持（在同 crate）
- `i18n.rs` 中的引用 → 保持（在同 crate）

**关键**: `engine/mod.rs` 中导出的 `DefaultEngine` 类型别名需保持 — 这是核心 crate 的公共 API。

**Step 4: 验证编译**

```bash
cargo build -p grep-excel-core
# 期望: 编译成功（可能有未使用导入的 warning，暂不处理）
```

**Step 5: 验证测试**

```bash
cargo test -p grep-excel-core
# 期望: 现有测试通过（如果有）
```

**Step 6: Commit**

```bash
git add crates/core/
git rm src/types.rs src/engine/ src/excel.rs src/i18n.rs
git commit -m "refactor: extract core crate with engine, types, excel, i18n"
```

---

### Task 0.3: 创建 CLI crate (`crates/cli`)

**Files:**
- 创建: `crates/cli/Cargo.toml`
- 移动: `src/main.rs` → `crates/cli/src/main.rs`
- 移动: `src/app/` → `crates/cli/src/app/`
- 移动: `src/mcp.rs` → `crates/cli/src/mcp.rs`
- 移动: `src/bin/spike.rs` → `crates/cli/src/bin/spike.rs`
- 删除: `src/lib.rs`（不再需要）或简化为 re-export
- 修改: 所有 `grep_excel::` → `grep_excel_core::` 引用

**Step 1: 编写 crates/cli/Cargo.toml**

```toml
[package]
name = "grep-excel"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "TUI tool for searching Excel/CSV files"

[[bin]]
name = "grep_excel"
path = "src/main.rs"

[[bin]]
name = "spike"
path = "src/bin/spike.rs"
required-features = ["gui"]

[dependencies]
grep-excel-core = { path = "../core" }

# TUI
ratatui = "0.30"
crossterm = "0.28"
tui-input = "0.11"
rfd = { version = "0.15", optional = true }

# CLI
clap = { version = "4", features = ["derive"] }

# 通用
anyhow.workspace = true
serde.workspace = true
serde_json.workspace = true
regex.workspace = true
unicode-width = "0.2"
tokio = { version = "1", features = ["macros", "rt-multi-thread"], optional = true }

# MCP
rmcp = { version = "1.5", features = ["server", "transport-io"], optional = true }
schemars = { version = "1", optional = true }

# GUI spike
eframe = { version = "0.31", optional = true }
egui = { version = "0.31", optional = true }
egui_extras = { version = "0.31", optional = true }

[features]
default = ["file-dialog"]
file-dialog = ["dep:rfd"]
mcp-server = ["dep:rmcp", "dep:tokio", "dep:schemars", "grep-excel-core/rust_xlsxwriter"]
gui = ["dep:eframe", "dep:egui", "dep:egui_extras", "file-dialog"]
full = ["file-dialog", "mcp-server"]

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

**Step 2: 修改 main.rs 中的 crate 引用**

将所有 `use grep_excel::` 替换为 `use grep_excel_core::`（类型、引擎）
将 `use crate::` 引用（指 app/mcp 模块）保持不变

具体需要修改的引用：
- `use grep_excel::app::App` → `use crate::app::App`（同一 crate）
- `use grep_excel::engine::*` → `use grep_excel_core::engine::*`
- `use grep_excel::types::*` → `use grep_excel_core::types::*`
- `use grep_excel::i18n` → `use grep_excel_core::i18n`
- `grep_excel::mcp::` → `crate::mcp::`
- `grep_excel::engine::export_results_csv` → `grep_excel_core::engine::export_results_csv`

**Step 3: 删除原始 src/ 目录中已移动的文件**

```bash
rm -f src/lib.rs   # 如果不需要保留
# src/ 下不再有源文件
```

**Step 4: 验证编译（每种配置）**

```bash
# 默认配置
cargo build -p grep-excel
# 期望: 成功

# 完整配置
cargo build -p grep-excel --features full
# 期望: 成功

# DuckDB 配置
cargo build -p grep-excel --features "grep-excel-core/engine-duckdb"
# 期望: 成功
```

**Step 5: 运行验证**

```bash
# CLI 搜索
cargo run -p grep-excel -- test_data.xlsx -q "test" -f simple
# 期望: 与重构前结果一致

# 帮助输出
cargo run -p grep-excel -- --help
# 期望: 与重构前一致
```

**Step 6: Commit**

```bash
git add crates/cli/
git rm -r src/
git commit -m "refactor: move CLI/TUI/MCP to cli crate, depend on core"
```

---

### Task 0.4: 清理与最终验证

**Step 1: 确保 tests/ 目录指向正确**

tests/ 目录下的集成测试需要更新 crate 引用。检查并修正。

```bash
grep -r "grep_excel::" tests/
# 如有引用，改为 grep_excel_core:: 或 grep_excel::
```

**Step 2: 全量构建验证**

```bash
cargo build --workspace
cargo test --workspace
cargo fmt --check --all
cargo clippy --workspace -- -D warnings
```

**Step 3: 确认 CLI 二进制产物**

```bash
cargo build -p grep-excel --release
ls -la target/release/grep_excel
# 期望: 存在且可执行
```

**Step 4: Commit**

```bash
git commit -am "chore: finalize workspace migration, update tests"
```

---

## 阶段 1: Desktop Tauri 整合

> **注意**: 根据决策，跳过 anyhow→thiserror 迁移（M-ERR-01）和文件拆分（M-ARCH-03），后续专门 PR 处理。
> 阶段 0 完成后直接进入 Desktop 整合。

---

### Task 1.1: 处理 Desktop 的嵌套 git 仓库

---

## 阶段 1: Desktop Tauri 整合

> **目标**: 将 Desktop skeleton 接入 workspace，依赖 core crate，暴露 Tauri 命令。
>
> **决策**: 移除 Desktop/.git/（嵌套仓库），纳入主仓库管理。Desktop 前端支持中英双语。Desktop 支持双引擎 feature（memory 默认 + duckdb-bundled 可选）。

### Task 1.1: 处理 Desktop 的 git 仓库

**当前状态**: Desktop/ 有独立的 `.git/` 目录（嵌套仓库），不是 submodule。

**决策**: 移除 Desktop/.git/，将 Desktop 纳入主仓库管理。

**Step 1: 备份并移除嵌套 git**

```bash
# 先确保 Desktop 的修改已保存
cd Desktop && git status

# 移入主仓库
rm -rf Desktop/.git Desktop/.github
# .gitignore 保留（可能有用）
```

**Step 2: 将 Desktop 加入主仓库**

```bash
git add Desktop/
git status
# 确认 Desktop 文件被正确跟踪
```

**Step 3: Commit**

```bash
git commit -m "chore: integrate desktop skeleton into main repo"
```

---

### Task 1.2: Desktop 加入 Workspace

**Files:**
- 修改: `Cargo.toml`（根 workspace 清单）
- 修改: `Desktop/src-tauri/Cargo.toml`
- 修改: `Desktop/src-tauri/src/lib.rs`

**Step 1: 更新 workspace members**

```toml
# Cargo.toml (根)
[workspace]
members = ["crates/*", "Desktop/src-tauri"]
```

**Step 2: 更新 Desktop/src-tauri/Cargo.toml**

```toml
[package]
name = "tauri-react-app"
version = "0.1.0"
edition.workspace = true

[dependencies]
# 核心引擎
grep-excel-core = { path = "../../crates/core" }

# Tauri
tauri = { version = "1.0.0", features = ["shell-open"] }

# 通用
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true

[build-dependencies]
tauri-build = { version = "1", features = [] }

[features]
default = ["engine-memory"]
engine-memory = []
engine-duckdb = ["grep-excel-core/engine-duckdb"]
duckdb-bundled = ["engine-duckdb", "grep-excel-core/duckdb-bundled"]
```

**Step 3: 验证 workspace 解析**

```bash
cargo metadata --no-deps
# 期望: workspace_members 包含 Desktop/src-tauri
```

**Step 4: Commit**

```bash
git add Cargo.toml Desktop/src-tauri/Cargo.toml
git commit -m "chore: add Desktop tauri app as workspace member"
```

---

### Task 1.3: 编写 Tauri 命令层

**Files:**
- 创建: `Desktop/src-tauri/src/commands.rs`
- 修改: `Desktop/src-tauri/src/lib.rs`
- 修改: `Desktop/src-tauri/src/main.rs`

**Step 1: 编写 commands.rs**

```rust
// Desktop/src-tauri/src/commands.rs
use grep_excel_core::engine::{DefaultEngine, SearchEngine};
use grep_excel_core::types::*;
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::State;

// 应用状态：持有搜索引擎实例
pub struct AppState {
    pub engine: Arc<Mutex<DefaultEngine>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            engine: Arc::new(Mutex::new(DefaultEngine::new().expect("Failed to initialize search engine"))),
        }
    }
}

#[tauri::command]
pub async fn import_file(
    path: String,
    state: State<'_, AppState>,
) -> Result<FileInfo, String> {
    let path = std::path::Path::new(&path);
    let mut engine = state.engine.lock();
    engine
        .import_excel(path, &|_, _| {})
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search(
    query: SearchQuery,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let engine = state.engine.lock();
    let (results, _stats) = engine
        .search(&query)
        .map_err(|e| e.to_string())?;
    Ok(results)
}

#[tauri::command]
pub async fn execute_sql(
    sql: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<SqlResult, String> {
    let engine = state.engine.lock();
    engine
        .execute_sql(&sql, limit.unwrap_or(1000))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_files(
    state: State<'_, AppState>,
) -> Result<Vec<FileInfo>, String> {
    let engine = state.engine.lock();
    Ok(engine.list_files())
}

#[tauri::command]
pub async fn get_metadata(
    file_name: String,
    state: State<'_, AppState>,
) -> Result<FileMetadataInfo, String> {
    let engine = state.engine.lock();
    engine
        .get_metadata(&file_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_sheet_data(
    params: GetSheetDataParams,
    state: State<'_, AppState>,
) -> Result<SheetDataResult, String> {
    let engine = state.engine.lock();
    engine
        .get_sheet_data(
            &params.file_name,
            &params.sheet_name,
            params.start_row,
            params.end_row,
            params.columns.as_deref(),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_cell(
    file_name: String,
    sheet_name: String,
    row: usize,
    column: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut engine = state.engine.lock();
    engine
        .update_cell(&file_name, &sheet_name, row, &column, &value)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_file(
    file_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let engine = state.engine.lock();
    // save 需要 save_as 或其他导出机制；rust_xlsxwriter feature
    // 暂时返回错误
    Err("save not yet implemented".into())
}

#[tauri::command]
pub async fn clear_data(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut engine = state.engine.lock();
    engine.clear().map_err(|e| e.to_string())
}
```

**Step 2: 更新 lib.rs**

```rust
// Desktop/src-tauri/src/lib.rs
mod commands;

pub use commands::*;
```

**Step 3: 更新 main.rs**

```rust
// Desktop/src-tauri/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri_react_app_lib::{
    AppState,
    import_file, search, execute_sql, list_files,
    get_metadata, get_sheet_data, update_cell, save_file, clear_data,
};

fn main() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            import_file,
            search,
            execute_sql,
            list_files,
            get_metadata,
            get_sheet_data,
            update_cell,
            save_file,
            clear_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**注意**: 需要确保 `SearchQuery`, `FileInfo`, `SearchResult` 等类型实现 `Serialize`/`Deserialize`。当前代码中这些类型已经 derive `Serialize`/`Deserialize`（通过 serde），可以经 Tauri IPC 传递。

**Step 4: 更新 tauri.conf.json 安全白名单**

需要允许文件系统访问：

```json
{
  "tauri": {
    "allowlist": {
      "all": false,
      "shell": { "all": false, "open": true },
      "fs": {
        "all": false,
        "readFile": true,
        "scope": ["**"]
      },
      "dialog": {
        "all": false,
        "open": true,
        "save": true
      }
    }
  }
}
```

**Step 5: 验证编译**

```bash
cargo build -p tauri-react-app
# 期望: 编译成功
```

**Step 6: Commit**

```bash
git add Desktop/src-tauri/src/
git add Desktop/src-tauri/tauri.conf.json
git commit -m "feat(desktop): add Tauri commands wrapping SearchEngine"
```

---

## 阶段 2: React 前端开发

> **目标**: 替换 Desktop 的 demo UI，构建中英双语的搜索 Excel 桌面界面。
>
> **i18n 方案**: 前端使用 [i18next](https://react.i18next.com/) + [react-i18next](https://react.i18next.com/)，与 grep-excel Rust 侧 i18n 独立（前端自己维护翻译文件）。用户可在界面中切换语言，通过 React Context 全局生效。

### Task 2.1: TypeScript 类型定义 + i18n 基础设施

**Files:**
- 创建: `Desktop/frontend/types/search.ts`

```typescript
// 与 Rust types.rs 对应的 TypeScript 类型
export interface SearchQuery {
  text: string;
  column: string | null;
  mode: "FullText" | "ExactMatch" | "Wildcard" | "Regex";
  limit: number;
  sheet: string | null;
  invert: boolean;
}

export interface SearchResult {
  sheet_name: string;
  file_name: string;
  row: string[];
  col_names: string[];
  matched_columns: number[];
  col_widths: number[];
  row_index: number;
}

export interface FileInfo {
  name: string;
  sheets: [string, number][];
  total_rows: number;
  sample: FileSample | null;
}

export interface FileSample {
  sheet_name: string;
  headers: string[];
  rows: string[][];
}

export interface FileMetadataInfo {
  file_name: string;
  sheet_count: number;
  sheets: SheetMetadataInfo[];
}

export interface SheetMetadataInfo {
  sheet_name: string;
  row_count: number;
  columns: string[];
}

export interface SheetDataResult {
  file_name: string;
  sheet_name: string;
  columns: string[];
  rows: string[][];
  row_count: number;
  total_rows: number;
  truncated: boolean;
}

export interface SqlResult {
  columns: string[];
  rows: string[][];
  row_count: number;
  truncated: boolean;
  duration: { secs: number; nanos: number };
}

export type SearchMode = "fulltext" | "exact" | "wildcard" | "regex";
```

### Task 2.2: API 层

**Files:**
- 修改: `Desktop/frontend/api/tauri.ts`（重命名为 commands.ts）
- 创建: `Desktop/frontend/api/commands.ts`

```typescript
import { invoke } from "@tauri-apps/api/tauri";
import type {
  SearchQuery, SearchResult, FileInfo,
  FileMetadataInfo, SheetDataResult, SqlResult,
} from "../types/search";

export async function importFile(path: string): Promise<FileInfo> {
  return invoke("import_file", { path });
}

export async function search(query: SearchQuery): Promise<SearchResult[]> {
  return invoke("search", { query });
}

export async function executeSql(sql: string, limit?: number): Promise<SqlResult> {
  return invoke("execute_sql", { sql, limit });
}

export async function listFiles(): Promise<FileInfo[]> {
  return invoke("list_files");
}

export async function getMetadata(fileName: string): Promise<FileMetadataInfo> {
  return invoke("get_metadata", { fileName });
}

export async function getSheetData(
  file_name: string,
  sheet_name: string,
  start_row?: number,
  end_row?: number,
  columns?: string[],
): Promise<SheetDataResult> {
  return invoke("get_sheet_data", {
    params: { file_name, sheet_name, start_row, end_row, columns }
  });
}

export async function updateCell(
  file_name: string,
  sheet_name: string,
  row: number,
  column: string,
  value: string,
): Promise<void> {
  return invoke("update_cell", { file_name, sheet_name, row, column, value });
}

export async function saveFile(fileName: string): Promise<void> {
  return invoke("save_file", { fileName });
}

export async function clearData(): Promise<void> {
  return invoke("clear_data");
}
```

### Task 2.3: UI 组件 — 搜索功能

**Files:**
- 创建: `Desktop/frontend/components/SearchBar.tsx`
- 创建: `Desktop/frontend/components/ResultsTable.tsx`
- 创建: `Desktop/frontend/components/FileImporter.tsx`
- 修改: `Desktop/frontend/App.tsx`

**组件设计要点:**
- SearchBar: 搜索输入框 + 模式切换（fulltext/exact/wildcard/regex）+ 列筛选 + 工作表筛选
- ResultsTable: 虚拟化表格（大数据量性能），高亮匹配列，行号显示
- FileImporter: 拖拽区域 + 文件选择按钮，导入后显示文件名/工作表数/行数

**样式**: 使用 TailwindCSS utility classes，保持与模板一致的 `primary` 色系。

### Task 2.4: UI 组件 — SQL 查询

**Files:**
- 创建: `Desktop/frontend/components/SqlEditor.tsx`

SQL 编辑器 + 结果表格。简单实现：`textarea` + 执行按钮 + 结果渲染。

### Task 2.5: UI 组件 — 数据编辑

**Files:**
- 创建: `Desktop/frontend/components/CellEditor.tsx`

双击单元格进入编辑模式，回车提交，ESC 取消。

### Task 2.6: i18n 语言切换 UI + 集成测试

```bash
cd Desktop
npm install
npm run tauri:dev
```

验证：
1. 点击导入按钮 → 文件对话框弹出 → 选择 .xlsx → 显示文件信息
2. 在搜索栏输入关键词 → 显示匹配结果
3. 切换搜索模式（精确/通配符/正则）→ 结果正确
4. SQL 查询 → 结果正确渲染
5. 双击单元格编辑 → 修改保存

---

## 阶段 3: 最终验证与清理

### Task 3.1: 全量 CI 检查

```bash
# 格式检查
cargo fmt --check --all

# Clippy
cargo clippy --workspace -- -D warnings

# 测试
cargo test --workspace

# 文档
cargo doc --workspace --no-deps
```

### Task 3.2: 确认 CLI 不受影响

```bash
cargo run -p grep-excel -- test_data.xlsx -q "test" -f simple
cargo run -p grep-excel -- --help
cargo run -p grep-excel  # TUI 模式
```

### Task 3.3: 确认 Desktop 构建

```bash
cd Desktop
npm run tauri:build
# 期望: 生成 .dmg / .AppImage / .msi
```

### Task 3.4: 提交最终变更

```bash
git add -A
git status
git commit -m "feat: complete tauri desktop integration with react frontend"
```

---

## 已确认决策

| 决策 | 结论 |
|------|------|
| M-ERR-01 anyhow→thiserror | **跳过**，后续 PR |
| Desktop .git/ 处理 | **移除嵌套 .git/**，纳入主仓库 |
| M-ARCH-03 文件拆分 | **跳过**，后续 PR |
| 前端语言 | **中英双语**，使用 react-i18next |
| Desktop 默认引擎 | **双 feature**：memory 默认 + duckdb-bundled 可选 |

---

## 时间估算

| 阶段 | 任务 | 预估工作量 |
|------|------|-----------|
| 阶段 0 | Workspace 重组 | 1-2 小时 |
| 阶段 1 | Desktop Tauri 整合 | 1-2 小时 |
| 阶段 2 | React 前端开发 | 5-8 小时 |
| 阶段 3 | 验证与清理 | 0.5 小时 |
| **合计** | | **8-13 小时** |

## 依赖关系

```
阶段 0 (Workspace) ──► 阶段 1 (Tauri整合) ──► 阶段 2 (React前端) ──► 阶段 3 (验证)
```

---

> **Plan saved**: `docs/plans/2026-06-16-tauri-desktop-integration.md`
