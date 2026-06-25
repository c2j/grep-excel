# grep-excel Desktop

基于 Tauri + React + TypeScript 的 grep-excel 桌面 GUI 版本。

grep-excel Desktop 提供图形化界面，复用 grep-excel 核心引擎（`grep-excel-core`），支持文件导入、多模式搜索、SQL 查询和单元格编辑，界面中英双语。

## 功能特性

- **图形化文件导入** — 原生文件选择对话框，支持 `.xlsx`、`.xls`、`.xlsm`、`.xlsb`、`.ods`、`.csv`
- **多模式搜索** — 全文、精确、通配符、正则四种模式，可按列/工作表筛选
- **SQL 查询** — 内置 SQL 编辑器，直接对导入数据执行 `SELECT` 查询
- **虚拟滚动结果表** — 大数据集流畅渲染（PR #12 优化），避免 DOM 节点爆炸
- **单元格编辑** — 双击结果表格即可编辑单元格值
- **中英双语界面** — 基于 react-i18next，随系统语言自动切换

## 技术栈

| 层 | 技术 |
|----|------|
| 桌面框架 | [Tauri](https://tauri.app/) v1 |
| 前端框架 | React 18 + TypeScript |
| 构建工具 | Vite |
| 样式 | TailwindCSS |
| 国际化 | react-i18next |
| 后端引擎 | `grep-excel-core`（通过 Tauri Commands 调用） |

## 前置条件

- [Node.js](https://nodejs.org/) 16+
- [Rust](https://www.rust-lang.org/)（最新 stable）
- [Tauri CLI v1](https://tauri.app/v1/guides/getting-started/prerequisites) 系统依赖

```bash
# 安装 Tauri CLI（如果尚未安装）
cargo install tauri-cli --version "^1.0"
```

## 快速开始

```bash
cd Desktop

# 1. 安装前端依赖
npm install

# 2. 开发模式（启动 Vite + Tauri 窗口）
npm run tauri:dev

# 3. 生产构建（生成 .dmg / .AppImage / .msi）
npm run tauri:build
```

构建产物位于 `src-tauri/target/release/bundle/`。

## 项目结构

```
Desktop/
├── src-tauri/                    # Tauri 后端 (Rust)
│   ├── src/
│   │   ├── main.rs               # 应用入口
│   │   ├── lib.rs                # Tauri Builder + invoke_handler 注册
│   │   └── commands.rs           # Tauri Commands（桥接 grep-excel-core）
│   ├── Cargo.toml                # Rust 依赖（依赖 grep-excel-core）
│   ├── tauri.conf.json           # Tauri 配置（窗口、权限、标识）
│   └── build.rs
│
├── frontend/                     # 前端源码 (React + TypeScript)
│   ├── App.tsx                   # 主组件（Search / SQL 标签页）
│   ├── index.tsx                 # React 入口
│   ├── index.css                 # 全局样式 (Tailwind directives)
│   ├── api/
│   │   └── commands.ts           # Tauri Command 类型化封装
│   ├── components/
│   │   ├── FileImporter.tsx      # 文件导入按钮 + 对话框
│   │   ├── FileList.tsx          # 已导入文件列表
│   │   ├── SearchBar.tsx         # 搜索栏（模式/列/工作表筛选）
│   │   ├── ResultsTable.tsx      # 结果表格（虚拟滚动）
│   │   └── SqlEditor.tsx         # SQL 编辑器
│   ├── i18n/                     # react-i18next 翻译文件
│   └── types/                    # TypeScript 类型定义
│
├── index.html                    # HTML 模板
├── package.json                  # Node.js 依赖与脚本
├── vite.config.ts                # Vite 配置
├── tailwind.config.js            # TailwindCSS 配置
└── tsconfig.json                 # TypeScript 配置
```

## Tauri Commands（后端接口）

前端通过 `@tauri-apps/api` 的 `invoke()` 调用以下 Rust 命令，这些命令在 `src-tauri/src/commands.rs` 中定义，桥接到 `grep-excel-core` 的 `SearchEngine` trait：

| Command | 功能 |
|---------|------|
| `import_file` | 导入 Excel/CSV 文件 |
| `search` | 多模式搜索（fulltext/exact/wildcard/regex） |
| `execute_sql` | 执行 SQL SELECT 查询 |
| `list_files` | 列出已导入文件 |
| `list_table_aliases` | 列出表别名（`文件名.工作表名`） |
| `get_metadata` | 获取文件元数据（列名等） |
| `get_sheet_sample` | 均匀采样行数据 |
| `get_sheet_data` | 分页获取行数据 |
| `update_cell` | 更新单元格 |
| `clear_data` | 清除所有已导入数据 |

前端封装位于 `frontend/api/commands.ts`，提供类型安全的 Promise 接口。

## 开发指南

### 添加新的 Tauri Command

1. 在 `src-tauri/src/commands.rs` 中添加命令函数：

```rust
#[tauri::command]
fn my_command(value: String) -> Result<String, String> {
    Ok(format!("Received: {}", value))
}
```

2. 在 `src-tauri/src/lib.rs` 的 `invoke_handler` 中注册：

```rust
.invoke_handler(tauri::generate_handler![
    commands::import_file,
    // ...
    commands::my_command,
])
```

3. 在 `frontend/api/commands.ts` 中添加类型化封装：

```typescript
export async function myCommand(value: string): Promise<string> {
  return await invoke("my_command", { value });
}
```

### 窗口配置

编辑 `src-tauri/tauri.conf.json` 中的 `app.windows` 数组：

```json
{
  "title": "grep-excel",
  "width": 1200,
  "height": 800,
  "resizable": true
}
```

### 中文输入法支持

WKWebView 中默认无法使用中文输入法。已在 `tauri.conf.json` 中设置 `lang=zh` 并在前端处理 IME composition 事件以确保 CJK 输入正常。

## 可用脚本

| 命令 | 说明 |
|------|------|
| `npm run dev` | 启动 Vite 开发服务器（无 Tauri 窗口） |
| `npm run build` | 构建前端生产包 |
| `npm run tauri:dev` | 开发模式（Vite + Tauri 窗口） |
| `npm run tauri:build` | 生产构建（生成安装包） |

## 相关文档

- [项目根 README](../README.md) — grep-excel CLI/TUI/MCP 完整说明
- [用户手册](../docs/UserGuide.md) — CLI、TUI、MCP 使用指南
- [开发者指南](../docs/DeveloperGuide.md) — 架构、引擎 trait、扩展开发
- [Desktop 集成计划](../docs/plans/2026-06-16-tauri-desktop-integration.md) — Tauri 集成设计文档

## 许可证

MIT License — 详见项目根目录 [LICENSE](../LICENSE)。
