# 贡献指南

感谢你对 grep-excel 的关注！本文档将帮助你了解项目结构、搭建开发环境并提交贡献。

## 目录

- [项目结构](#项目结构)
- [开发环境搭建](#开发环境搭建)
- [构建与测试](#构建与测试)
- [Feature Flags（功能标志）](#feature-flags功能标志)
- [代码风格](#代码风格)
- [提交流程](#提交流程)
- [Pull Request 指南](#pull-request-指南)
- [版本发布](#版本发布)

---

## 项目结构

```
grep-excel/
├── .github/
│   └── workflows/        # CI/CD 工作流（GitHub Actions）
│       └── release.yml   # 自动构建发布
├── docs/
│   ├── plans/            # 开发计划文档（.sisyphus）
│   ├── UserGuide.md      # 用户手册
│   └── DeveloperGuide.md # 开发者指南
├── src/
│   ├── main.rs           # 入口点：CLI 解析、模式分发
│   ├── lib.rs            # 库入口：模块声明
│   ├── types.rs          # 核心类型定义 + MCP 参数结构体
│   ├── excel.rs          # Excel/CSV 文件解析（calamine + csv crate）
│   ├── event.rs          # 事件通道（TUI 异步通信）
│   ├── i18n.rs           # 国际化：中/英文翻译函数
│   ├── mcp.rs            # MCP 服务器实现（rmcp）
│   ├── app/
│   │   ├── mod.rs        # App 状态管理、事件循环
│   │   ├── handlers.rs   # 键盘事件处理
│   │   ├── render.rs     # 终端渲染逻辑
│   │   ├── theme.rs      # 颜色主题
│   │   └── ui.rs         # UI 组件绘制
│   └── engine/
│       ├── mod.rs        # SearchEngine trait 定义
│       ├── memory.rs     # 内存引擎实现
│       ├── duckdb.rs     # DuckDB 引擎实现（可选）
│       └── sqlite.rs     # SQLite 引擎实现（可选）
├── tests/                # 集成测试
├── Cargo.toml            # 项目配置、依赖、feature flags
├── README.md             # 项目说明（中英双语）
└── CONTRIBUTION.md       # 本文件
```

### 核心架构

```
┌──────────────────────────────────────────────────┐
│                    main.rs                        │
│  CLI 解析 → 路由到 run_cli / run_tui / run_mcp   │
└────────┬──────────────────┬──────────────────────┘
         │                  │
         ▼                  ▼
   ┌──────────┐     ┌──────────────┐
   │   CLI    │     │     TUI      │
   │ 模式分发  │     │  app/mod.rs  │
   │ exec/sql │     │  ratatui UI  │
   └────┬─────┘     └──────┬───────┘
        │                  │
        ▼                  ▼
   ┌──────────────────────────────────┐
   │         engine/mod.rs            │
   │       SearchEngine trait         │
   ├────────┬───────────┬─────────────┤
   │ memory │  duckdb   │   sqlite    │
   └────────┴───────────┴─────────────┘
                │
                ▼
   ┌──────────────────────┐
   │   MCP Server (mcp.rs)│
   │   rmcp + stdio       │
   └──────────────────────┘
```

所有引擎实现 `SearchEngine` trait（定义在 `src/engine/mod.rs`），使得 CLI、TUI 和 MCP 可以透明切换后端。

---

## 开发环境搭建

### 前提条件

- **Rust** 1.70+（推荐使用 [rustup](https://rustup.rs)）
- **Cargo**（随 Rust 一起安装）
- **Git**

### 克隆仓库

```bash
git clone https://github.com/c2j/grep-excel.git
cd grep-excel
```

### 编辑器配置

推荐使用 VS Code + rust-analyzer 插件。项目根目录无需额外配置，rust-analyzer 会自动识别 Cargo.toml。

> **注意**：由于项目使用多个 feature flags，你可能需要在 VS Code 中配置 rust-analyzer 使用的 features：
>
> ```json
> {
>   "rust-analyzer.cargo.features": ["full"]
> }
> ```

---

## 构建与测试

### 快速开发构建

```bash
# 默认构建（内存引擎，最快）
cargo build

# 带 DuckDB 引擎（生产环境推荐，编译较慢）
cargo build --features full
```

### Feature Flag 组合

开发时可以根据需要启用不同组合：

```bash
# 仅内存引擎（最快编译，功能最少）
cargo build --no-default-features --features engine-memory

# 内存引擎 + DuckDB
cargo build --no-default-features --features engine-memory,engine-duckdb

# DuckDB 引擎 + MCP + 文件编辑
cargo build --no-default-features --features engine-duckdb,mcp-server,rust_xlsxwriter

# DuckDB 捆绑模式（自包含，无需系统库）
cargo build --features duckdb-bundled,mcp-server
```

### 运行测试

```bash
# 全部测试
cargo test

# 特定引擎测试
cargo test --features engine-duckdb
cargo test --features engine-sqlite

# 包含 MCP 功能的测试
cargo test --features full
```

### 测试数据

项目根目录包含测试用 Excel 文件：

- `test_data.xlsx` — 基本测试数据
- `test_data2.xlsx` — 多工作表测试
- `test_data3.xlsx` — 包含不同类型数据
- `test_data4.xlsx` — 更大数据集

### 手动验证

```bash
# CLI 搜索
cargo run -- test_data.xlsx -q "test"

# TUI 模式
cargo run

# MCP 模式
cargo run -- --mcp
```

### 加速 DuckDB 编译

DuckDB 的 C 库编译非常耗时（10-30 分钟）。使用以下方式加速：

```bash
# 方式一：下载预编译 DuckDB 库
DUCKDB_DOWNLOAD_LIB=1 cargo build --features full

# 方式二：先开发内存引擎功能，最后再测试 DuckDB
cargo build --features engine-memory,mcp-server
```

---

## Feature Flags（功能标志）

项目使用 Cargo features 来管理可选功能。以下是所有可用的 feature flags：

| Flag | 依赖 | 说明 |
|------|------|------|
| `engine-memory` | 无 | 纯内存引擎。无外部数据库依赖，编译最快。**默认启用**。 |
| `engine-duckdb` | `duckdb` crate | DuckDB 引擎。高性能 OLAP 分析。 |
| `duckdb-bundled` | `duckdb/bundled` | DuckDB + 捆绑 C 库。独立运行，无需系统安装 DuckDB。自动继承 `engine-duckdb`。 |
| `engine-sqlite` | `rusqlite` (bundled) | SQLite 引擎。轻量级 SQL 查询。 |
| `file-dialog` | `rfd` | 原生文件选择对话框。**默认启用**。无头环境可禁用。 |
| `mcp-server` | `rmcp`, `tokio`, `schemars` | MCP 服务器模式 + JSON Schema 生成。同时启用读写功能。 |
| `rust_xlsxwriter` | `rust_xlsxwriter` | xlsx 写入支持。`save_as` 和 `save` 工具的必要依赖。 |
| `full` | 上述全部 | 完整功能：`engine-memory` + `file-dialog` + `mcp-server` |

### Feature Flag 设计原则

1. **默认轻量**：`default = ["engine-memory", "file-dialog"]`，保证开箱即用
2. **可选重型依赖**：DuckDB、SQLite、MCP 通过 feature flags 按需引入
3. **条件编译**：使用 `#[cfg(feature = "...")]` 保护可选代码路径
4. **组合灵活**：各 flag 互相独立（除 `duckdb-bundled` → `engine-duckdb`）

---

## 代码风格

### Rust 约定

- 遵循标准 Rust 风格（`rustfmt`）
- 使用 `cargo clippy` 检查代码质量
- 避免 `unsafe` 代码（当前仅在 `SyncDb` wrapper 中使用必要的 `unsafe impl`）
- 使用 `anyhow::Result` 进行错误处理
- 使用 `thiserror` 定义库级别的错误类型

### 项目约定

- **模块化**：每个源文件职责单一
- **trait 抽象**：所有引擎通过 `SearchEngine` trait 交互，新增引擎只需实现该 trait
- **i18n 优先**：所有面向用户的文本必须通过 `i18n.rs` 的函数获取，支持中英双语
- **类型安全**：禁止使用 `as any`、`@ts-ignore` 等类型规避手段
- **MCP 参数**：所有 MCP 工具参数结构体定义在 `src/types.rs`，同时用于 CLI `--exec` 和 MCP server

### 格式化与 Lint

```bash
# 格式化代码
cargo fmt

# 运行 clippy
cargo clippy --all-features -- -D warnings
```

---

## 提交流程

### Commit Message 规范

遵循 [Conventional Commits](https://www.conventionalcommits.org/) 格式：

```
<type>(<scope>): <description>

[optional body]
```

类型（type）：

| Type | 说明 |
|------|------|
| `feat` | 新功能 |
| `fix` | Bug 修复 |
| `refactor` | 代码重构（无功能变更） |
| `chore` | 构建/配置/依赖更新 |
| `docs` | 文档更新 |
| `test` | 测试相关 |
| `ci` | CI/CD 配置 |
| `style` | 代码风格（格式化等） |

示例：

```
feat(cli): add --repair flag to recover data from damaged xlsx files
fix(duckdb): handle non-TEXT column types in execute_sql via get_ref()
refactor(mcp): use shared param types from types.rs
chore: bump version to v0.2.4
```

### 分支策略

- `main` — 稳定分支，可发布
- `feat/*` — 功能分支（如 `feat/repair-corrupted-xlsx`）
- `fix/*` — 修复分支
- `refactor/*` — 重构分支

### 开发流程

1. 从 `main` 创建功能分支
2. 在功能分支上开发和测试
3. 确保所有测试通过：`cargo test --features full`
4. 运行 clippy：`cargo clippy --all-features`
5. 提交并推送
6. 创建 Pull Request

---

## Pull Request 指南

### 提交前检查清单

- [ ] 代码已格式化（`cargo fmt`）
- [ ] 通过 clippy 检查（`cargo clippy --all-features`）
- [ ] 所有测试通过（`cargo test --features full`）
- [ ] 新增功能已添加测试
- [ ] 新功能已在 `README.md` 或相关文档中说明
- [ ] 如果修改了 MCP 工具接口，已更新 JSON Schema
- [ ] 如果新增了面向用户的文本，已添加中英双语翻译
- [ ] Commit 信息符合 Conventional Commits 规范

### PR 描述模板

```markdown
## 概述

简述此 PR 的变更内容。

## 变更类型

- [ ] 新功能 (feat)
- [ ] Bug 修复 (fix)
- [ ] 重构 (refactor)
- [ ] 文档 (docs)
- [ ] 其他

## 测试

- [ ] 已添加/更新单元测试
- [ ] 已手动验证功能
- [ ] `cargo test --features full` 通过

## 影响范围

列出受影响的模块或功能。
```

---

## 版本发布

### 版本号规则

遵循 [SemVer](https://semver.org/lang/zh-CN/)（语义化版本）：

- **补丁版本**（0.2.x → 0.2.x+1）：Bug 修复
- **次版本**（0.2.x → 0.3.0）：向后兼容的新功能
- **主版本**（0.x.x → 1.0.0）：不兼容的 API 变更

### 发布步骤

1. 更新 `Cargo.toml` 中的 `version` 字段
2. 更新 `README.md` 中的版本号（如有）
3. 提交：`chore: bump to vX.Y.Z`
4. 打 tag：`git tag vX.Y.Z`
5. 推送 tag：`git push origin vX.Y.Z`
6. CI 自动构建并发布到 GitHub Releases

---

## 获取帮助

- **Issue**：在 [GitHub Issues](https://github.com/c2j/grep-excel/issues) 提交问题
- **讨论**：在 [GitHub Discussions](https://github.com/c2j/grep-excel/discussions) 发起讨论
- **开发文档**：参考 [docs/DeveloperGuide.md](docs/DeveloperGuide.md)
