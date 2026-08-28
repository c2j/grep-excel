# AGENTS.md

grep-excel: Rust TUI/CLI/MCP tool for searching tabular files (Excel/CSV/HTML/text/Markdown/docx/pptx/archives) via pluggable SQL engines. Cargo workspace, version in root `[workspace.package]` (cli + core inherit it).

## Workspace layout

- `crates/cli` — package `grep-excel`, bin `grep_excel`. Entrypoints: `src/main.rs` (CLI parse + mode dispatch), `src/app/` (ratatui TUI), `src/mcp.rs` (MCP server), `src/interactive.rs` (SQL REPL). `src/bin/spike.rs` is an egui experiment, requires `--features gui`.
- `crates/core` — `grep-excel-core`: parsing (`excel.rs`, `html_table.rs`, `text_table.rs`, `docx_table.rs`, `pptx_table.rs`, `xml_table.rs`, `archive.rs`), `engine/` (`SearchEngine` trait + memory/duckdb/sqlite impls), `types.rs`, `i18n.rs`.
- `Desktop/src-tauri` — `grep-excel-desktop` (Tauri v1 + React/Vite/Tailwind frontend in `Desktop/`). **Separate version** (`0.3.3` in Cargo.toml and `tauri.conf.json`; not auto-bumped with workspace).

Dependency direction: `cli → core`, `desktop → core`. Never reverse.

## Build / test gotchas (read before running anything)

- **`cargo build` or `cargo test` at the workspace root builds ALL members**, including `grep-excel-desktop` whose default feature is `engine-duckdb` — that compiles the DuckDB C++ library (10–30 min) and needs Tauri/webkit system libs on Linux. For CLI work always scope: `cargo build -p grep-excel`, `cargo test -p grep-excel-core`.
- CLI default features = `engine-memory` + `file-dialog` (fast, no DuckDB). `full` = memory + file-dialog + mcp-server + share-url + archive-support + pdf-support + parquet-support — **still no DuckDB**. Release binaries use `--no-default-features --features file-dialog,engine-duckdb,mcp-server,share-url,pdf-support` (see `.github/workflows/release.yml`).
- Set `DUCKDB_DOWNLOAD_LIB=1` to download prebuilt DuckDB instead of compiling (works for `engine-duckdb`). `duckdb-bundled` always compiles from source. Avoid `cargo clippy --all-features` — it pulls `duckdb-bundled` + `gui`; prefer `cargo clippy -p grep-excel --features full -- -D warnings`.
- Engine is chosen at **compile time** via features; runtime `DefaultEngine` priority: DuckDB > SQLite > Memory.

Typical verification loop:
```bash
cargo fmt
cargo clippy -p grep-excel --features full -- -D warnings
cargo test -p grep-excel-core --features pdf-support,parquet-support,archive-support  # core tests, no duckdb
cargo test -p grep-excel --features full         # cli + REPL unit tests
cargo test -p grep-excel-core --test regress     # one test file
cargo test -p grep-excel-core regress_html_fragment   # single test
cargo run -p grep-excel --bin grep_excel -- tests/fixtures/manual/test_data2.xlsx -q "keyword"   # manual smoke
```

## Tests & fixtures

- Real tests: `crates/core/tests/*.rs` (regress, html_wdr, text_table, docx, pptx, xml, dbf, tsv, merged_cells, …) + `#[cfg(test)]` units in cli (`interactive.rs`, `main.rs`).
- Core tests reach workspace-root fixtures via `CARGO_MANIFEST_DIR` + `../..` (e.g. `tests/regress/awr.txt`); plain relative paths do NOT resolve from `crates/core`. Follow the `workspace_fixture()` helper in `text_table_test.rs`.
- `tests/regress/README.md` catalogs the HTML edge cases covered by `crates/core/tests/regress.rs`.
- Shared fixtures live under `tests/fixtures/` (pdf, parquet, manual, …) and `tests/regress/`. Manual smoke sample: `tests/fixtures/manual/test_data2.xlsx`.
- Local scratch (`test_attr`, `test.docx`, root `test_data.xlsx`) is gitignored — do not commit or rely on it in CI.

## Conventions

- **i18n mandatory**: all user-facing strings go through `i18n.rs` (Chinese + English). PR checklist requires translations for new text.
- **MCP param structs are shared**: every MCP tool's params live in `crates/core/src/types.rs` and are used by both MCP server and CLI `--exec`. Changing a tool interface means updating schema + README tool table together.
- New engine backend = implement `SearchEngine` (`crates/core/src/engine/mod.rs`); optional deps are feature-gated with `#[cfg(feature = "...")]` and must be wired through both `crates/core/Cargo.toml` and `crates/cli/Cargo.toml` feature tables.
- Commits: Conventional Commits, e.g. `feat(engine):`, `fix(archive):`, `docs:` (check `git log` for scope vocabulary). Branches: `feat/*`, `fix/*` from `main`.
- Dated design/impl plans are committed to `docs/plans/YYYY-MM-DD-topic.md` — check recent ones for in-flight work before large changes. `.sisyphus/` is gitignored local scaffolding.

## Docs: trust these

- Trust: `README.md` (user surface, English primary; `README.zh-CN.md` for Chinese), `docs/UserGuide.md` (end-user manual), `docs/DeveloperGuide.md` (architecture, SearchEngine trait, MCP dev), root `CONTRIBUTING.md` (process, features, PR checklist), `AGENTS.md` (this file). Each main doc has a `.zh-CN.md` Chinese variant.
- Do not reintroduce foreign-project docs or Tauri template boilerplate under `docs/` or `Desktop/`.

## Release

1. Bump `version` only in root `Cargo.toml` `[workspace.package]` (cli + core inherit; desktop version is independent — keep `Desktop/src-tauri/Cargo.toml` and `tauri.conf.json` in sync with each other).
2. `git tag vX.Y.Z && git push origin vX.Y.Z` → `release.yml` builds: Windows (prebuilt duckdb.dll via `DUCKDB_LIB_DIR`), Linux x86_64/aarch64 via `cargo zigbuild --target <t>.2.31` (glibc 2.31, `duckdb-bundled`), Win7 via nightly `-Zbuild-std --target x86_64-win7-windows-msvc`, plus Tauri desktop artifacts (`custom-protocol`; Linux adds `duckdb-bundled`).

## TDD 工作流（Red → Green → Refactor）

本仓库采用测试驱动开发。一次循环只锁定一个行为：先写会失败的测试（Red），再写最小实现让它通过（Green），最后在测试全绿的前提下重构（Refactor）。探索草稿不得直接合入，必须按本文件用 TDD 重写。

### 先读再改
1. 确认改动落在哪个 crate（本仓库是 Cargo workspace，见「仓库地图」）。
2. 只用本文件列出的 cargo 命令；不要发明裸 `cargo update`、不要擅自切换 toolchain（以 `rust-toolchain.toml` 为准）。
3. 先跑与改动相关的最小测试；提交前再跑 workspace 门禁（fmt + clippy + test）。
4. 完成一个循环后按「完成标准与汇报」汇报，不要只说「做完了」。

### Never / Ask first / Always

**Never（不必请示，直接禁止）**
- 删除、注释、跳过已有测试：`#[ignore]`、注释掉 `#[test]`、把断言改成 `is_ok()` / `unwrap()` 了事
- 修改人类已有测试的断言来迁就实现
- 先提交无测试的业务行为，再「回头补」
- 写永真测试：无断言、只检查 `is_some()`、只 verify 调用次数不查参数与状态
- 用全量端到端测试覆盖本可单测完成的改动
- 提交半成品；每次对人类可见的结果必须能构建且相关测试为绿
- 把探索草稿、临时脚本、调试 `dbg!`/`println!` 留在主代码

**Ask first**
- 改人类已有测试（含断言、fixture、snapshot）
- 新增运行时依赖、`unsafe`、新的 workspace crate、新的外部服务
- 为不可测代码做超出当前改动路径的重构
- 接受/更新 snapshot（insta / golden file）且行为含义发生变化
- 关闭 clippy lint、新增 `#[allow]`

**Always**
- 改遗留路径前：先写特征测试，锁定当前可观察行为（允许丑，必须可重复）
- 新行为：先有会失败的行为断言，再写最少实现
- 难以测试时：先造接缝，再写测试（见「遗留代码与接缝」）
- 测试名描述行为：`should_reject_negative_amount`
- 现有测试因你的改动失败：修实现，不修测试（除非人类明确要求）

测试权限：

| 测试来源 | 权限 |
|---|---|
| 人类已有测试 | 只读 |
| 本任务新建测试 | 可改，直到该行为稳定 |
| 过时或环境偶发失败 | 只报告，不擅自跳过 |

### 工作流

**Red** — 写生产行为之前先写测试；测试必须能被收集且必须失败（断言失败，或因缺失 API 导致编译失败，二者都算合法 Red）。修改已有功能先写特征测试锁定当前输出。一次只加一个行为的测试。

**Green** — 只写让当前失败测试通过的最少代码。禁止删掉/改掉失败测试、一次引入多个未验证变更、用更宽断言或 `unwrap()` 换绿。

**Refactor** — 相关测试全绿后才重构；重构后立刻跑同一组测试；范围限于当前 crate。

**探索 vs 实现** — 需求或方案不清可写草稿验证；草稿不得合并；方案确定后必须走 TDD 重写。

### 遗留代码与接缝

**特征测试** — 锁定现有行为，不是证明它正确。用固定 fixture 或 `insta` snapshot。更新 snapshot 必须在汇报里写清 diff 含义；默认不接受「看起来差不多」。

**接缝（优先顺序，靠后的更差）**
1. trait + 泛型或 `impl Trait`，测试用假类型
2. 用类型去掉非法状态（enum / newtype），而不是在测试里补分支
3. 时钟、ID、熵、文件系统做成可注入依赖；测试用 `tempfile` / 内存实现
4. `unsafe` 不是接缝。新增 `unsafe` 必须 Ask first，并写 `SAFETY` 注释

只给即将修改的代码路径补测试，不要一次性给整个模块「补全覆盖率」。

### 测试分层

| 层级 | 位置 | 测什么 |
|---|---|---|
| 单元 | `src` 内 `#[cfg(test)] mod tests` | 模块不变量、错误类型、状态转换 |
| 集成 | `tests/*.rs` | 公共 API；不可访问私有项 |
| 文档测试 | `///` 示例 | 公共 API 必须可运行；禁止滥用 `no_run` |
| CLI/二进制 | 项目惯用方式 | 退出码与 stdout 契约 |
| 不变量 | `proptest`（项目已用时） | 往返解析、幂等、单调性 |
| 特征/快照 | `insta` 或固定 fixture | 遗留输出；接受 snapshot 必须说明 |

不要把本该测公共契约的内容塞进 `#[cfg(test)]` 去读私有字段。

Rust 的 Red 允许是：测试引用了尚不存在的类型/函数导致编译失败。不要为了先编译而写空 `todo!()` 实现再补测试——可以留 `todo!()` 仅作为 Green 的最小占位，且下一步必须替换。

### Rust Never 补遗
- 库代码（非 main/example/测试）用 `unwrap` / `expect` / `panic!` 做控制流
- 无必要 `unsafe`；有则必须 `SAFETY` 注释
- 一次性 `cargo update` 整个 lockfile
- 用 `#[allow(...)]` 静默应修复的 lint
- 为绿而改 snapshot 却不解释行为是否应该变

### 命令

```bash
# 单测（单 crate，按测试名过滤）
cargo test -p grep-excel-core <test_name>

# 默认引擎测试
cargo test

# 特定引擎测试
cargo test --features engine-duckdb
cargo test --features engine-sqlite

# 全功能测试（含 MCP）
cargo test --features full

# 提交前门禁
cargo fmt --all -- --check
cargo clippy --all --all-targets
cargo test
```

循环内只跑受影响 crate；提交前再 workspace。

### 完成标准与汇报

提交或交还人类前，确认：
- [ ] 新行为有失败→通过的测试
- [ ] 修改的遗留路径有特征测试
- [ ] 未删除、跳过、改写人类已有测试
- [ ] 已跑与改动匹配的门禁（fmt + clippy + test）
- [ ] `cargo fmt` 与 clippy 干净
- [ ] 没有把草稿、调试输出、无主 lockfile 大面积变更带上

每个 TDD 循环汇报：
1. 测试了什么行为（测试函数名）
2. 最小实现改了哪些文件
3. 是否重构、边界在哪
4. 实际执行的命令和结果（通过 / 失败原因；不要只写「测过了」）

### 质量判断（自我检查）
- 这条测试在实现写错时会失败吗？
- 我是否在测行为，而不是私有实现细节？
- 我是否用 skip、更宽断言、unwrap、snapshot 盲收换绿？
- 命令是否来自本文件，而不是我编的？
