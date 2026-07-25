# OSS Repo Hygiene Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix open-source hygiene blockers (missing LICENSE, polluted docs, dead tests, thin gitignore, no PR CI) without changing product behavior or crate APIs.

**Architecture:** Pure repository hygiene. No Rust/TS business logic changes. Delete pollution and dead weight; add standard OSS files; rewrite the single contribution guide against the real `crates/*` layout; add a scoped PR CI workflow that avoids full-workspace DuckDB/Tauri builds.

**Tech Stack:** Git, GitHub Actions, MIT license text, Markdown docs, existing Cargo workspace (`crates/cli`, `crates/core`, `Desktop/src-tauri`).

**Out of scope (explicit non-goals):**
- Renaming `Desktop/` → `desktop/`
- Splitting large source files (`main.rs`, `duckdb.rs`, `ui.rs`)
- Archiving historical `docs/plans/*` (except no new pollution)
- Changing bin name `grep_excel` → `grep-excel`
- Removing `Desktop/src-tauri/Cargo.lock` (evaluate later; Tauri often keeps nested lock)
- Adding CODE_OF_CONDUCT / SECURITY / CHANGELOG / dependabot (follow-up PR)
- Committing unless user explicitly asks

**Success criteria:**
1. `LICENSE` exists; README / Desktop README links resolve
2. No Metamorphosis / template boilerplate docs remain tracked
3. No dead `tests/integration_test.rs`
4. Single contribution entry: root `CONTRIBUTING.md` with accurate `crates/*` tree
5. `.sisyphus/` fully untracked; `.gitignore` covers root clutter + OS junk
6. Root has no orphan `test_data*.xlsx`; one kept manual fixture under `tests/fixtures/manual/` if still useful
7. `.github/workflows/ci.yml` runs fmt + scoped clippy + scoped tests on PR/push to main
8. Desktop `authors`, license expression, and `tauri.conf.json` version aligned with `Desktop/src-tauri/Cargo.toml`
9. `AGENTS.md` / `docs/DeveloperGuide.md` links updated; no references to deleted pollution paths as “trust these”
10. Verification commands below pass (no product regression)

---

## Preconditions / facts (do not re-discover)

| Fact | Evidence |
|------|----------|
| No `LICENSE` file | `ls LICENSE` fails; README links to it |
| Workspace version | root `Cargo.toml` `[workspace.package] version = "0.7.5"`, `license = "MIT"` |
| Desktop version | `Desktop/src-tauri/Cargo.toml` `0.3.3`; `tauri.conf.json` `"version": "0.3.0"`; `authors = ["you"]`; `license = "MIT OR Apache-2.0"` |
| Pollution docs | `docs/CONTRIBUTING.md` = Metamorphosis; `docs/BEST-PRATICE.md` = generic + typo |
| Dead test | `tests/integration_test.rs` uses removed `grep_excel::database` |
| Tracked despite ignore | `.sisyphus/plans/2026-04-17-optimize-eav-to-wide-table.md` |
| Orphan xlsx | `test_data3/4/5.xlsx` have zero `*.rs` references; `test_data2.xlsx` only used by dead integration test + AGENTS smoke example |
| Real tests | `crates/core/tests/*` + fixtures under `tests/regress/`, `tests/fixtures/{pdf,parquet}/` |
| CI today | only `.github/workflows/release.yml` (tag push) |
| Build gotcha | root `cargo test` builds Desktop + DuckDB; CI must use `-p grep-excel` / `-p grep-excel-core` |

---

### Task 1: Add MIT LICENSE

**Files:**
- Create: `LICENSE`

**Step 1: Write standard MIT license**

Use the canonical MIT text. Copyright holder: match repo owner from `repository = "https://github.com/c2j/grep-excel"` → `c2j`. Year: `2024-2026` (or current year range if preferred; use `2024-2026`).

```text
MIT License

Copyright (c) 2024-2026 c2j

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

**Step 2: Verify**

```bash
test -f LICENSE && head -1 LICENSE
```

Expected: `MIT License`

**Step 3: Commit (only if user requested commits)**

```bash
git add LICENSE
git commit -m "docs: add MIT LICENSE"
```

---

### Task 2: Delete pollution and dead weight

**Files:**
- Delete: `docs/CONTRIBUTING.md`
- Delete: `docs/BEST-PRATICE.md`
- Delete: `tests/integration_test.rs`
- Delete: `Desktop/QUICKSTART.md`
- Delete: `Desktop/TEMPLATE_SUMMARY.md`
- Delete: `Desktop/PROJECT_STRUCTURE.md`
- Delete: `Desktop/FIXES.md`
- Untrack: `.sisyphus/plans/2026-04-17-optimize-eav-to-wide-table.md` via `git rm --cached` (keep local file optional; prefer remove from index only if file still useful locally, else `git rm`)

**Step 1: Remove tracked pollution**

```bash
git rm docs/CONTRIBUTING.md docs/BEST-PRATICE.md tests/integration_test.rs \
  Desktop/QUICKSTART.md Desktop/TEMPLATE_SUMMARY.md \
  Desktop/PROJECT_STRUCTURE.md Desktop/FIXES.md
git rm --cached .sisyphus/plans/2026-04-17-optimize-eav-to-wide-table.md
# If the only tracked sisyphus file is gone, ensure empty dirs not re-added
```

**Step 2: Verify nothing remains**

```bash
git ls-files docs/CONTRIBUTING.md docs/BEST-PRATICE.md tests/integration_test.rs \
  Desktop/QUICKSTART.md Desktop/TEMPLATE_SUMMARY.md Desktop/PROJECT_STRUCTURE.md \
  Desktop/FIXES.md '.sisyphus/*'
```

Expected: empty output

**Step 3: Commit (if requested)**

```bash
git commit -m "chore: remove stale docs, dead tests, and template leftovers"
```

---

### Task 3: Relocate / drop root manual Excel fixtures

**Files:**
- Move (if keeping one smoke fixture): `test_data2.xlsx` → `tests/fixtures/manual/test_data2.xlsx`
- Delete from git: `test_data3.xlsx`, `test_data4.xlsx`, `test_data5.xlsx`, `test_date.xlsx`
- Delete root copies after move
- Modify: `AGENTS.md` smoke path
- Do **not** rewrite historical `docs/plans/*` example paths (historical docs may still say `test_data.xlsx`)

**Decision rule:**
- Keep **only** `test_data2.xlsx` as the documented manual smoke fixture (smallest multi-sheet sample still referenced in AGENTS).
- Drop `test_data3/4/5/date` — zero production/test code references.

**Step 1: Move and delete**

```bash
mkdir -p tests/fixtures/manual
git mv test_data2.xlsx tests/fixtures/manual/test_data2.xlsx
git rm test_data3.xlsx test_data4.xlsx test_data5.xlsx test_date.xlsx
```

**Step 2: Update AGENTS.md smoke line**

Change:
```bash
cargo run -p grep-excel -- test_data2.xlsx -q "keyword"
```
To:
```bash
cargo run -p grep-excel -- tests/fixtures/manual/test_data2.xlsx -q "keyword"
```

Also fix AGENTS “Tests & fixtures” bullet that says root `test_data2..5.xlsx` are tracked manual-test data — point to `tests/fixtures/manual/` and note orphans removed.

**Step 3: Verify no remaining root xlsx tracked**

```bash
git ls-files '*.xlsx' | grep -v '^tests/' || true
ls test_data*.xlsx 2>/dev/null || echo "root clean"
```

Expected: no root tracked xlsx; working tree root clean of test_data*.xlsx

**Step 4: Commit (if requested)**

```bash
git commit -m "chore(test): move manual fixture under tests/fixtures/manual"
```

---

### Task 4: Expand `.gitignore`

**Files:**
- Modify: `.gitignore`

**Target content** (replace entire file):

```gitignore
# Rust
/target
**/target/

# Local manual / scratch (never commit)
test_attr
test.docx
test_data.xlsx
*.swp
*.swo
*~

# OS / editors
.DS_Store
.DS_Store?
._*
Thumbs.db
.idea/
.vscode/*
!.vscode/extensions.json

# Local agent / worktree scaffolding
.sisyphus/
.worktrees/

# Env / secrets
.env
.env.*
!.env.example

# Logs
*.log
```

Notes:
- Do **not** ignore `tests/fixtures/**` (real fixtures stay tracked).
- Desktop already has its own `.gitignore` for `node_modules/` — leave it.
- After edit, confirm `.sisyphus` still ignored: `git check-ignore -v .sisyphus/plans/foo.md`

**Commit (if requested):** `chore: expand root .gitignore`

---

### Task 5: Replace CONTRIBUTION.md with accurate CONTRIBUTING.md

**Files:**
- Delete: `CONTRIBUTION.md`
- Create: `CONTRIBUTING.md`
- Modify: `docs/DeveloperGuide.md` link at end (`CONTRIBUTION.md` → `CONTRIBUTING.md`)
- Modify: `AGENTS.md` “Docs: trust these” section

**Content requirements for `CONTRIBUTING.md`:**

1. Title: `# Contributing` (bilingual optional; Chinese is fine to match existing docs — prefer **Chinese primary** like old CONTRIBUTION.md, or bilingual short EN intro + CN body).
2. **Accurate tree** (must match reality):

```
grep-excel/
├── .github/workflows/     # ci.yml + release.yml
├── crates/
│   ├── cli/               # package grep-excel, bin grep_excel (TUI/CLI/MCP/REPL)
│   └── core/              # grep-excel-core (parsers, engines, types, i18n)
├── Desktop/               # Tauri v1 + React GUI (independent version)
├── docs/
│   ├── UserGuide.md
│   ├── DeveloperGuide.md
│   └── plans/             # dated design/impl plans
├── tests/
│   ├── fixtures/          # shared sample files (regress, pdf, parquet, manual)
│   ├── regress/           # HTML/text regression fixtures
│   └── benchmark/         # performance scripts
├── AGENTS.md              # agent/maintainer build gotchas
├── CONTRIBUTING.md
├── LICENSE
├── README.md
└── Cargo.toml             # workspace root
```

3. Dependency rule: `cli → core`, `desktop → core`; never reverse.
4. Build/test commands **scoped** (copy from AGENTS.md verification loop) — **forbid** recommending bare `cargo test` / `cargo clippy --all-features` as the default contributor path.
5. Feature flags table: refresh from current `crates/cli/Cargo.toml` (include archive/pdf/parquet/share-url if present; do not copy stale “full = only memory+dialog+mcp”).
6. Conventions: i18n mandatory; MCP params in `crates/core/src/types.rs`; Conventional Commits; branches `feat/*` `fix/*`.
7. PR checklist: fmt, clippy scoped, tests scoped, i18n, README/tool table if MCP interface changes.
8. Release: bump only root `[workspace.package].version` for cli/core; note Desktop version is independent.
9. Link to `docs/DeveloperGuide.md`, `docs/UserGuide.md`, `AGENTS.md`.
10. **Do not** link to deleted BEST-PRATICE or docs/CONTRIBUTING.

**AGENTS.md updates:**
- Trust `CONTRIBUTING.md` (not `CONTRIBUTION.md`).
- Remove “Ignore docs/CONTRIBUTING.md and docs/BEST-PRATICE.md” bullets **or** replace with “removed; do not reintroduce foreign-project docs”.
- Remove Desktop QUICKSTART boilerplate note (file deleted).
- Fix fixtures bullets per Task 3.

**DeveloperGuide.md:**
- Change link `../CONTRIBUTION.md` → `../CONTRIBUTING.md`
- Grep for any other `CONTRIBUTION` / `BEST-PRATICE` / `docs/CONTRIBUTING` references in **active** docs (README, DeveloperGuide, UserGuide, AGENTS) and fix. Historical `docs/plans/*` may keep old paths.

**Step verify:**

```bash
test ! -f CONTRIBUTION.md && test -f CONTRIBUTING.md
rg -n 'CONTRIBUTION\.md|BEST-PRATICE|docs/CONTRIBUTING' README.md AGENTS.md docs/DeveloperGuide.md docs/UserGuide.md CONTRIBUTING.md || true
```

Expected: no stale references in active docs.

**Commit (if requested):** `docs: replace CONTRIBUTION.md with accurate CONTRIBUTING.md`

---

### Task 6: Align Desktop metadata (no behavior change)

**Files:**
- Modify: `Desktop/src-tauri/Cargo.toml`
- Modify: `Desktop/src-tauri/tauri.conf.json`

**Changes:**
1. `authors = ["you"]` → `authors = ["c2j"]` (or omit authors if preferred; prefer real id matching LICENSE copyright).
2. `license = "MIT OR Apache-2.0"` → `license = "MIT"` (match workspace + LICENSE file).
3. `tauri.conf.json` `"version": "0.3.0"` → `"0.3.3"` to match `Cargo.toml` package version.
4. Do **not** force Desktop version to workspace `0.7.5` in this PR (document independence in CONTRIBUTING/AGENTS only).

**Verify:**

```bash
rg -n 'authors|license|version' Desktop/src-tauri/Cargo.toml Desktop/src-tauri/tauri.conf.json | head -20
```

**Commit (if requested):** `chore(desktop): align license, authors, and tauri version`

---

### Task 7: Add PR CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

**Requirements:**
- Triggers: `pull_request` + `push` to `main`
- Runner: `ubuntu-latest`
- Jobs (single job OK):
  1. Checkout
  2. Install Rust stable (dtolnay/rust-toolchain or actions-rs equivalent — prefer `dtolnay/rust-toolchain@stable` with components `rustfmt`, `clippy`)
  3. Cache cargo (`Swatinem/rust-cache@v2`)
  4. `cargo fmt --all -- --check`
  5. `cargo clippy -p grep-excel --features full -- -D warnings`
  6. `cargo test -p grep-excel-core --features pdf-support,parquet-support,archive-support`
  7. `cargo test -p grep-excel --features full`

Note: bare `cargo test -p grep-excel-core` fails `parse_simple_pdf` when `pdf-support` is off (pre-existing). CI must enable optional format features.

**Hard constraints:**
- **Never** `cargo test` / `cargo build` / `cargo clippy` without `-p` at workspace root (pulls Desktop + DuckDB).
- **Never** `--all-features` (pulls `duckdb-bundled` + `gui`).
- Do **not** build Desktop/Tauri in this CI job (no webkit deps).
- Do **not** enable `engine-duckdb` in this PR CI (keep fast; release.yml already covers DuckDB artifacts).

**Optional env:** none required for memory engine path.

**YAML sketch:**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - name: fmt
        run: cargo fmt --all -- --check
      - name: clippy (cli full, no duckdb)
        run: cargo clippy -p grep-excel --features full -- -D warnings
      - name: test core
        run: cargo test -p grep-excel-core
      - name: test cli
        run: cargo test -p grep-excel --features full
```

**Local dry-run (must pass before claiming done):**

```bash
cargo fmt --all -- --check
cargo clippy -p grep-excel --features full -- -D warnings
cargo test -p grep-excel-core
cargo test -p grep-excel --features full
```

**Commit (if requested):** `ci: add PR workflow for fmt, clippy, and scoped tests`

---

### Task 8: Final cross-link and consistency pass

**Files to grep and fix if needed:**
- `README.md` — LICENSE link already correct once file exists; no need to bloat README
- `Desktop/README.md` — LICENSE link; ensure it still points to `../LICENSE`
- `docs/DeveloperGuide.md` — release section mentions LICENSE in zip (OK)
- `AGENTS.md` — fully consistent with new layout

**Grep checklist (active surfaces only):**

```bash
rg -n 'CONTRIBUTION\.md|BEST-PRATICE|Metamorphosis|QUICKSTART\.md|TEMPLATE_SUMMARY|PROJECT_STRUCTURE\.md|FIXES\.md|test_data3|test_data4|test_data5|integration_test' \
  README.md AGENTS.md CONTRIBUTING.md docs/DeveloperGuide.md docs/UserGuide.md Desktop/README.md \
  .github/workflows/ || true
```

Expected: no hits (or only intentional historical notes).

**Verify Desktop README still valid** after deleting template docs (it should not link to QUICKSTART etc.).

---

### Task 9: Verification gate (mandatory before “done”)

Run from repo root:

```bash
# Structure
test -f LICENSE
test -f CONTRIBUTING.md
test ! -f CONTRIBUTION.md
test ! -f docs/CONTRIBUTING.md
test ! -f docs/BEST-PRATICE.md
test ! -f tests/integration_test.rs
test ! -f Desktop/QUICKSTART.md
test -f tests/fixtures/manual/test_data2.xlsx
test -f .github/workflows/ci.yml
git ls-files '.sisyphus/*' | wc -l   # expect 0

# Build/test (scoped)
cargo fmt --all -- --check
cargo clippy -p grep-excel --features full -- -D warnings
cargo test -p grep-excel-core
cargo test -p grep-excel --features full

# Smoke (optional if binary builds quickly with default features)
cargo run -p grep-excel -- tests/fixtures/manual/test_data2.xlsx -t
```

All must succeed. Do not claim completion without command evidence.

---

## Commit strategy (if user asks to commit)

Prefer **atomic conventional commits** matching repo style:

1. `docs: add MIT LICENSE`
2. `chore: remove stale docs, dead tests, and template leftovers`
3. `chore(test): move manual fixture under tests/fixtures/manual`
4. `chore: expand root .gitignore`
5. `docs: replace CONTRIBUTION.md with accurate CONTRIBUTING.md`
6. `chore(desktop): align license, authors, and tauri version`
7. `ci: add PR workflow for fmt, clippy, and scoped tests`

Or one squash commit if user prefers single PR commit:
`chore: open-source repo hygiene cleanup`

**Do not push or open PR unless user asks.**

---

## Risk register

| Risk | Mitigation |
|------|------------|
| CI fails on pre-existing clippy/fmt | Fix only issues blocking CI in touched scope; if pre-existing mass failures, report and either fix minimally or temporarily soften CI with issue filed — prefer fix if small |
| Contributors relied on root xlsx paths | Document new path in CONTRIBUTING + AGENTS; keep one fixture |
| `git rm --cached` .sisyphus confuses local agents | `.gitignore` already has `.sisyphus/`; local plans remain untracked |
| Desktop license change MIT-only | Matches root; no dual-license file existed anyway |
| fmt --all touches unrelated files | If huge unrelated fmt churn, run fmt only on intentional files OR accept full fmt as separate commit — prefer full `cargo fmt` once for consistency |

---

## Follow-ups (NOT this PR)

- `CHANGELOG.md`, `SECURITY.md`, dependabot
- `rustfmt.toml` / `.editorconfig` / `rust-toolchain.toml`
- Archive old `docs/plans/*`
- Rename `Desktop/` → `desktop/`
- Nested `Desktop/src-tauri/Cargo.lock` consolidation
- Source file splits

---

## Execution order summary

1. LICENSE  
2. Delete pollution + untrack `.sisyphus`  
3. Fixtures move/delete + AGENTS path  
4. `.gitignore`  
5. `CONTRIBUTING.md` + link fixes  
6. Desktop metadata  
7. `ci.yml`  
8. Verification gate  

**Stop after plan approval by Momus + user.** Do not implement until both agree.
