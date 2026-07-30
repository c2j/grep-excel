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
