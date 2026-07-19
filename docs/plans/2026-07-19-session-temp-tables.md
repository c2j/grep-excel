# Session Temp Tables (`materialize_query`) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let MCP / CLI `--exec` / `-i` users materialize a read-only SQL result into a named session table, reuse it in later `execute_sql` calls, and drop it when done — without opening general DDL/DML on `execute_sql`.

**Architecture:** Add privileged engine APIs `materialize_query` + `drop_temp_table` that internally run `CREATE [OR REPLACE] TABLE … AS <validated SELECT>` on the long-lived DuckDB/SQLite connection. Keep `validate_sql` unchanged (still blocks `CREATE`/`DROP`/`;`). Surface the APIs as two new MCP/`--exec` tools and two REPL dot-commands (`.let` / `.drop`). Temp tables appear in `list_table_aliases` / `.tables` / `-t` with alias `temp.<name>`. Memory engine returns a clear unsupported error (same posture as `execute_sql`).

**Tech Stack:** Rust, DuckDB / SQLite engines, shared MCP param structs in `crates/core/src/types.rs`, rmcp MCP server, clap CLI `--exec`, rustyline REPL, bilingual `i18n.rs`.

**Non-goals (explicit YAGNI):**
- Do **not** relax `validate_sql` to allow user DDL (rejected alternative E).
- Do **not** support multi-statement SQL or arbitrary `INSERT`/`UPDATE` into temp tables in v1.
- Do **not** add TUI / Desktop UI for this in v1.
- Do **not** add per-MCP-client isolation (existing process-wide `Arc<Mutex<Engine>>` stays; document the shared-session caveat).
- Do **not** rename or overload existing `SearchEngine::materialize` (virtual-file loader).

**Prior art / related plans:**
- `docs/plans/2026-04-22-sql-query-support.md` — `execute_sql` + `validate_sql` read-only boundary
- `docs/plans/2026-05-02-cli-exec-option.md` — `--exec` shares one engine across steps
- Internal CTAS/temp swap in `crates/core/src/engine/duckdb.rs` (`insert_rows` edit helper)

---

## Design decisions (locked)

### D1 — API shape (preferred over open DDL)

| Surface | Name | Role |
|---------|------|------|
| Trait | `materialize_query(&mut self, name, sql, opts) -> Result<TempTableInfo>` | Create/replace session table from SELECT |
| Trait | `drop_temp_table(&mut self, name) -> Result<()>` | Drop session table only |
| MCP / `--exec` | `materialize_query` | Tool wrapper |
| MCP / `--exec` | `drop_temp_table` | Tool wrapper |
| REPL | `.let <name> AS <sql>` | Ergonomic create |
| REPL | `.drop <name>` | Ergonomic drop |
| Discovery | extend `list_table_aliases` | Show temps with `temp.<name>` |

`execute_sql` remains **read-only** and **`&self`**. New methods are **`&mut self`**.

### D2 — Naming rules

User-facing `name` must match:

```text
^[A-Za-z_][A-Za-z0-9_]*$
```

- Max length: 64.
- Case-sensitive storage; collision checks are **case-insensitive** (match existing alias uniqueness spirit).
- Reserved / forbidden names (reject):
  - Anything starting with `sheet_` (internal import tables)
  - SQL keywords we care about is unnecessary if identifier-only; still reject empty and names containing `.` / `"` / whitespace
- Physical table name in engine: exactly the validated `name` (quoted via `quote_ident`).
- Friendly alias: `temp.<name>` (parallel to `{file_stem}.{sheet}`).
- SQL may reference either bare `"name"` or `"temp"."name"` / `temp.name` depending on how the alias VIEW is registered — **implementation must make both bare name and `temp.name` queryable**, matching import alias UX. Prefer:
  1. `CREATE TABLE <name> AS …`
  2. `CREATE OR REPLACE VIEW` / schema trick for `temp.<name>` **or** document bare name only + list alias as `temp.name` for display.

**Decision for implementer (pick one, document in code comment):**

- **Preferred:** bare table `name` is the only physical object; `list_table_aliases` reports `alias = format!("temp.{}", name)`. Users query `SELECT * FROM name` (and may quote). Display prefix `temp.` is informational like a namespace tag, not necessarily a DuckDB schema.
- **Alternative (if bare names collide with user habits):** create DuckDB schema `temp` and table `temp.name`. More complex; only if bare-name collisions with sheet aliases become real.

**Collision policy:**
- If `name` equals an existing **import** `table_name` or would shadow a friendly import alias stem in a confusing way → error.
- If `name` already exists as a **temp** table → replace when `replace: true` (default), else error.

### D3 — SQL validation boundary (security)

```
User SQL ──► validate_sql(sql) ──► CREATE TABLE … AS ( <sql> )
                  ▲
                  └── same gate as execute_sql; no CREATE/DROP/INSERT from user text
```

- Call existing `validate_sql` on the **source** query only.
- Engine-owned DDL strings are hardcoded / format! with `quote_ident(name)` only — never concatenate raw user identifiers without validation.
- **No LIMIT wrap** on materialize (unlike early execute_sql designs). Full result is stored. Optional safety: `max_rows: Option<usize>` — if set, run a counting precheck or `CREATE TABLE AS SELECT * FROM (<sql>) LIMIT max_rows` and return `truncated: true` in result metadata. **v1 default: no max_rows cap** (document memory risk); optional param reserved in params struct as `Option<usize>` for forward compat, ignored or enforced — **enforce if provided**.

### D4 — Engine support matrix

| Engine | Behavior |
|--------|----------|
| DuckDB | Full support |
| SQLite | Full support |
| Memory | `bail!("Session temp tables are not supported with the memory engine. Rebuild with --features engine-duckdb or engine-sqlite.")` |

`clear()` on DuckDB/SQLite must also drop all registered temp tables (and clear the in-engine registry).

### D5 — Catalog / discovery

Extend `TableAliasInfo` with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableKind {
    File, // imported sheet
    Temp, // session materialize_query result
}

// TableAliasInfo gains:
pub kind: TableKind, // default File for serde backward compat if needed
```

For temp rows:
- `table_name` = physical name
- `alias` = `temp.{name}`
- `file_name` = `"<temp>"` (sentinel string, documented)
- `sheet_name` = `name`
- `row_count` / `columns` from registry after CTAS

Engine keeps `HashMap<String, TempTableMeta>` (or catalog table `temp_tables`) so `list_table_aliases` can merge file sheets + temps. **Do not** insert fake rows into `files`/`sheets` if that breaks edit/save paths — prefer side registry.

### D6 — REPL ergonomics

```text
.let filtered AS SELECT * FROM data.Sheet1 WHERE status = 'open'
.let summary AS SELECT city, count(*) AS n FROM filtered GROUP BY city
SELECT * FROM summary ORDER BY n DESC;
.drop filtered
.drop summary
.tables   -- shows temp.* entries
```

- `.let` requires `AS` and a non-empty SQL remainder (multi-line: user finishes SQL with `;` in normal REPL flow — **dot-commands are single-line today**).  
  **Constraint:** `.let name AS <sql>` is **one physical line** in v1 (same as other dot-commands). For multi-line SQL, users use MCP/`execute_sql` + `materialize_query`, or keep using CTE. Document this.
- `.drop <name>` only drops **temp** tables; refusing to drop import tables is mandatory.
- Do **not** implement “materialize last_result” in v1 (last_result lacks original SQL; row re-insert is a different feature).

### D7 — MCP / `--exec` workflow (canonical)

```json
[
  {"tool":"import_file","params":{"file_path":"data.xlsx"}},
  {"tool":"materialize_query","params":{
    "name":"eng",
    "sql":"SELECT * FROM data.Employees WHERE Department = 'Engineering'"
  }},
  {"tool":"execute_sql","params":{
    "sql":"SELECT City, COUNT(*) AS n FROM eng GROUP BY City ORDER BY n DESC"
  }},
  {"tool":"drop_temp_table","params":{"name":"eng"}}
]
```

### D8 — i18n & docs

All new user-facing strings via `i18n.rs` (zh + en). Update:
- `README.md` English MCP tool table + tool count (17 → 19)
- Chinese feature bullet tool count
- `--exec` help tool lists in `main.rs` (en + zh)
- `repl_help()` multiline
- Optional short “Session temp tables” tip under MCP workflows (en; zh bullet if no full table)

### D9 — Out of scope follow-ups (mention only)

- Per-client temp namespaces for multi-tenant MCP
- `.let` multi-line SQL
- TUI binding
- Whitelisted user DDL mode (`--allow-temp-ddl`)

---

## Types (target shapes)

### `crates/core/src/types.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableKind {
    File,
    Temp,
}

// Extend TableAliasInfo:
//   pub kind: TableKind,  // #[serde(default = "default_file_kind")] if needed

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempTableInfo {
    pub name: String,
    pub alias: String,       // "temp.{name}"
    pub row_count: usize,
    pub columns: Vec<String>,
    pub replaced: bool,      // true if an existing temp was dropped first
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct MaterializeQueryParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Session table name: [A-Za-z_][A-Za-z0-9_]{0,63}"))]
    pub name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Read-only SQL (SELECT/WITH/...) whose full result is stored as a session table"))]
    pub sql: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Replace existing temp table with the same name (default true)"))]
    pub replace: Option<bool>,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Optional safety cap on rows materialized. MUST be a number, not a string."))]
    pub max_rows: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct DropTempTableParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the session temp table to drop"))]
    pub name: String,
}
```

### Trait additions — `crates/core/src/engine/mod.rs`

```rust
/// Materialize a read-only SQL result into a session-scoped table.
/// Source `sql` is validated with `validate_sql`. Does not open general DDL to callers.
fn materialize_query(
    &mut self,
    name: &str,
    sql: &str,
    replace: bool,
    max_rows: Option<usize>,
) -> Result<crate::types::TempTableInfo>;

/// Drop a session temp table previously created by `materialize_query`.
/// Must refuse to drop imported file tables.
fn drop_temp_table(&mut self, name: &str) -> Result<()>;
```

Shared helper (same file or small `temp_table.rs` module):

```rust
/// Validate user temp table name. Returns Ok(name) or bail with clear error.
pub fn validate_temp_table_name(name: &str) -> Result<&str>;
```

---

## TODOs

- [ ] Task 1: Types + `TableKind` + name validator + unit tests
- [ ] Task 2: Trait methods + Memory stubs
- [ ] Task 3: DuckDB `materialize_query` / `drop_temp_table` / `list_table_aliases` / `clear`
- [ ] Task 4: SQLite parity
- [ ] Task 5: Core integration tests
- [ ] Task 6: MCP tools
- [ ] Task 7: CLI `--exec` dispatch + help strings
- [ ] Task 8: REPL `.let` / `.drop` + i18n + `repl_help`
- [ ] Task 9: README + workflow docs
- [ ] Task 10: Final verification (fmt, clippy, tests)

---

### Task 1: Types, name validator, unit tests

**Files:**
- Modify: `crates/core/src/types.rs`
- Modify: `crates/core/src/engine/mod.rs` (add `validate_temp_table_name` + `#[cfg(test)]` module)
- Modify: any `TableAliasInfo` construction sites (duckdb/sqlite/memory `list_table_aliases`) to set `kind: TableKind::File`

**Step 1: Write failing unit tests for name validation**

In `engine/mod.rs` under `#[cfg(test)] mod temp_name_tests`:

```rust
#[test]
fn accepts_simple_names() {
    assert!(validate_temp_table_name("eng").is_ok());
    assert!(validate_temp_table_name("t1").is_ok());
    assert!(validate_temp_table_name("_x").is_ok());
}

#[test]
fn rejects_bad_names() {
    assert!(validate_temp_table_name("").is_err());
    assert!(validate_temp_table_name("1ab").is_err());
    assert!(validate_temp_table_name("a-b").is_err());
    assert!(validate_temp_table_name("sheet_1_0").is_err());
    assert!(validate_temp_table_name("a.b").is_err());
    assert!(validate_temp_table_name(&"a".repeat(65)).is_err());
}
```

**Step 2: Run tests — expect fail (fn missing)**

```bash
cargo test -p grep-excel-core temp_name_tests -- --nocapture
```

Expected: compile error / not found.

**Step 3: Implement types + validator**

- Add `TableKind`, `TempTableInfo`, params structs.
- Extend `TableAliasInfo` with `kind: TableKind` (default `File` via `#[serde(default)]` impl Default for TableKind = File).
- Implement `validate_temp_table_name`.

**Step 4: Fix all `TableAliasInfo { ... }` struct literals** to include `kind: TableKind::File`.

```bash
rg "TableAliasInfo \\{" -n
```

**Step 5: Run tests — expect pass**

```bash
cargo test -p grep-excel-core temp_name_tests
cargo check -p grep-excel-core
```

**Step 6: Commit**

```bash
git add crates/core/src/types.rs crates/core/src/engine/mod.rs crates/core/src/engine/*.rs
git commit -m "feat(engine): add temp table types and name validator"
```

---

### Task 2: Trait methods + Memory engine stubs

**Files:**
- Modify: `crates/core/src/engine/mod.rs` — trait
- Modify: `crates/core/src/engine/memory.rs` — stubs

**Step 1: Add trait methods** (after `execute_sql` / near `list_table_aliases`).

**Step 2: Memory implementations**

```rust
fn materialize_query(
    &mut self,
    _name: &str,
    _sql: &str,
    _replace: bool,
    _max_rows: Option<usize>,
) -> Result<crate::types::TempTableInfo> {
    anyhow::bail!(
        "Session temp tables are not supported with the memory engine. \
         Rebuild with --features engine-duckdb or engine-sqlite."
    )
}

fn drop_temp_table(&mut self, _name: &str) -> Result<()> {
    anyhow::bail!(
        "Session temp tables are not supported with the memory engine. \
         Rebuild with --features engine-duckdb or engine-sqlite."
    )
}
```

**Step 3: Verify compile**

```bash
cargo check -p grep-excel-core
# If default features lack duckdb/sqlite, still need duckdb/sqlite impls next —
# temporarily `todo!()` in duckdb/sqlite OR complete Task 3/4 before full check with features.
```

Prefer completing Task 3 immediately after trait add so workspace compiles with `engine-duckdb`.

**Step 4: Commit**

```bash
git commit -m "feat(engine): add materialize_query/drop_temp_table trait + memory stubs"
```

---

### Task 3: DuckDB implementation

**Files:**
- Modify: `crates/core/src/engine/duckdb.rs`

**Internal state:**

```rust
pub struct DuckDbEngine {
    conn: Connection,
    temp_tables: HashMap<String, TempTableMeta>, // key: lowercase for lookup? store canonical name
}

struct TempTableMeta {
    name: String,
    columns: Vec<String>,
    row_count: usize,
}
```

**Algorithm `materialize_query`:**

1. `validate_temp_table_name(name)?`
2. `validate_sql(sql)?`
3. If name collides with any import `table_name` from `sheets` catalog → bail (`"Name conflicts with imported table"`).
4. Let `exists_temp = self.temp_tables.contains_key(...)`.
5. If `exists_temp && !replace` → bail.
6. If `exists_temp && replace` → `DROP TABLE IF EXISTS {quote_ident(name)}` and remove from map.
7. Build CTAS:
   - If `max_rows` is `Some(n)`:  
     `CREATE TABLE {qname} AS SELECT * FROM ({sql}) LIMIT {n}`  
     (sql already validated; still no user-controlled identifier interpolation except qname)
   - Else:  
     `CREATE TABLE {qname} AS {sql}`  
     **Careful:** DuckDB accepts `CREATE TABLE t AS SELECT ...`. Do **not** wrap with outer `SELECT * FROM (...)` unless needed for LIMIT.
8. Introspect columns + row count:
   - `SELECT COUNT(*) FROM {qname}`
   - `PRAGMA table_info` / `DESCRIBE` / query `information_schema` — match existing duckdb patterns in codebase if any; else `LIMIT 0` query for column names.
9. Insert `TempTableMeta` into map.
10. Return `TempTableInfo { name, alias: format!("temp.{}", name), row_count, columns, replaced: exists_temp }`.

**Algorithm `drop_temp_table`:**

1. Validate name format (same rules).
2. If not in `temp_tables` → bail (`"Unknown temp table: ..."`).
3. Never drop if name is only in `sheets` and not in temp map.
4. `DROP TABLE IF EXISTS {qname}`; remove from map.

**`list_table_aliases`:** after building file aliases, append temp entries with `kind: TableKind::Temp`.

**`clear`:** for each temp name, `DROP TABLE IF EXISTS`; `temp_tables.clear()`; then existing clear logic.

**`new` / `with_path`:** init `temp_tables: HashMap::new()`.

**Step 1: Implement + compile**

```bash
cargo check -p grep-excel-core --features engine-duckdb
```

**Step 2: Commit**

```bash
git commit -m "feat(engine): DuckDB session temp tables via materialize_query"
```

---

### Task 4: SQLite implementation

**Files:**
- Modify: `crates/core/src/engine/sqlite.rs`

Mirror DuckDB:
- Same `temp_tables` HashMap on struct
- `CREATE TABLE … AS …` / `DROP TABLE`
- Collision check against sqlite sheets catalog
- `list_table_aliases` + `clear` updates

```bash
cargo check -p grep-excel-core --features engine-sqlite
```

**Commit:**

```bash
git commit -m "feat(engine): SQLite session temp tables via materialize_query"
```

---

### Task 5: Core integration tests

**Files:**
- Create: `crates/core/tests/temp_tables.rs`

Use in-memory engine with duckdb feature. Pattern from `text_table_test.rs` for fixtures if needed; for pure SQL temps, import a tiny CSV/xlsx from workspace or `import_sheets` with synthetic data.

**Minimum cases:**

1. `materialize_query` then `execute_sql("SELECT * FROM t")` returns expected rows.
2. Second materialize with `replace: true` overwrites; `replace: false` errors.
3. `drop_temp_table` then SELECT fails / list no longer shows it.
4. `drop_temp_table` on import table name → error (do not drop file data).
5. `validate_sql` still rejects `CREATE TABLE` via `execute_sql`.
6. Bad names rejected.
7. `list_table_aliases` includes `kind == Temp` and alias `temp.t`.
8. `clear()` removes temps.
9. `max_rows: Some(1)` truncates materialization (row_count ≤ 1).
10. Forbidden SQL in source (`INSERT…`) rejected before any DDL.

Gate tests:

```rust
#![cfg(feature = "engine-duckdb")]
// or cfg any duckdb/sqlite
```

**Run:**

```bash
cargo test -p grep-excel-core --features engine-duckdb --test temp_tables
```

**Commit:**

```bash
git commit -m "test(engine): session temp table materialize/drop coverage"
```

---

### Task 6: MCP tools

**Files:**
- Modify: `crates/cli/src/mcp.rs`
- Params already in `types.rs` (Task 1)

Add two `#[tool]` methods next to `execute_sql` / `export_query`:

```rust
#[tool(description = "Materialize a read-only SQL result into a named session temp table for reuse in later execute_sql calls. Source SQL must pass the same read-only checks as execute_sql. Name: [A-Za-z_][A-Za-z0-9_]*. Query the table by bare name; list_files/list aliases show temp.<name>. Does not write files. Not supported on the memory engine.")]
pub async fn materialize_query(...) -> Result<String, String> { /* spawn_blocking, lock db, call engine */ }

#[tool(description = "Drop a session temp table created by materialize_query. Cannot drop imported file tables.")]
pub async fn drop_temp_table(...) -> Result<String, String> { ... }
```

**Mutex note:** `materialize_query` needs `&mut` on engine — existing `Mutex<SyncDb>` already serializes; use `lock()` then call mut method (same as import/edit tools). Confirm `SyncDb` tuple field allows mut access like other mutating tools.

**Commit:**

```bash
git commit -m "feat(mcp): add materialize_query and drop_temp_table tools"
```

---

### Task 7: CLI `--exec` dispatch + help

**Files:**
- Modify: `crates/cli/src/main.rs`

1. Add match arms in `exec_dispatch` for `materialize_query` and `drop_temp_table` (deserialize params, call engine, pretty JSON / message).
2. Update unknown-tool list (~L2506).
3. Update `format_exec_output` tool allowlist if present (~L2210).
4. Update English + Chinese `--exec` help tool enumerations (~L1683, L1720, L1801, L1850 — verify with `rg "execute_sql, export_query"`).

**Manual smoke (after build):**

```bash
cargo run -p grep-excel --features full -- test_data2.xlsx --exec '[
  {"tool":"materialize_query","params":{"name":"t","sql":"SELECT * FROM sheet_1_0 LIMIT 5"}},
  {"tool":"execute_sql","params":{"sql":"SELECT COUNT(*) AS n FROM t"}},
  {"tool":"drop_temp_table","params":{"name":"t"}}
]'
```

(Adjust table name via `-t` first if needed.)

**Commit:**

```bash
git commit -m "feat(cli): wire materialize_query/drop_temp_table into --exec"
```

---

### Task 8: REPL `.let` / `.drop` + i18n

**Files:**
- Modify: `crates/cli/src/interactive.rs`
- Modify: `crates/core/src/i18n.rs`
- Modify: unit tests in `interactive.rs` `#[cfg(test)]` if any parse helpers added

**Parsing `.let`:**

```text
.let <name> AS <sql...>
```

- Split: strip `.let`, then find case-insensitive ` AS ` separator.
- Errors via i18n: missing AS, missing name, missing sql.

**`.drop <name>`:** single token name.

**i18n functions (Form A/B):**
- `repl_let_usage() -> &'static str`
- `repl_let_ok(name, rows, cols) -> String`
- `repl_drop_ok(name) -> String`
- `repl_drop_usage()`
- Update `repl_help()` with both commands (zh + en branches).

**Commit:**

```bash
git commit -m "feat(repl): add .let and .drop for session temp tables"
```

---

### Task 9: README documentation

**Files:**
- Modify: `README.md`

1. Bump “17 MCP tools” → “19 MCP tools” (en feature list + zh bullet).
2. English MCP tools table: add `materialize_query`, `drop_temp_table` rows.
3. Add short workflow under MCP tips / SQL section:

```markdown
**Multi-step SQL with session temp tables:**
Use `materialize_query` to store a SELECT result as a named table for later `execute_sql` calls in the same MCP/`--exec`/`-i` session. Prefer CTEs for single-shot logic; use temp tables when the intermediate result is reused across multiple tool calls. Drop with `drop_temp_table` when finished. Not available on the memory engine.
```

4. REPL examples: show `.let` / `.drop`.
5. `--exec` available-tools sentence: include new tools.
6. Chinese: at minimum bump count + one-sentence tip if no full tool table exists.

**Commit:**

```bash
git commit -m "docs: document session temp tables for MCP/CLI/REPL"
```

---

### Task 10: Final verification

**Commands (from AGENTS.md):**

```bash
cargo fmt
cargo clippy -p grep-excel --features full -- -D warnings
cargo test -p grep-excel-core --features engine-duckdb
cargo test -p grep-excel --features full
# optional sqlite:
cargo test -p grep-excel-core --features engine-sqlite --test temp_tables
```

**Manual matrix:**

| Mode | Check |
|------|--------|
| MCP | import → materialize → execute_sql on temp → drop |
| `--exec` | JSON array multi-step |
| `-i` | `.let` / SELECT / `.tables` / `.drop` |
| Memory build | clear error message |
| Security | `execute_sql("CREATE TABLE x AS SELECT 1")` still fails |

**Regression:** existing edit/save paths still ignore `"<temp>"` sentinel; `list_files` unchanged (temps are not files).

---

## Final Verification Wave

- [ ] F1: Goal alignment — temps work in MCP, `--exec`, `-i` on DuckDB (+ SQLite)
- [ ] F2: Security — `validate_sql` unchanged; no user DDL path; name quoting safe; cannot drop import tables
- [ ] F3: Engine parity — Memory fails loudly; DuckDB/SQLite pass tests
- [ ] F4: Docs/i18n — bilingual strings; README counts; tool lists updated
- [ ] F5: No collision with `SearchEngine::materialize` (virtual files)

---

## Risk register

| Risk | Mitigation |
|------|------------|
| Large CTAS OOM | Document; optional `max_rows`; DuckDB `memory_limit` already set |
| Name collision with imports | Explicit catalog check before CREATE |
| MCP multi-client shared temps | Document process-wide session; future namespace |
| `TableAliasInfo` field break | Add `kind` with serde default `File` |
| CTAS SQL dialect differences | Separate duckdb/sqlite tests; keep SQL subset = validate_sql |
| `.let` single-line only | Document; agents use MCP tool for complex SQL |
| Accidentally dropping file tables | `drop_temp_table` only touches `temp_tables` map |

---

## Implementation notes for the agent

1. **Never** change `validate_sql` forbidden list for this feature.
2. **Never** commit unless the user asks during execution (plan commits are for human/executing-plans sessions that were told to commit).
3. Scope builds: `cargo test -p grep-excel-core`, `cargo clippy -p grep-excel --features full` — do **not** bare `cargo test` at workspace root (pulls desktop/duckdb compile).
4. Follow Conventional Commits: `feat(engine):`, `feat(mcp):`, `feat(cli):`, `feat(repl):`, `docs:`, `test(engine):`.
5. After implementation, preferred verification skill: `verification-before-completion`.

---

## Execution handoff

Plan saved to `docs/plans/2026-07-19-session-temp-tables.md`.

**Two execution options:**

1. **Subagent-Driven (this session)** — fresh subagent per task, review between tasks  
2. **Parallel Session (separate)** — new session with `executing-plans` in a worktree  

Do not start implementation until Momus review findings are addressed and the user chooses an execution path.
