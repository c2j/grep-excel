# SQL Query Support Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add raw SQL query support to grep-excel, letting users execute SQL directly against imported Excel/CSV data through TUI, MCP, and CLI interfaces.

**Architecture:** Add `execute_sql()` to the `SearchEngine` trait. DuckDB and SQLite engines pass SQL directly to their respective databases; the memory engine returns an "unsupported" error. A new `AppMode::EditingSql` TUI mode lets users type and execute SQL. A new `execute_sql` MCP tool and `--sql` CLI flag provide non-interactive access. Only `SELECT` statements are allowed; DDL/DML is rejected before reaching the engine.

**Tech Stack:** Rust, duckdb crate, rusqlite crate, ratatui (TUI), rmcp (MCP), clap (CLI), i18n (zh/en)

---

## TODOs

- [ ] Task 1: Add `SqlResult` type to `types.rs`
- [ ] Task 2: Add `execute_sql` to `SearchEngine` trait in `engine/mod.rs`
- [ ] Task 3: Implement `execute_sql` in DuckDB engine
- [ ] Task 4: Implement `execute_sql` in SQLite engine
- [ ] Task 5: Implement `execute_sql` in Memory engine (unsupported error)
- [ ] Task 6: Add `SqlCompleted` event to `event.rs`
- [ ] Task 7: Add `AppMode::EditingSql`, SQL fields, and `execute_sql` method to `App`
- [ ] Task 8: Add SQL mode keybindings in `handlers.rs`
- [ ] Task 9: Draw SQL mode UI in `ui.rs`
- [ ] Task 10: Add i18n strings for SQL mode
- [ ] Task 11: Add `execute_sql` MCP tool
- [ ] Task 12: Add `--sql` CLI flag

---

## Final Verification Wave

- [ ] F1: Oracle reviews goal alignment — SQL works in TUI, MCP, CLI for both DuckDB and SQLite
- [ ] F2: Oracle reviews code quality — no unsafe, no unwrap, proper error handling
- [ ] F3: Oracle reviews security — only SELECT allowed, LIMIT enforced, no injection path
- [ ] F4: Hands-on QA — TUI SQL mode works, MCP tool returns results, CLI `--sql` works

---

### Task 1: Add `SqlResult` type to `types.rs`

**Files:**
- Modify: `src/types.rs`

**Step 1: Add the new type**

Add after `FileInfo` (line 52):

```rust
#[derive(Debug, Clone)]
pub struct SqlResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub truncated: bool,
    pub duration: std::time::Duration,
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: Success (type is defined but unused, no warnings in Rust for unused structs)

**Step 3: Commit**

```bash
git add src/types.rs
git commit -m "feat: add SqlResult type for SQL query results"
```

---

### Task 2: Add `execute_sql` to `SearchEngine` trait in `engine/mod.rs`

**Files:**
- Modify: `src/engine/mod.rs`

**Step 1: Add the trait method**

Add to `SearchEngine` trait (after `fn clear`, line 17):

```rust
fn execute_sql(&self, sql: &str, limit: usize) -> Result<crate::types::SqlResult>;
```

**Step 2: Add SQL validation helper**

Add after `quote_ident` function (after line 150):

```rust
/// Validate that SQL is a read-only SELECT statement.
/// Returns an error for DDL/DML or empty input.
pub fn validate_sql(sql: &str) -> Result<()> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        anyhow::bail!("SQL query is empty");
    }
    let upper = trimmed.to_uppercase();
    // Allow WITH ... SELECT (CTE) and plain SELECT
    if !upper.starts_with("SELECT") && !upper.starts_with("WITH") {
        anyhow::bail!(
            "Only SELECT statements are allowed. Your query starts with: {}",
            trimmed.split_whitespace().next().unwrap_or("")
        );
    }
    // Reject common DDL/DML keywords at statement start within the query
    for forbidden in &[
        "INSERT ", "UPDATE ", "DELETE ", "DROP ", "CREATE ", "ALTER ", "ATTACH ", "DETACH ",
        "COPY ", "EXPORT ", "PRAGMA ",
    ] {
        if upper.contains(forbidden) {
            anyhow::bail!(
                "Forbidden keyword found: {}. Only SELECT queries are allowed.",
                forbidden.trim()
            );
        }
    }
    Ok(())
}
```

**Step 3: Verify compilation**

Run: `cargo check`
Expected: FAIL — all three engines need to implement the new trait method. This is expected; we fix in Tasks 3-5.

---

### Task 3: Implement `execute_sql` in DuckDB engine

**Files:**
- Modify: `src/engine/duckdb.rs`

**Step 1: Implement the method**

Add `execute_sql` implementation to `DuckDbEngine` (after the `build_wide_where_clause` method, around line 557):

```rust
impl SearchEngine for DuckDbEngine {
    // ... existing methods ...

    fn execute_sql(&self, sql: &str, limit: usize) -> Result<SqlResult> {
        super::validate_sql(sql)?;
        let start = std::time::Instant::now();

        let limited_sql = format!("SELECT * FROM ({}) LIMIT {}", sql, limit);
        let mut stmt = self.conn.prepare(&limited_sql)?;
        let col_count = stmt.column_count();

        let columns: Vec<String> = (0..col_count)
            .map(|i| stmt.column_name(i).to_string())
            .collect();

        let rows: Vec<Vec<String>> = stmt
            .query_map([], |row| {
                (0..col_count)
                    .map(|i| row.get::<_, Option<String>>(i).map(|v| v.unwrap_or_default()))
                    .collect::<Result<Vec<_>, _>>()
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let row_count = rows.len();
        let truncated = row_count >= limit;
        let duration = start.elapsed();

        Ok(SqlResult {
            columns,
            rows,
            row_count,
            truncated,
            duration,
        })
    }
}
```

Note: Add `use crate::types::SqlResult;` if needed (it's already available via `use super::*;`).

**Step 2: Verify compilation**

Run: `cargo check --features engine-duckdb`
Expected: Success

**Step 3: Commit**

```bash
git add src/engine/duckdb.rs
git commit -m "feat(engine-duckdb): implement execute_sql"
```

---

### Task 4: Implement `execute_sql` in SQLite engine

**Files:**
- Modify: `src/engine/sqlite.rs`

**Step 1: Implement the method**

Add to `SearchEngine` impl for `SqliteEngine` (after the `clear` method, around line 533):

```rust
fn execute_sql(&self, sql: &str, limit: usize) -> Result<SqlResult> {
    super::validate_sql(sql)?;
    let start = std::time::Instant::now();

    let limited_sql = format!("SELECT * FROM ({}) LIMIT {}", sql, limit);
    let mut stmt = self.conn.prepare(&limited_sql)?;
    let col_count = stmt.column_count();

    let columns: Vec<String> = (0..col_count)
        .map(|i| stmt.column_name(i).to_string())
        .collect();

    let rows: Vec<Vec<String>> = stmt
        .query_map([], |row| {
            (0..col_count)
                .map(|i| row.get::<_, Option<String>>(i).map(|v| v.unwrap_or_default()))
                .collect::<Result<Vec<_>, _>>()
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let row_count = rows.len();
    let truncated = row_count >= limit;
    let duration = start.elapsed();

    Ok(SqlResult {
        columns,
        rows,
        row_count,
        truncated,
        duration,
    })
}
```

**Step 2: Verify compilation**

Run: `cargo check --features engine-sqlite`
Expected: Success

**Step 3: Commit**

```bash
git add src/engine/sqlite.rs
git commit -m "feat(engine-sqlite): implement execute_sql"
```

---

### Task 5: Implement `execute_sql` in Memory engine (unsupported error)

**Files:**
- Modify: `src/engine/memory.rs`

**Step 1: Implement the method**

Add to `SearchEngine` impl for `MemEngine` (after the `clear` method, around line 158):

```rust
fn execute_sql(&self, _sql: &str, _limit: usize) -> Result<SqlResult> {
    anyhow::bail!(
        "SQL queries are not supported with the memory engine. \
         Rebuild with --features engine-duckdb or engine-sqlite."
    );
}
```

**Step 2: Verify full compilation**

Run: `cargo check` (default features: engine-memory)
Expected: Success

```bash
cargo check --features engine-duckdb
cargo check --features engine-sqlite
```

All three should compile.

**Step 3: Commit**

```bash
git add src/engine/memory.rs src/engine/mod.rs
git commit -m "feat(engine-memory): implement execute_sql as unsupported; add validate_sql helper"
```

---

### Task 6: Add `SqlCompleted` event to `event.rs`

**Files:**
- Modify: `src/event.rs`

**Step 1: Add the event variant**

Add `SqlCompleted` to the `AppEvent` enum (after `SearchCompleted`, line 10):

```rust
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    FileImported(Result<FileInfo>),
    SearchCompleted(Result<(Vec<SearchResult>, SearchStats)>),
    SqlCompleted(Result<crate::types::SqlResult>),
    Progress(usize, usize),
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: FAIL — `handle_event` in `App` doesn't handle `SqlCompleted` yet. We'll fix in Task 7.

**Step 3: Commit (together with Task 7)**

---

### Task 7: Add `AppMode::EditingSql`, SQL fields, and `execute_sql` method to `App`

**Files:**
- Modify: `src/app/mod.rs`

**Step 1: Add `EditingSql` to AppMode**

In `src/app/mod.rs`, add variant to `AppMode` enum (line 31):

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppMode {
    Normal,
    EditingSearch,
    EditingColumn,
    EditingSql,
    SelectFile,
    Help,
    DetailPanel,
}
```

**Step 2: Add SQL fields to `App` struct**

Add to the `App` struct (after `result_limit`, line 57):

```rust
pub(crate) sql_input: Input,
pub(crate) sql_result: Option<crate::types::SqlResult>,
```

**Step 3: Initialize new fields in `App::new`**

In the `App` constructor, add initializations:

```rust
sql_input: Input::default(),
sql_result: None,
```

**Step 4: Add `execute_sql_query` method**

Add to `impl App` (after `execute_search` method, around line 161):

```rust
pub fn execute_sql_query(&mut self) {
    if self.sql_input.value().is_empty() {
        return;
    }

    self.loading = true;
    self.status_message = crate::i18n::status_executing_sql().to_string();
    self.error_message = None;

    let sql = self.sql_input.value().to_string();
    let limit = self.result_limit;
    let db = Arc::clone(&self.database);
    let tx = self.event_tx.clone();

    std::thread::spawn(move || {
        let result = {
            let db_guard = db.read();
            db_guard.0.execute_sql(&sql, limit)
        };
        let _ = tx.send(AppEvent::SqlCompleted(result));
    });
}
```

**Step 5: Handle `SqlCompleted` event in `handle_event`**

Add match arm to `handle_event` (after `SearchCompleted`, around line 226):

```rust
AppEvent::SqlCompleted(result) => {
    self.loading = false;
    match result {
        Ok(sql_result) => {
            let count = sql_result.row_count;
            let duration = sql_result.duration.as_secs_f64();
            let truncated = sql_result.truncated;
            self.sql_result = Some(sql_result);
            self.error_message = None;
            self.results.clear();
            self.results_by_sheet.clear();
            self.stats = None;
            self.tab_state = 0;
            self.col_offset = 0;
            self.table_state.select(Some(0));
            self.update_scroll_state();

            self.status_message = if truncated {
                crate::i18n::status_sql_truncated(count, self.result_limit, duration)
            } else {
                crate::i18n::status_sql_done(count, duration)
            };
        }
        Err(e) => {
            self.error_message = Some(crate::i18n::status_sql_error(&e.to_string()));
            self.status_message = crate::i18n::status_sql_failed().to_string();
        }
    }
}
```

**Step 6: Verify compilation**

Run: `cargo check`
Expected: Success (all variants handled, i18n functions defined in Task 10)

**Note:** This task should be committed together with Task 6 and Task 10 (i18n strings).

---

### Task 8: Add SQL mode keybindings in `handlers.rs`

**Files:**
- Modify: `src/app/handlers.rs`

**Step 1: Add `EditingSql` case to `handle_key_event`**

Add to the match in `handle_key_event` (line 8):

```rust
AppMode::EditingSql => self.handle_sql_edit_mode(key),
```

**Step 2: Add `S` keybinding in normal mode**

In `handle_normal_mode`, add before the `_ => {}` catch-all (around line 145):

```rust
KeyCode::Char('S') => {
    self.mode = AppMode::EditingSql;
}
```

**Step 3: Add `handle_sql_edit_mode` method**

Add to `impl App` in handlers.rs:

```rust
fn handle_sql_edit_mode(&mut self, key: crossterm::event::KeyEvent) {
    use crossterm::event::KeyCode;

    match key.code {
        KeyCode::Enter => {
            self.execute_sql_query();
            self.mode = AppMode::Normal;
        }
        KeyCode::Esc => {
            self.mode = AppMode::Normal;
        }
        _ => {
            self.sql_input
                .handle_event(&crossterm::event::Event::Key(key));
        }
    }
}
```

**Step 4: Also clear `sql_result` when pressing `d` (clear)**

In the `KeyCode::Char('d')` handler, add after clearing results:

```rust
self.sql_result = None;
```

**Step 5: Verify compilation**

Run: `cargo check`
Expected: Success

**Step 6: Commit**

```bash
git add src/app/handlers.rs src/app/mod.rs src/event.rs
git commit -m "feat(tui): add SQL editing mode, keybindings, and event handling"
```

---

### Task 9: Draw SQL mode UI in `ui.rs`

**Files:**
- Modify: `src/app/ui.rs`

**Step 1: Update `draw_title_bar` to include `EditingSql`**

Add to the match in `draw_title_bar` (around line 52):

```rust
AppMode::EditingSql => (crate::i18n::appmode_sql(), Color::Magenta),
```

**Step 2: Modify `draw_search_bar` to show SQL input when in SQL mode**

When `self.mode == AppMode::EditingSql`, replace the search bar with a full-width SQL input. Add this logic inside `draw_search_bar`:

At the very beginning of `draw_search_bar`, add an early return for SQL mode:

```rust
if self.mode == AppMode::EditingSql || !self.sql_input.value().is_empty() {
    // In SQL mode, show a full-width SQL input instead of search/column/mode
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(20),
        ])
        .split(area);

    let label_area = Rect {
        x: chunks[0].x,
        y: chunks[0].y + 1,
        width: chunks[0].width,
        height: 1,
    };
    let sql_label = Paragraph::new(crate::i18n::label_sql())
        .style(Style::default().fg(Color::Magenta))
        .alignment(Alignment::Center);
    frame.render_widget(sql_label, label_area);

    let border_color = if self.mode == AppMode::EditingSql {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    let scroll = self.sql_input.visual_scroll(chunks[1].width as usize);
    let sql_paragraph = Paragraph::new(self.sql_input.value())
        .style(Style::default().fg(Color::White))
        .scroll((0, scroll as u16))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(border_color)),
        );
    frame.render_widget(sql_paragraph, chunks[1]);

    if self.mode == AppMode::EditingSql {
        let cursor_pos = self.sql_input.visual_cursor();
        let cursor_x = chunks[1].x + (cursor_pos.saturating_sub(scroll)) as u16 + 1;
        let cursor_y = chunks[1].y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
    return;
}
```

**Step 3: Draw SQL results in `draw_results_table`**

When `self.sql_result.is_some()` and `self.results.is_empty()` and `self.search_input.value().is_empty()`, draw the SQL result table instead. Add at the beginning of `draw_results_table`, after the DetailPanel early return:

```rust
// SQL results take priority if present
if let Some(ref sql_result) = self.sql_result {
    if self.results.is_empty() && self.search_input.value().is_empty() {
        self.draw_sql_results(frame, area, sql_result);
        return;
    }
}
```

**Step 4: Add `draw_sql_results` method**

```rust
fn draw_sql_results(&mut self, frame: &mut Frame, area: Rect, sql_result: &crate::types::SqlResult) {
    if sql_result.rows.is_empty() {
        let paragraph = Paragraph::new(Line::from(Span::styled(
            crate::i18n::sql_no_results(),
            Style::default().fg(Color::Gray),
        )))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
        frame.render_widget(paragraph, area);
        return;
    }

    let col_names = &sql_result.columns;
    let total_cols = col_names.len();

    let col_widths: Vec<u16> = {
        let mut widths = vec![10u16; total_cols];
        for (i, name) in col_names.iter().enumerate() {
            widths[i] = UnicodeWidthStr::width(name.as_str()).clamp(4, 50) as u16;
        }
        for row in sql_result.rows.iter().take(100) {
            for (i, cell) in row.iter().enumerate() {
                if i < total_cols {
                    let w = UnicodeWidthStr::width(cell.as_str()).clamp(4, 50) as u16;
                    if w > widths[i] {
                        widths[i] = w;
                    }
                }
            }
        }
        widths
    };

    let available_width = area.width.saturating_sub(4);
    let mut visible_count = 0usize;
    let mut used: u16 = 0;
    for &w in &col_widths {
        if used + w > available_width {
            break;
        }
        used += w;
        visible_count += 1;
    }
    visible_count = visible_count.max(1);

    let mut header_cells: Vec<ratatui::widgets::Cell<'_>> = Vec::new();
    for name in col_names.iter().take(visible_count) {
        header_cells.push(
            ratatui::widgets::Cell::from(name.as_str()).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        );
    }
    let header_row = ratatui::widgets::Row::new(header_cells)
        .height(1)
        .bottom_margin(1);

    let rows: Vec<_> = sql_result
        .rows
        .iter()
        .map(|row| {
            let cells: Vec<ratatui::widgets::Cell<'_>> = row
                .iter()
                .take(visible_count)
                .map(|cell| {
                    ratatui::widgets::Cell::from(truncate_str(cell, 50))
                        .style(Style::default().fg(Color::White))
                })
                .collect();
            ratatui::widgets::Row::new(cells).height(1)
        })
        .collect();

    let constraints: Vec<Constraint> = col_widths
        .iter()
        .take(visible_count)
        .map(|&w| Constraint::Length(w))
        .collect();

    let table = Table::new(rows, constraints)
        .header(header_row)
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            crate::i18n::sql_results_title(sql_result.row_count),
            Style::default().fg(Color::Cyan),
        )))
        .row_highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    frame.render_stateful_widget(table, area, &mut self.table_state);
}
```

**Step 5: Update `draw_status_bar` hints for `EditingSql`**

Add to the hints match (after `EditingColumn`, around line 899):

```rust
AppMode::EditingSql => vec![
    Span::styled(" [Enter]", Style::default().fg(Color::Cyan)),
    Span::raw(crate::i18n::hint_execute()),
    Span::styled("[Esc]", Style::default().fg(Color::Cyan)),
    Span::raw(crate::i18n::hint_cancel()),
],
```

**Step 6: Add `S` to normal mode hints**

In the `AppMode::Normal` hints section, add after the `[s] export` hint:

```rust
Span::styled("[S]", Style::default().fg(Color::Cyan)),
Span::raw(crate::i18n::hint_sql()),
```

**Step 7: Update help popup**

In `draw_help_popup`, add a new help entry in the search group:

```rust
Line::from(vec![
    Span::styled("    S     ", key_style),
    Span::styled("···  ", sep_style),
    Span::styled(crate::i18n::help_search_sql(), desc_style),
]),
```

**Step 8: Verify compilation**

Run: `cargo check`
Expected: Success (assuming i18n strings from Task 10 are in place)

**Step 9: Commit**

```bash
git add src/app/ui.rs
git commit -m "feat(tui): draw SQL input bar and SQL results table"
```

---

### Task 10: Add i18n strings for SQL mode

**Files:**
- Modify: `src/i18n.rs`

**Step 1: Add all SQL-related i18n strings**

Add these functions to `src/i18n.rs` in appropriate sections:

```rust
// ── SQL mode ──

pub fn appmode_sql() -> &'static str {
    match current() { Lang::Zh => "SQL", Lang::En => "SQL" }
}

pub fn label_sql() -> &'static str {
    match current() { Lang::Zh => "[SQL]", Lang::En => "[SQL]" }
}

pub fn status_executing_sql() -> &'static str {
    match current() { Lang::Zh => "执行 SQL...", Lang::En => "Executing SQL..." }
}

pub fn status_sql_done(count: usize, duration: f64) -> String {
    match current() {
        Lang::Zh => format!("SQL 查询完成: {} 行, 用时 {:.2}s", count, duration),
        Lang::En => format!("SQL complete: {} rows, took {:.2}s", count, duration),
    }
}

pub fn status_sql_truncated(count: usize, limit: usize, duration: f64) -> String {
    match current() {
        Lang::Zh => format!("SQL 查询完成: {}+ 行 (显示前 {}), 用时 {:.2}s — [n] 加载更多", count, limit, duration),
        Lang::En => format!("SQL complete: {}+ rows (showing first {}), took {:.2}s — [n] load more", count, limit, duration),
    }
}

pub fn status_sql_error(e: &str) -> String {
    match current() {
        Lang::Zh => format!("SQL 错误: {}", e),
        Lang::En => format!("SQL error: {}", e),
    }
}

pub fn status_sql_failed() -> &'static str {
    match current() { Lang::Zh => "SQL 执行失败", Lang::En => "SQL execution failed" }
}

pub fn sql_no_results() -> &'static str {
    match current() { Lang::Zh => "SQL 查询无结果", Lang::En => "SQL query returned no results" }
}

pub fn sql_results_title(count: usize) -> String {
    match current() {
        Lang::Zh => format!(" SQL 结果 ({} 行) ", count),
        Lang::En => format!(" SQL Results ({} rows) ", count),
    }
}

pub fn hint_sql() -> &'static str {
    match current() { Lang::Zh => "SQL  ", Lang::En => "SQL  " }
}

pub fn help_search_sql() -> &'static str {
    match current() {
        Lang::Zh => "进入 SQL 查询模式",
        Lang::En => "Enter SQL query mode",
    }
}

// CLI SQL messages
pub fn cli_sql_failed(e: &str) -> String {
    match current() {
        Lang::Zh => format!("SQL 执行失败: {}", e),
        Lang::En => format!("SQL execution failed: {}", e),
    }
}

pub fn cli_sql_no_results() -> &'static str {
    match current() { Lang::Zh => "SQL 查询无结果", Lang::En => "SQL query returned no results" }
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: Success

**Step 3: Commit**

```bash
git add src/i18n.rs
git commit -m "feat(i18n): add SQL mode strings in zh/en"
```

---

### Task 11: Add `execute_sql` MCP tool

**Files:**
- Modify: `src/mcp.rs`

**Step 1: Add `SqlQueryParams` struct**

Add after `SearchParams` (line 34):

```rust
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SqlQueryParams {
    #[schemars(description = "SQL SELECT query to execute against imported data")]
    pub sql: String,
    #[schemars(description = "Maximum results to return (default: 1000)")]
    pub limit: Option<usize>,
}
```

**Step 2: Add `execute_sql` tool to `GrepExcelServer`**

Add to the `#[tool_router(server_handler)]` impl block:

```rust
#[tool(description = "Execute a SQL SELECT query against imported Excel/CSV data. Table names follow pattern: sheet_{file_id}_{sheet_idx}. Use list_files to see available tables.")]
pub async fn execute_sql(
    &self,
    Parameters(params): Parameters<SqlQueryParams>,
) -> Result<String, String> {
    let sql = params.sql;
    let limit = params.limit.unwrap_or(1000);
    let db = Arc::clone(&self.db);
    tokio::task::spawn_blocking(move || {
        let guard = db.read();
        guard
            .0
            .execute_sql(&sql, limit)
            .map(|result| {
                let output = McpSqlResult::from(result);
                serde_json::to_string_pretty(&output)
                    .unwrap_or_else(|_| "SQL query complete".to_string())
            })
            .map_err(|e| format!("SQL error: {}", e))
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}
```

**Step 3: Add `McpSqlResult` serialization type**

```rust
#[derive(Debug, Serialize)]
pub struct McpSqlResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub truncated: bool,
    pub duration_ms: u64,
}

impl From<crate::types::SqlResult> for McpSqlResult {
    fn from(r: crate::types::SqlResult) -> Self {
        McpSqlResult {
            columns: r.columns,
            rows: r.rows,
            row_count: r.row_count,
            truncated: r.truncated,
            duration_ms: r.duration.as_millis() as u64,
        }
    }
}
```

**Step 4: Verify compilation**

Run: `cargo check --features "engine-duckdb mcp-server"`
Expected: Success

**Step 5: Commit**

```bash
git add src/mcp.rs
git commit -m "feat(mcp): add execute_sql tool"
```

---

### Task 12: Add `--sql` CLI flag

**Files:**
- Modify: `src/main.rs`

**Step 1: Add `--sql` argument to `Args`**

Add to the `Args` struct (after `--export`, line 41):

```rust
#[arg(short = 'x', long, help = "Execute a SQL SELECT query against imported data")]
sql: Option<String>,
```

**Step 2: Add CLI SQL execution path**

In `main()`, modify the logic to check for `--sql` before `--query`:

```rust
if args.sql.is_some() {
    return run_sql_cli(&args);
}
if args.query.is_some() {
    return run_cli(&args);
}
```

**Step 3: Implement `run_sql_cli`**

Add the function after `run_cli`:

```rust
fn run_sql_cli(args: &Args) -> Result<()> {
    let mut db = DefaultEngine::new()?;

    for file in &args.files {
        if !file.exists() {
            eprintln!("{}", grep_excel::i18n::cli_file_not_found(&file.display().to_string()));
            continue;
        }
        match db.import_excel(file, &|_, _| {}) {
            Ok(info) => {
                eprintln!(
                    "{}",
                    grep_excel::i18n::cli_imported(&info.name, info.sheets.len(), info.total_rows)
                );
            }
            Err(e) => eprintln!(
                "{}",
                grep_excel::i18n::cli_import_failed(&file.display().to_string(), &e.to_string())
            ),
        }
    }

    let sql = args.sql.as_ref().unwrap();
    let result = match db.execute_sql(sql, 10000) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", grep_excel::i18n::cli_sql_failed(&e.to_string()));
            return Ok(());
        }
    };

    if result.rows.is_empty() {
        println!("{}", grep_excel::i18n::cli_sql_no_results());
        return Ok(());
    }

    // Print column header
    let widths: Vec<usize> = result
        .columns
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let name_w = UnicodeWidthStr::width(name.as_str());
            let max_data_w = result
                .rows
                .iter()
                .take(200)
                .filter_map(|r| r.get(i))
                .map(|c| UnicodeWidthStr::width(c.as_str()))
                .max()
                .unwrap_or(0);
            name_w.max(max_data_w).min(40)
        })
        .collect();

    print_header(&result.columns, &widths);
    print_separator(&widths);

    for row in &result.rows {
        let parts: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| pad_to(cell, widths.get(i).copied().unwrap_or(10)))
            .collect();
        println!("  {}", parts.join(" │ "));
    }

    println!();
    println!(
        "{}",
        grep_excel::i18n::cli_match_summary(
            result.row_count,
            result.row_count,
            result.duration.as_millis()
        )
    );

    Ok(())
}
```

**Step 4: Verify compilation**

Run: `cargo check`
Expected: Success

**Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat(cli): add --sql flag for direct SQL queries"
```

---

## Implementation Notes

### Table Naming Convention

Users need to know table names to write SQL. The convention is:
- `sheet_{file_id}_{sheet_idx}` where `file_id` is auto-incremented and `sheet_idx` is 0-based

To discover table names, users can:
- **TUI**: Press `o` to see loaded files (which shows sheets)
- **MCP**: Use `list_files` tool
- **CLI**: Run without `--query` to see imported files

A future enhancement could add a `list_tables` command that returns all table names and their schemas.

### Safety Model

1. **SQL validation** (`validate_sql`): Rejects non-SELECT statements before they reach the engine
2. **LIMIT enforcement**: All SQL is wrapped in `SELECT * FROM (...) LIMIT {limit}` to prevent unbounded results
3. **Read-only connection**: DuckDB/SQLite connections are in-memory and only used for reads during search
4. **Forbidden keywords**: INSERT, UPDATE, DELETE, DROP, CREATE, ALTER, ATTACH, DETACH, COPY, EXPORT, PRAGMA are all blocked

### Error Handling Strategy

- Empty SQL → "SQL query is empty"
- Non-SELECT → "Only SELECT statements are allowed"
- Forbidden keyword → "Forbidden keyword found: INSERT"
- Syntax error → Let the engine's native error bubble up (e.g., DuckDB: "Catalog Error: Table with name xxx does not exist")
- Memory engine → "SQL queries are not supported with the memory engine"
