[中文版](./CONTRIBUTING.zh-CN.md)

# Contributing Guide

Thanks for your interest in grep-excel! This document covers the project structure, development setup, build & test workflow, and contribution process.

For maintainer/agent build gotchas see [AGENTS.md](AGENTS.md). For architecture and extensibility see [docs/DeveloperGuide.md](docs/DeveloperGuide.md). For end-user usage see [docs/UserGuide.md](docs/UserGuide.md).

## Table of Contents

- [Project Structure](#project-structure)
- [Development Setup](#development-setup)
- [Build & Test](#build--test)
- [Feature Flags](#feature-flags)
- [Code Style & Conventions](#code-style--conventions)
- [Commit Process](#commit-process)
- [Pull Request Guidelines](#pull-request-guidelines)
- [Release Process](#release-process)

---

## Project Structure

```
grep-excel/
├── .github/workflows/     # ci.yml + release.yml
├── crates/
│   ├── cli/               # package grep-excel, bin grep_excel (TUI/CLI/MCP/REPL)
│   └── core/              # grep-excel-core (parsing, engines, types, i18n)
├── Desktop/               # Tauri v1 + React GUI (independent version)
├── docs/
│   ├── UserGuide.md
│   ├── DeveloperGuide.md
│   └── plans/             # committed design/implementation plans
├── tests/
│   ├── fixtures/          # shared samples (pdf, parquet, manual, etc.)
│   ├── regress/           # HTML/text regression fixtures
│   └── benchmark/         # performance scripts
├── AGENTS.md              # maintainer/agent build notes
├── CONTRIBUTING.md        # this file
├── LICENSE                # MIT
├── README.md
└── Cargo.toml             # workspace root
```

### Dependency Direction

```
cli ──► core
Desktop/src-tauri ──► core
```

**Never** let `core` depend on `cli` or Desktop. A new engine implements `SearchEngine` (`crates/core/src/engine/mod.rs`) and must be wired through the feature tables in both `crates/core` and `crates/cli`.

---

## Development Setup

### Prerequisites

- **Rust** 1.70+ ([rustup](https://rustup.rs) recommended)
- **Cargo**, **Git**
- For Desktop development: Node.js 16+ and the [Tauri v1 system prerequisites](https://tauri.app/v1/guides/getting-started/prerequisites)

### Clone

```bash
git clone https://github.com/c2j/grep-excel.git
cd grep-excel
```

### Editor

VS Code with rust-analyzer is recommended. Optional settings:

```json
{
  "rust-analyzer.cargo.features": ["full"]
}
```

---

## Build & Test

> **Important**: Running `cargo build` / `cargo test` / `cargo clippy` without a package name at the workspace root builds **all members**, including `grep-excel-desktop` (which defaults to `engine-duckdb`, compiling DuckDB can take 10–30 minutes, and Linux requires Tauri/webkit dependencies). Always scope CLI development with `-p`.

### Recommended Verification Loop

```bash
cargo fmt
cargo clippy -p grep-excel --features full -- -D warnings
cargo test -p grep-excel-core --features pdf-support,parquet-support,archive-support
cargo test -p grep-excel --features full
```

### Common Builds

```bash
# Default (in-memory engine + file dialog, fast)
cargo build -p grep-excel

# full = memory + file-dialog + mcp + share-url + archive + pdf + parquet (still no DuckDB)
cargo build -p grep-excel --features full

# DuckDB (recommended for production; download prebuilt lib to speed up)
DUCKDB_DOWNLOAD_LIB=1 cargo build -p grep-excel --no-default-features \
  --features file-dialog,engine-duckdb,mcp-server,share-url,pdf-support

# Avoid: cargo clippy --all-features (pulls duckdb-bundled + gui)
```

### Unit Tests & Regression

```bash
cargo test -p grep-excel-core --test regress
cargo test -p grep-excel-core regress_html_fragment
```

Core tests reach workspace-root fixtures via `CARGO_MANIFEST_DIR` + `../..` (see the `workspace_fixture()` helper in `crates/core/tests/text_table_test.rs`). Do not use bare relative paths resolved from `crates/core`.

### Manual Smoke Test

```bash
cargo run -p grep-excel --bin grep_excel -- tests/fixtures/manual/test_data2.xlsx -t
cargo run -p grep-excel --bin grep_excel -- tests/fixtures/manual/test_data2.xlsx -q "keyword"
```

Shared fixtures live under `tests/fixtures/` and `tests/regress/`. Local scratch files (`test_attr`, `test.docx`, etc.) are gitignored — do not commit them.

### Speeding Up DuckDB Builds

```bash
DUCKDB_DOWNLOAD_LIB=1 cargo build -p grep-excel --features engine-duckdb
# duckdb-bundled always compiles from source; do not enable it for CI/local dev by default
```

---

## Feature Flags

Defined in `crates/cli/Cargo.toml` (and forwarded to core):

| Flag | Description |
|------|-------------|
| `engine-memory` | Pure in-memory engine (default) |
| `engine-duckdb` | DuckDB engine |
| `duckdb-bundled` | DuckDB + bundled C library (self-contained, slow to compile) |
| `engine-sqlite` | SQLite engine |
| `file-dialog` | Native file picker dialog (default) |
| `mcp-server` | MCP server mode including xlsx write support for AI assistant integration |
| `share-url` | Cloud share URL import (kdocs.cn / enterprise domains) |
| `archive-support` | Archive support (zip, tar, etc.) |
| `pdf-support` | PDF table extraction |
| `parquet-support` | Parquet read-only |
| `gui` | egui experimental spike bin |
| `full` | memory + file-dialog + mcp + share-url + archive + pdf + parquet (**excludes** DuckDB) |

Design principle: keep the default lightweight; gate heavy dependencies behind features; use `#[cfg(feature = "...")]` for conditional compilation.

The engine is selected at **compile time**; at runtime `DefaultEngine` priority is: DuckDB > SQLite > Memory.

---

## Code Style & Conventions

- `cargo fmt` + `cargo clippy -p grep-excel --features full -- -D warnings`
- Avoid unnecessary `unsafe`
- Error handling: `anyhow` / `thiserror` (consistent with existing code)
- **i18n is mandatory**: all user-visible strings go through `crates/core/src/i18n.rs` (Chinese + English)
- **MCP params are shared**: tool parameter structs live in `crates/core/src/types.rs` and are shared by both the MCP server and CLI `--exec`; changing a tool interface requires updating the schema and the README tool table together
- Never use type-suppression to mask errors (e.g. `@ts-ignore` in TS; avoid unnecessary `as` casts on the Rust side)

---

## Commit Process

### Conventional Commits

```
<type>(<scope>): <description>
```

Common types: `feat` `fix` `refactor` `chore` `docs` `test` `ci` `style`
Scope examples: `engine` `archive` `cli` `mcp` `desktop` (check `git log` for the established vocabulary)

### Branches

- `main` — stable
- `feat/*` / `fix/*` / `refactor/*` — branched from `main`

### Workflow

1. Create a branch from `main`
2. Develop and self-test with the **scoped** commands above
3. Push and open a PR

---

## Pull Request Guidelines

Before submitting:

- [ ] `cargo fmt`
- [ ] `cargo clippy -p grep-excel --features full -- -D warnings`
- [ ] `cargo test -p grep-excel-core --features pdf-support,parquet-support,archive-support`
- [ ] `cargo test -p grep-excel --features full`
- [ ] New features have tests (where applicable)
- [ ] User-visible changes are reflected in `README.md` / `docs/UserGuide.md` / `docs/DeveloperGuide.md`
- [ ] MCP tool interface changes are reflected in types + README tool table
- [ ] New user-facing copy is bilingual in `i18n.rs`
- [ ] Commits follow Conventional Commits

### PR Description Template

```markdown
## Summary

Briefly describe the change.

## Type of Change

- [ ] feat
- [ ] fix
- [ ] refactor
- [ ] docs
- [ ] ci / chore
- [ ] other

## Testing

- [ ] Tests added/updated
- [ ] Manually verified
- [ ] Scoped cargo test / clippy passes

## Affected Areas

List the modules touched (cli / core / desktop / docs / ci).
```

---

## Release Process

- **cli + core**: bump only `version` under `[workspace.package]` in the root `Cargo.toml` (both inherit it)
- **Desktop**: independent version (`Desktop/src-tauri/Cargo.toml` and `tauri.conf.json`); not auto-bumped with the workspace
- Tagging: `git tag vX.Y.Z && git push origin vX.Y.Z` → `release.yml` builds across platforms

SemVer: choose patch / minor / major based on compatibility.

---

## Getting Help

- [GitHub Issues](https://github.com/c2j/grep-excel/issues)
- [docs/DeveloperGuide.md](docs/DeveloperGuide.md)
- [AGENTS.md](AGENTS.md)
