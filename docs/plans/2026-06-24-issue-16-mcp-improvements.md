# Issue #16 MCP Improvements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Address GitHub issue #16 by adding field-level schemars descriptions (root cause of #1), clarifying docs (#2, #5 already exist), and adding four genuinely new capabilities: context-line search (#3 partial), sheet statistics (#7 partial), SQL-export, and minimal multi-condition search (#3 partial).

**Architecture:** Extend the existing `SearchEngine` trait with new methods where needed, implement across `MemEngine` (default) and `DuckDbEngine` (feature-gated), expose via MCP using the established rmcp `#[tool]` pattern. All new schema fields use the project's existing `#[cfg_attr(feature = "mcp-server", schemars(description = "..."))]` convention.

**Tech Stack:** Rust, rmcp (MCP), schemars 1.x (JSON schema generation), serde, existing multi-engine infrastructure (`MemEngine`, `DuckDbEngine`, `SqliteEngine`).

---

## Existing Codebase Context

### Key Files
- `crates/core/src/types.rs` — All shared types including MCP param structs (242 lines)
- `crates/core/src/engine/mod.rs` — `SearchEngine` trait + shared helpers (`find_matched_columns`, `like_match`, `validate_sql`, `write_xlsx`) (349 lines)
- `crates/core/src/engine/memory.rs` — `MemEngine` (default; 507 lines)
- `crates/core/src/engine/duckdb.rs` — `DuckDbEngine` (feature-gated; ~1348 lines)
- `crates/core/src/engine/sqlite.rs` — `SqliteEngine` (feature-gated)
- `crates/cli/src/mcp.rs` — MCP server with `#[tool]` handlers + Mcp-prefixed response types (601 lines)
- `README.md` — Project documentation

### Conventions (MANDATORY to follow)

**MCP param struct pattern** (from `SearchParams` in types.rs:117-132):
```rust
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct SearchParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Search query string"))]
    pub query: String,
    // ...
}
```

**MCP tool handler pattern** (from mcp.rs):
```rust
#[tool(description = "...")]
pub async fn tool_name(
    &self,
    Parameters(params): Parameters<XxxParams>,
) -> Result<String, String> {
    let db = Arc::clone(&self.db);
    tokio::task::spawn_blocking(move || {
        let guard = db.read();  // or db.write() for mutations
        guard.0.engine_method(...)
            .map(|r| serde_json::to_string_pretty(&McpType::from(r)).unwrap_or_else(|_| "...".into()))
            .map_err(|e| format!("Failed: {}", e))
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}
```

**Engine trait extension pattern** (from engine/mod.rs):
```rust
fn method_name(&self, ...) -> Result<ReturnType>;
```
Then implement in MemEngine (always) and DuckDbEngine + SqliteEngine (feature-gated).

### Testing Strategy

**No active test infrastructure** — the orphaned `tests/integration_test.rs` at workspace root is dead code (references nonexistent `grep_excel::database` module, isn't in any `[package]`).

**Verification approach for this plan:**
1. `cargo check --features mcp-server` — compile verification
2. `cargo check --features full` — full build verification
3. Add `#[cfg(test)] mod tests { ... }` inline unit tests in modified `src/` files for pure logic
4. MCP CLI smoke test: `cargo run --features mcp-server -- exec '{"tool":"...","params":{...}}'` against `test_data.xlsx`

---

## Out of Scope (explicitly rejected)

These issue points are NOT being implemented (with rationale):

| Issue Point | Reason Rejected |
|---|---|
| #1 "schema type bug" | Root cause already addressed in Task 1 — server schema is correct (`Option<usize>`); the missing field descriptions caused client confusion |
| #2 `get_sheet_preview` | **Already exists** as `get_sheet_sample` — documented in Task 2 |
| #4 standalone `filter`/`sort` MCP tools | **Redundant** with `execute_sql` (WHERE + ORDER BY). Adding them is duplicate API surface for the same SQL power |
| #5 SQL improvements | **Already addressed**: friendly aliases (`file.sheet`), JOIN, window functions all work via `execute_sql`. Documented in Task 2 |
| #6 `import_batch` | Low value — trivially worked around by multiple `import_file` calls. `update_cells` (batch update) already exists |
| #8 anomaly detection, smart fill suggestions | **Scope creep** — grep-excel is a search/query/export tool, not a data analysis platform |

---

## Task 1: P0 — Add field-level schemars descriptions to all MCP param structs

**Files:**
- Modify: `crates/core/src/types.rs:149-242`

**Why:** The actual root cause of issue #1. `SearchParams` has descriptions on every field; 8 other param structs have **none**, producing ambiguous JSON schemas that MCP clients (Cursor, Claude) may misinterpret (e.g., sending `"0"` as a string instead of `0` as a number).

**Step 1: Add schemars descriptions**

For each of these structs, add `#[cfg_attr(feature = "mcp-server", schemars(description = "..."))]` to every field, matching the `SearchParams` pattern:

- `GetSheetSampleParams` (line 149-155): `file_name`, `sheet_name`, `sample_size`
- `GetSheetDataParams` (line 157-165): `file_name`, `sheet_name`, `start_row`, `end_row`, `columns`
- `SaveAsParams` (line 167-173): `file_name`, `output_path`, `sheet_name`
- `SaveParams` (line 175-180): `file_name`, `sheet_name`
- `UpdateCellParams` (line 182-190): `file_name`, `sheet_name`, `row`, `column`, `value`
- `UpdateCellsParams` (line 192-198): `file_name`, `sheet_name`, `updates`
- `CellUpdate` (line 200-206): `row`, `column`, `value`
- `InsertRowsParams` (line 208-215): `file_name`, `sheet_name`, `start_row`, `rows`
- `DeleteRowsParams` (line 217-224): `file_name`, `sheet_name`, `start_row`, `count`
- `AddColumnParams` (line 226-233): `file_name`, `sheet_name`, `column_name`, `default_value`
- `RenameColumnParams` (line 235-242): `file_name`, `sheet_name`, `old_name`, `new_name`

**Suggested descriptions** (use clear, type-explicit language; mention 0-based indexing where relevant):

```rust
// GetSheetDataParams
file_name: "Name of the imported file (basename, e.g. \"data.xlsx\")"
sheet_name: "Name of the sheet within the file"
start_row: "0-based row index to start from (inclusive). Omit for beginning."
end_row: "0-based row index to end at (exclusive). Omit for through end."
columns: "Optional list of column names to include (others are filtered out)"
```

**Step 2: Verify compile**

Run: `cargo check --features mcp-server`
Expected: clean (no errors)

**Step 3: Commit**

```bash
git add crates/core/src/types.rs
git commit -m "fix(mcp): add field-level schemars descriptions to all param structs

Addresses root cause of issue #1: missing field descriptions caused MCP
clients to mis-serialize numeric fields as strings. SearchParams already
had descriptions; now all 11 param structs do."
```

---

## Task 2: P0 — Update README to clarify underutilized features

**Files:**
- Modify: `README.md` (both English and Chinese sections)

**Why:** Issue author missed that `get_sheet_sample` (preview) and friendly aliases (`file.sheet`) already exist. Documentation should make these discoverable.

**Step 1: Add "Data Preview" callout**

In the MCP Server Mode section, after the tool table, add a "Common Workflows" subsection:

```markdown
### Exploring Data Efficiently

**Preview large files without loading everything:**
Use `get_sheet_sample` to get evenly-spaced rows (default 10). This is the
fastest way to understand a sheet's structure without fetching all rows.

**Use friendly table aliases in SQL:**
Instead of internal names like `sheet_1_0`, use `filename.sheetname` syntax:
\`\`\`sql
SELECT * FROM data.Employees WHERE Department = 'Engineering'
\`\`\`
Run `--list-tables` or `list_files` MCP tool to discover available aliases.

**SQL already supports JOINs, window functions, and aggregations:**
\`\`\`sql
-- JOIN across files
SELECT e.Name, d.DeptName
FROM employees.Sheet1 e JOIN departments.Sheet1 d ON e.DeptId = d.Id

-- Window function
SELECT *, ROW_NUMBER() OVER (PARTITION BY DeptId ORDER BY Salary DESC) AS rank
FROM data.Employees

-- Aggregation
SELECT DeptId, COUNT(*) AS headcount FROM data.Employees GROUP BY DeptId
\`\`\`
```

**Step 2: Mirror the section in Chinese** (below the English, in the 中文 section)

**Step 3: Commit**

```bash
git add README.md
git commit -m "docs: clarify existing preview/alias/SQL features in README

Helps users discover get_sheet_sample (data preview), friendly table
aliases (file.sheet syntax), and SQL JOIN/window/aggregate capabilities.
Addresses the documentation gap underlying issue #16 points #2 and #5."
```

---

## Task 3: P1 — Add `context_lines` to search (grep -A/-B/-C style)

**Files:**
- Modify: `crates/core/src/types.rs` — extend `SearchQuery` and `SearchResult`
- Modify: `crates/core/src/engine/memory.rs` — implement context retrieval
- Modify: `crates/core/src/engine/duckdb.rs` — implement via `rowid BETWEEN`
- Modify: `crates/core/src/engine/sqlite.rs` — implement via `rowid BETWEEN`
- Modify: `crates/cli/src/mcp.rs` — add `context_lines` to `SearchParams`, extend `McpSearchResult`

**Why:** Issue #3 sub-request: "search_with_preview: 搜索时返回匹配行的上下文（前后各3行）". This is the grep-style context feature, genuinely missing.

**Step 1: Extend core types**

In `types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    pub column: Option<String>,
    pub mode: SearchMode,
    pub limit: usize,
    pub sheet: Option<String>,
    pub invert: bool,
    /// Number of rows to include before and after each match (grep -C style).
    /// 0 (default when None) means no context rows.
    #[serde(default)]
    pub context_lines: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextRows {
    pub before: Vec<Vec<String>>,  // rows immediately preceding the match, in order (nearest last)
    pub after: Vec<Vec<String>>,   // rows immediately following the match, in order (nearest first)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    // ... existing fields ...
    #[serde(default)]
    pub context: ContextRows,
}
```

**Step 2: Implement in MemEngine.search()**

In `memory.rs`, when building a `SearchResult` and `query.context_lines.unwrap_or(0) > 0`:
```rust
let n = query.context_lines.unwrap_or(0);
let start_ctx = row_idx.saturating_sub(n);
let end_ctx = (row_idx + n + 1).min(sheet.rows.len());
let before = sheet.rows[start_ctx..row_idx].to_vec();
let after = sheet.rows[row_idx+1..end_ctx].to_vec();
// populate SearchResult.context
```

**Step 3: Implement in DuckDbEngine.search()**

Two-stage approach in `duckdb.rs`:
1. Find matching `rowid`s via existing WHERE clause
2. For each match, run a second query: `SELECT ... FROM table WHERE rowid BETWEEN ? AND ?`
3. Or use a single query with window functions to include context

Simpler: for each match rowid `r`, fetch `WHERE rowid BETWEEN r-N AND r+N`, then split into before/match/after.

**Step 4: Mirror in SqliteEngine** (sqlite.rs has same rowid semantics)

**Step 5: Wire MCP**

In `mcp.rs`:
- Add `context_lines: Option<usize>` to `SearchParams`
- Pass it into `SearchQuery` construction in the `search` handler
- Add `before: Vec<Vec<String>>` and `after: Vec<Vec<String>>` to `McpSearchResult`
- Populate from `SearchResult.context` in the `From` impl

**Step 6: Update CLI search handler** (in main.rs) to construct SearchQuery with `context_lines: None` (default behavior unchanged)

**Step 7: Verify**

```bash
cargo check --features mcp-server
cargo check --features full
# MCP smoke test:
cargo run --features mcp-server -- test_data.xlsx --exec '{"tool":"search","params":{"query":"Engineering","context_lines":2}}'
```

Expected: search results include `before` and `after` arrays.

**Step 8: Commit**

```bash
git commit -m "feat(search): add context_lines for grep-style context rows

Implements issue #3 sub-request for context-line return. When context_lines
is N, each match includes N rows before and N rows after. Default behavior
(context_lines=None or 0) is unchanged — empty before/after arrays."
```

---

## Task 4: P1 — Add `get_sheet_statistics` MCP tool

**Files:**
- Modify: `crates/core/src/types.rs` — add stats result types
- Modify: `crates/core/src/engine/mod.rs` — add `get_sheet_statistics` to trait
- Modify: `crates/core/src/engine/memory.rs` — implement
- Modify: `crates/core/src/engine/duckdb.rs` — implement via SQL aggregations
- Modify: `crates/core/src/engine/sqlite.rs` — implement via SQL aggregations
- Modify: `crates/cli/src/mcp.rs` — add tool handler + MCP response type

**Why:** MCP-side gap — the CLI has `--aggregate` but the MCP API has no equivalent for quick column-distribution stats. Issue #7 sub-request.

**Step 1: Define types**

In `types.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStatistics {
    pub column_name: String,
    pub total_count: usize,
    pub non_null_count: usize,
    pub null_count: usize,
    pub distinct_count: usize,
    pub top_values: Vec<(String, usize)>,  // top 5 (value, count)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetStatistics {
    pub file_name: String,
    pub sheet_name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<ColumnStatistics>,
}
```

**Step 2: Add trait method**

In `engine/mod.rs`:
```rust
fn get_sheet_statistics(&self, file_name: &str, sheet_name: &str, max_top_values: usize) -> Result<SheetStatistics>;
```

**Step 3: MemEngine implementation**

For each column, iterate rows once collecting:
- null count (empty string or whitespace-only)
- distinct values via HashMap
- top N via sorting by count

**Step 4: DuckDbEngine implementation**

For each column, run:
```sql
SELECT 
  COUNT(*) AS total,
  COUNT(column_name) AS non_null,  -- DuckDB treats empty string as non-null
  COUNT(DISTINCT column_name) AS distinct_count
FROM table_name
```
And for top values:
```sql
SELECT column_name, COUNT(*) AS cnt
FROM table_name
GROUP BY column_name
ORDER BY cnt DESC
LIMIT ?
```

**Step 5: Mirror in SqliteEngine**

**Step 6: MCP wiring**

In `mcp.rs`, add new param struct (in types.rs):
```rust
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct GetSheetStatisticsParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "..."))]
    pub file_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "..."))]
    pub sheet_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Max number of top values to return per column (default 5)"))]
    pub max_top_values: Option<usize>,
}
```

Add McpSheetStatistics response type and From impl.

Add `#[tool]` handler:
```rust
#[tool(description = "Get per-column statistics for a sheet: null counts, distinct counts, top values. Useful for data profiling.")]
pub async fn get_sheet_statistics(...) -> Result<String, String> { ... }
```

**Step 7: Verify**

```bash
cargo check --features mcp-server
cargo run --features mcp-server -- test_data.xlsx --exec '{"tool":"get_sheet_statistics","params":{"file_name":"test_data.xlsx","sheet_name":"Employees"}}'
```

**Step 8: Commit**

```bash
git commit -m "feat(mcp): add get_sheet_statistics tool for data profiling

Returns per-column stats: total/non-null/null counts, distinct count, top N
values. MCP equivalent of CLI --aggregate. Addresses issue #7 sub-request."
```

---

## Task 5: P2 — Add `export_query` MCP tool

**Files:**
- Modify: `crates/core/src/types.rs` — add ExportQueryParams
- Modify: `crates/cli/src/mcp.rs` — add tool handler

**Why:** Issue #7 sub-request: export filtered data. Currently the only export path is `save_as` (full data). With `export_query`, users can run a SQL filter and save the result rows directly to xlsx. Reuses existing `execute_sql` + `write_xlsx`.

**Step 1: Add param struct** (in types.rs):
```rust
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct ExportQueryParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "SQL SELECT query whose result will be exported"))]
    pub sql: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Absolute or relative path for the output .xlsx file"))]
    pub output_path: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Sheet name in the output file (default: \"Sheet1\")"))]
    pub sheet_name: Option<String>,
}
```

**Step 2: MCP handler** (in mcp.rs):

Logic:
1. Call `engine.execute_sql(sql, limit=10000)` to get columns + rows
2. Call `write_xlsx(&[(sheet_name, &columns, &rows)], output_path)` to save
3. Return success message with row count

```rust
#[tool(description = "Run a SQL SELECT query and export the result rows to a new .xlsx file. Combines execute_sql + save_as for filtered exports.")]
pub async fn export_query(
    &self,
    Parameters(params): Parameters<ExportQueryParams>,
) -> Result<String, String> {
    let db = Arc::clone(&self.db);
    let result: Result<String, String> = tokio::task::spawn_blocking(move || {
        let guard = db.read();
        let sql_result = guard.0.execute_sql(&params.sql, 10000)
            .map_err(|e| format!("SQL execution failed: {}", e))?;
        if sql_result.rows.is_empty() {
            return Err("Query returned no rows; nothing to export".into());
        }
        let sheet_name = params.sheet_name.as_deref().unwrap_or("Sheet1");
        let sheet_tuple: (&str, &[String], &[Vec<String>]) = 
            (sheet_name, &sql_result.columns, &sql_result.rows);
        crate::engine::write_xlsx(&[sheet_tuple], std::path::Path::new(&params.output_path))
            .map(|_| format!("Exported {} rows to '{}'", sql_result.row_count, params.output_path))
            .map_err(|e| format!("Failed to write xlsx: {}", e))
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?;
    result
}
```

**Step 3: Verify**

```bash
cargo check --features mcp-server
cargo run --features mcp-server -- test_data.xlsx --exec '{"tool":"export_query","params":{"sql":"SELECT Name, Department FROM test_data.Employees WHERE Department = '\''Engineering'\''","output_path":"/tmp/eng.xlsx"}}'
```

**Step 4: Commit**

```bash
git commit -m "feat(mcp): add export_query tool for filtered exports

Runs a SQL SELECT and writes the result rows to a new .xlsx file. Combines
execute_sql + write_xlsx. Addresses issue #7 sub-request for filtered export."
```

---

## Task 6: P3 — Add multi-condition search (AND semantics, minimal scope)

**Files:**
- Modify: `crates/core/src/types.rs` — add `conditions` field to SearchQuery
- Modify: `crates/core/src/engine/memory.rs` — apply conditions as AND filter
- Modify: `crates/core/src/engine/duckdb.rs` — build AND where clause
- Modify: `crates/core/src/engine/sqlite.rs` — mirror duckdb
- Modify: `crates/cli/src/mcp.rs` — add `conditions` to SearchParams

**Why:** Issue #3 sub-request for multi-condition. **Scope deliberately limited to AND-only** (no OR between conditions) — keeps complexity low; OR logic is already available via `query` with regex `|` or via raw SQL.

**Step 1: Define condition type**

In `types.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct SearchCondition {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Column name to compare"))]
    pub column: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Comparison operator: =, !=, ILIKE, LIKE, >, <, >=, <="))]
    pub operator: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Value to compare against"))]
    pub value: String,
}
```

Extend `SearchQuery`:
```rust
pub struct SearchQuery {
    // ... existing ...
    #[serde(default)]
    pub conditions: Vec<SearchCondition>,  // empty = no extra conditions
}
```

Extend `SearchParams` (MCP):
```rust
pub struct SearchParams {
    // ... existing ...
    #[cfg_attr(feature = "mcp-server", schemars(description = "Additional AND conditions. Each is {column, operator, value}. Operators: =, !=, ILIKE, LIKE, >, <, >=, <="))]
    pub conditions: Option<Vec<SearchCondition>>,
}
```

**Step 2: Implement AND logic in MemEngine**

In `memory.rs` search(), before checking if a row matches the main query, first verify all conditions pass:
```rust
fn matches_conditions(row: &[String], headers: &[String], conditions: &[SearchCondition]) -> bool {
    for cond in conditions {
        let idx = match headers.iter().position(|h| h == &cond.column) {
            Some(i) => i,
            None => return false,  // column missing = condition fails
        };
        let val = row.get(idx).map(|s| s.as_str()).unwrap_or("");
        let matched = match cond.operator.as_str() {
            "=" | "==" => val == cond.value,
            "!=" | "<>" => val != cond.value,
            "ILIKE" => val.to_lowercase().contains(&cond.value.to_lowercase()),
            "LIKE" => like_match(&cond.value, val),
            ">" | "<" | ">=" | "<=" => {
                // try numeric compare, fall back to string
                match (val.parse::<f64>(), cond.value.parse::<f64>()) {
                    (Ok(a), Ok(b)) => match cond.operator.as_str() {
                        ">" => a > b, "<" => a < b, ">=" => a >= b, "<=" => a <= b, _ => false,
                    },
                    _ => false,
                }
            }
            _ => false,
        };
        if !matched { return false; }
    }
    true
}
```

Apply: `if !matches_conditions(row, &sheet.headers, &query.conditions) { continue; }`

**Step 3: DuckDbEngine implementation**

In `duckdb.rs` `build_wide_where_clause` (or a new helper), append conditions to the WHERE:
```rust
for cond in &query.conditions {
    let col = quote_ident(&cond.column);
    let clause = match cond.operator.as_str() {
        "=" | "==" => format!("{} = ?", col),
        "!=" | "<>" => format!("{} <> ?", col),
        "ILIKE" => format!("{} ILIKE ?", col),
        "LIKE" => format!("{} LIKE ?", col),
        ">" | "<" | ">=" | "<=" => format!("{} {} ?", col, cond.operator),
        _ => continue,  // skip unknown operators
    };
    and_parts.push(clause);
    values.push(cond.value.clone());
}
let and_clause = if and_parts.is_empty() { 
    None 
} else { 
    Some(format!("({})", and_parts.join(" AND "))) 
};
// Combine with the OR-matched clause via AND
```

**Step 4: Mirror in SqliteEngine** (use `LIKE` and `GLOB` instead of `ILIKE`)

**Step 5: MCP wiring**

Pass `conditions` through from `SearchParams` to `SearchQuery`.

**Step 6: Verify**

```bash
cargo check --features mcp-server
cargo run --features mcp-server -- test_data.xlsx --exec '{"tool":"search","params":{"query":"Eng","conditions":[{"column":"Department","operator":"=","value":"Engineering"},{"column":"City","operator":"ILIKE","value":"San"}]}}'
```

**Step 7: Commit**

```bash
git commit -m "feat(search): add multi-condition AND filtering

Adds optional conditions:[{column, operator, value}] to search. Conditions
are AND-combined. Operators: =, !=, ILIKE, LIKE, >, <, >=, <=. OR logic
remains available via regex | in the main query, or via raw SQL. Addresses
issue #3 multi-condition sub-request with minimal-scope AND semantics."
```

---

## Task 7: Final Verification

**Step 1: Full build matrix**

```bash
cargo check --features mcp-server
cargo check --features full
cargo build --features full --release
```

All must succeed without errors.

**Step 2: MCP smoke tests**

```bash
# P0 verification: schema now has field descriptions
cargo run --features mcp-server -- --mcp &
# Connect and verify tools/list output includes field descriptions

# P1 context_lines
cargo run --features mcp-server -- test_data.xlsx --exec '{"tool":"search","params":{"query":"Engineering","context_lines":2}}'

# P1 statistics
cargo run --features mcp-server -- test_data.xlsx --exec '{"tool":"get_sheet_statistics","params":{"file_name":"test_data.xlsx","sheet_name":"Employees"}}'

# P2 export_query
cargo run --features mcp-server -- test_data.xlsx --exec '{"tool":"export_query","params":{"sql":"SELECT * FROM test_data.Employees","output_path":"/tmp/test_export.xlsx"}}'

# P3 multi-condition
cargo run --features mcp-server -- test_data.xlsx --exec '{"tool":"search","params":{"query":"a","conditions":[{"column":"Department","operator":"=","value":"Engineering"}]}}'
```

**Step 3: Confirm no regressions**

```bash
# Existing search still works
cargo run --features mcp-server -- test_data.xlsx --exec '{"tool":"search","params":{"query":"Engineering"}}'
# Existing get_sheet_data still works (no string "0" required)
cargo run --features mcp-server -- test_data.xlsx --exec '{"tool":"get_sheet_data","params":{"file_name":"test_data.xlsx","sheet_name":"Employees","start_row":0,"end_row":3}}'
```

---

## Execution Approach

This plan will be executed via **subagent-driven-development**:
- Task 1 (P0 docs): direct execution (small, mechanical)
- Task 2 (P0 README): direct execution
- Tasks 3-6: delegate each to a `deep` agent in sequence (each touches types.rs + multiple engines, parallelization would conflict)
- Between each task: verify with `cargo check` before proceeding
- Final review uses `requesting-code-review` skill
