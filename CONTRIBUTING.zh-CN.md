[English](./CONTRIBUTING.md)

# 贡献指南

感谢你对 grep-excel 的关注！本文档说明项目结构、开发环境、构建测试与提交流程。

维护者/Agent 构建陷阱见 [AGENTS.md](AGENTS.md)。架构与扩展见 [docs/DeveloperGuide.zh-CN.md](docs/DeveloperGuide.zh-CN.md)。用户用法见 [docs/UserGuide.zh-CN.md](docs/UserGuide.zh-CN.md)。

## 目录

- [项目结构](#项目结构)
- [开发环境搭建](#开发环境搭建)
- [构建与测试](#构建与测试)
- [Feature Flags](#feature-flags)
- [代码风格与约定](#代码风格与约定)
- [提交流程](#提交流程)
- [Pull Request 指南](#pull-request-指南)
- [版本发布](#版本发布)

---

## 项目结构

```
grep-excel/
├── .github/workflows/     # ci.yml + release.yml
├── crates/
│   ├── cli/               # package grep-excel, bin grep_excel（TUI/CLI/MCP/REPL）
│   └── core/              # grep-excel-core（解析、引擎、types、i18n）
├── Desktop/               # Tauri v1 + React GUI（独立版本号）
├── docs/
│   ├── UserGuide.md
│   ├── DeveloperGuide.md
│   └── plans/             # 已提交的设计/实现计划
├── tests/
│   ├── fixtures/          # 共享样例（pdf、parquet、manual 等）
│   ├── regress/           # HTML/文本回归 fixtures
│   └── benchmark/         # 性能脚本
├── AGENTS.md              # 维护者/Agent 构建注意点
├── CONTRIBUTING.md        # 本文件
├── LICENSE                # MIT
├── README.md
└── Cargo.toml             # workspace 根
```

### 依赖方向

```
cli ──► core
Desktop/src-tauri ──► core
```

**禁止** `core` 依赖 `cli` 或 Desktop。新引擎实现 `SearchEngine`（`crates/core/src/engine/mod.rs`），并在 `crates/core` 与 `crates/cli` 的 feature 表中接线。

---

## 开发环境搭建

### 前提条件

- **Rust** 1.70+（推荐 [rustup](https://rustup.rs)）
- **Cargo**、**Git**
- Desktop 开发另需 Node.js 16+ 与 [Tauri v1 系统依赖](https://tauri.app/v1/guides/getting-started/prerequisites)

### 克隆

```bash
git clone https://github.com/c2j/grep-excel.git
cd grep-excel
```

### 编辑器

推荐 VS Code + rust-analyzer。可选：

```json
{
  "rust-analyzer.cargo.features": ["full"]
}
```

---

## 构建与测试

> **重要**：在 workspace 根执行无包名的 `cargo build` / `cargo test` / `cargo clippy` 会构建 **全部成员**，包括 `grep-excel-desktop`（默认 `engine-duckdb`，编译 DuckDB 可达 10–30 分钟，且 Linux 需要 Tauri/webkit 依赖）。CLI 开发请始终加 `-p`。

### 推荐验证循环

```bash
cargo fmt
cargo clippy -p grep-excel --features full -- -D warnings
cargo test -p grep-excel-core --features pdf-support,parquet-support,archive-support
cargo test -p grep-excel --features full
```

### 常用构建

```bash
# 默认（内存引擎 + 文件对话框，快）
cargo build -p grep-excel

# full = memory + file-dialog + mcp + share-url + archive + pdf + parquet（仍不含 DuckDB）
cargo build -p grep-excel --features full

# DuckDB（生产推荐；可下载预编译库加速）
DUCKDB_DOWNLOAD_LIB=1 cargo build -p grep-excel --no-default-features \
  --features file-dialog,engine-duckdb,mcp-server,share-url,pdf-support

# 避免：cargo clippy --all-features（会拉 duckdb-bundled + gui）
```

### 单测与回归

```bash
cargo test -p grep-excel-core --test regress
cargo test -p grep-excel-core regress_html_fragment
```

Core 测试通过 `CARGO_MANIFEST_DIR` + `../..` 访问仓库根下 `tests/` fixtures（见 `crates/core/tests/text_table_test.rs` 的 `workspace_fixture()`）。不要用从 `crates/core` 出发的裸相对路径。

### 手动冒烟

```bash
cargo run -p grep-excel --bin grep_excel -- tests/fixtures/manual/test_data2.xlsx -t
cargo run -p grep-excel --bin grep_excel -- tests/fixtures/manual/test_data2.xlsx -q "keyword"
```

共享 fixtures：`tests/fixtures/`、`tests/regress/`。本地临时文件（`test_attr`、`test.docx` 等）已 gitignore，勿提交。

### 加速 DuckDB

```bash
DUCKDB_DOWNLOAD_LIB=1 cargo build -p grep-excel --features engine-duckdb
# duckdb-bundled 始终从源码编译，CI/本地开发默认不要开
```

---

## Feature Flags

定义在 `crates/cli/Cargo.toml`（并转发到 core）：

| Flag | 说明 |
|------|------|
| `engine-memory` | 纯内存引擎（默认） |
| `engine-duckdb` | DuckDB 引擎 |
| `duckdb-bundled` | DuckDB + 捆绑 C 库（自包含，编译慢） |
| `engine-sqlite` | SQLite 引擎 |
| `file-dialog` | 原生文件选择（默认） |
| `mcp-server` | MCP 服务器 + xlsx 写入相关依赖 |
| `share-url` | 云文档分享链接导入 |
| `archive-support` | zip/tar 等归档 |
| `pdf-support` | PDF 表格 |
| `parquet-support` | Parquet 只读 |
| `gui` | egui spike 实验 bin |
| `full` | memory + file-dialog + mcp + share-url + archive + pdf + parquet（**不含** DuckDB） |

设计原则：默认轻量；重型依赖按需 feature；`#[cfg(feature = "...")]` 条件编译。

引擎在 **编译期** 选择；运行时 `DefaultEngine` 优先级：DuckDB > SQLite > Memory。

---

## 代码风格与约定

- `cargo fmt` + `cargo clippy -p grep-excel --features full -- -D warnings`
- 避免不必要的 `unsafe`
- 错误处理：`anyhow` / `thiserror`（与现有代码一致）
- **i18n 强制**：用户可见字符串走 `crates/core/src/i18n.rs`（中/英）
- **MCP 参数共享**：工具参数结构体在 `crates/core/src/types.rs`，供 MCP 与 CLI `--exec` 共用；改接口须同步 schema 与 README 工具表
- 禁止用类型压制掩盖错误（如 TS 的 `@ts-ignore`；Rust 侧避免无必要的 `as` 绕过）

---

## 提交流程

### Conventional Commits

```
<type>(<scope>): <description>
```

常见 type：`feat` `fix` `refactor` `chore` `docs` `test` `ci` `style`  
scope 示例：`engine` `archive` `cli` `mcp` `desktop`（以 `git log` 为准）

### 分支

- `main` — 稳定
- `feat/*` / `fix/*` / `refactor/*` — 从 `main` 拉出

### 流程

1. 从 `main` 建分支  
2. 开发并用上文 **scoped** 命令自测  
3. 推送并开 PR  

---

## Pull Request 指南

提交前：

- [ ] `cargo fmt`
- [ ] `cargo clippy -p grep-excel --features full -- -D warnings`
- [ ] `cargo test -p grep-excel-core --features pdf-support,parquet-support,archive-support`
- [ ] `cargo test -p grep-excel --features full`
- [ ] 新功能有测试（如适用）
- [ ] 用户可见变更已更新 `README.md` / `docs/UserGuide.md`
- [ ] MCP 工具接口变更已更新 types + README 工具表
- [ ] 新用户文案已中英双语（`i18n.rs`）
- [ ] Commit 符合 Conventional Commits

### PR 描述模板

```markdown
## 概述

简述变更。

## 变更类型

- [ ] feat
- [ ] fix
- [ ] refactor
- [ ] docs
- [ ] ci / chore
- [ ] 其他

## 测试

- [ ] 已添加/更新测试
- [ ] 已手动验证
- [ ] scoped cargo test / clippy 通过

## 影响范围

列出模块（cli / core / desktop / docs / ci）。
```

---

## 版本发布

- **cli + core**：只改根 `Cargo.toml` 的 `[workspace.package] version`（二者 inherit）
- **Desktop**：独立版本（`Desktop/src-tauri/Cargo.toml` 与 `tauri.conf.json`），不随 workspace 自动 bump
- 打 tag：`git tag vX.Y.Z && git push origin vX.Y.Z` → `release.yml` 多平台构建

SemVer：补丁 / 次版本 / 主版本按兼容性选择。

---

## 获取帮助

- [GitHub Issues](https://github.com/c2j/grep-excel/issues)
- [docs/DeveloperGuide.zh-CN.md](docs/DeveloperGuide.zh-CN.md)
- [AGENTS.md](AGENTS.md)
