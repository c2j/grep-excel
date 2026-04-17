# EAV → Wide Table Optimization Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the EAV `cells` table with per-sheet wide tables in DuckDB to achieve 10x-100x query performance improvement.

**Architecture:** Instead of storing every cell as a separate row in a single `cells` table (`sheet_id, row_idx, col_idx, col_name, cell_value`), we create one DuckDB table per sheet with actual column names from the Excel header. Search becomes a single `SELECT * FROM sheet_table WHERE col LIKE '%term%'` instead of the current 3-query-per-sheet dance (find matching rows → fetch row data → find matched columns).

**Tech Stack:** Rust, DuckDB (duckdb crate), calamine (Excel parsing), existing ratatui TUI

---

## Design

### Current (EAV) vs New (Wide Table)

**Current:** One `cells` table, every cell = 1 row. Search requires:
1. `SELECT DISTINCT row_idx FROM cells WHERE sheet_id=? AND (cell_value ILIKE ?)` → find matching rows
2. `SELECT col_idx, cell_value, col_name FROM cells WHERE sheet_id=? AND row_idx=?` → fetch row data (per match!)
3. `SELECT col_idx FROM cells WHERE sheet_id=? AND row_idx=? AND (cell_value ILIKE ?)` → find which columns matched

**New:** One table per sheet (e.g., `sheet_1_Employees`), columns = Excel headers. Search requires:
1. `SELECT * FROM sheet_1_Employees WHERE "col1" ILIKE '%term%' OR "col2" ILIKE '%term%'` → single query, all data + matches in one pass

### Schema Changes

Keep `files` and `sheets` metadata tables. Replace `cells` EAV table with dynamically created per-sheet wide tables.

New `sheets` table adds `table_name TEXT` column to track the DuckDB table name for each sheet.

Wide table columns are named after Excel headers (quoted to handle spaces/special chars). All column types are `TEXT`.

### Search Logic

For each sheet, build a single SQL query:
- FullText: `WHERE "col" ILIKE '%term%'` (all columns OR'd)
- ExactMatch: `WHERE "col" = 'term'`
- Wildcard: `WHERE "col" LIKE 'pattern'`
- Column filter: only apply to the specified column

Results include a `rowid` pseudo-column for positioning, and we determine `matched_columns` by checking which columns contain the search term.

### Column Name Sanitization

DuckDB allows quoted identifiers. We'll quote all column names with double-quotes. For duplicate headers, append `_2`, `_3`, etc.

---

## Tasks

### Task 1: Update `database.rs` - Schema & Import

**Files:**
- Modify: `src/database.rs`
- Test: `tests/integration_test.rs`

**Step 1: Write the new schema (replace `cells` table creation)**

In `Database::new()`, remove the `cells` table creation. The schema becomes:

```rust
pub fn new() -> Result<Self> {
    let conn = Connection::open_in_memory()?;

    conn.execute_batch(
        "CREATE SEQUENCE IF NOT EXISTS file_id_seq START 1;
        CREATE SEQUENCE IF NOT EXISTS sheet_id_seq START 1;
        CREATE TABLE IF NOT EXISTS files (
            file_id INTEGER DEFAULT nextval('file_id_seq') PRIMARY KEY,
            file_name TEXT NOT NULL,
            imported_at TIMESTAMP DEFAULT current_timestamp
        );
        CREATE TABLE IF NOT EXISTS sheets (
            sheet_id INTEGER DEFAULT nextval('sheet_id_seq') PRIMARY KEY,
            file_id INTEGER NOT NULL REFERENCES files(file_id),
            sheet_name TEXT NOT NULL,
            table_name TEXT NOT NULL,
            row_count INTEGER DEFAULT 0,
            col_names TEXT[] DEFAULT []
        );",
    )?;

    Ok(Database { conn })
}
```

**Step 2: Add column name sanitization helper**

Add a helper to create valid DuckDB column names:

```rust
fn sanitize_col_names(headers: &[String]) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    headers
        .iter()
        .map(|h| {
            let base = if h.is_empty() {
                "column".to_string()
            } else {
                h.clone()
            };
            let count = seen.entry(base.clone()).and_modify(|c| *c += 1).or_insert(1);
            if *count == 1 {
                base
            } else {
                format!("{}_{}", base, count)
            }
        })
        .collect()
}

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
```

**Step 3: Rewrite `import_excel` to create wide tables**

```rust
pub fn import_excel(
    &mut self,
    path: &Path,
    progress_callback: impl Fn(usize, usize),
) -> Result<FileInfo> {
    let sheets = parse_excel(path)?;
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
    let total_rows: usize = sheets.iter().map(|s| s.rows.len()).sum();

    let sample = sheets.first().map(|s| FileSample {
        sheet_name: s.name.clone(),
        headers: s.headers.clone(),
        rows: s.rows.iter().take(3).cloned().collect(),
    });

    self.conn.execute("INSERT INTO files (file_name) VALUES (?)", params![&file_name])?;
    let file_id: i64 = self.conn.query_row("SELECT currval('file_id_seq')", [], |row| row.get(0))?;

    let mut sheet_info = Vec::new();
    let mut processed_rows = 0;
    let mut sheet_idx = 0;

    for sheet in sheets {
        let row_count = sheet.rows.len() as i32;
        let col_names = sanitize_col_names(&sheet.headers);
        let table_name = format!("sheet_{}_{}", file_id, sheet_idx);

        // Build CREATE TABLE statement
        let col_defs: Vec<String> = col_names.iter().map(|c| format!("{} TEXT", quote_ident(c))).collect();
        let create_sql = format!("CREATE TABLE {} ({});", quote_ident(&table_name), col_defs.join(", "));
        self.conn.execute(&create_sql, [])?;

        // Insert rows using appender
        let col_list: Vec<String> = col_names.iter().map(|c| quote_ident(c)).collect();
        // Use transaction + appender for bulk insert
        let tx = self.conn.transaction()?;
        {
            let mut appender = tx.appender(&table_name)?;
            for row in &sheet.rows {
                // Pad row to match column count
                let mut padded_row = row.clone();
                padded_row.resize(col_names.len(), String::new());
                // Build params
                let params: Vec<&dyn duckdb::ToSql> = padded_row.iter().map(|s| s as &dyn duckdb::ToSql).collect();
                appender.append_row(params.as_slice())?;

                processed_rows += 1;
                progress_callback(processed_rows, total_rows);
            }
        }
        tx.commit()?;

        // Store col_names as JSON array in the sheets table
        let col_names_json = serde_json::to_string(&col_names).unwrap_or("[]".to_string());
        // Actually, DuckDB supports arrays. Let's use a TEXT field with comma-separated names for simplicity.
        // Or better: store as a TEXT column with JSON.
        // Simplest approach: store col_names as a pipe-delimited string.
        let col_names_str = col_names.join("\x1f"); // unit separator as delimiter

        self.conn.execute(
            "INSERT INTO sheets (file_id, sheet_name, table_name, row_count, col_names) VALUES (?, ?, ?, ?, ?)",
            params![file_id, &sheet.name, &table_name, row_count, &col_names_str],
        )?;

        sheet_info.push((sheet.name.clone(), sheet.rows.len()));
        sheet_idx += 1;
    }

    Ok(FileInfo {
        name: file_name,
        sheets: sheet_info,
        total_rows,
        sample,
    })
}
```

> **Note:** We need to add `serde_json` to Cargo.toml OR use a simple delimiter. Using unit separator `\x1f` is simpler with no new dependency. Let's use the delimiter approach.

**Step 4: Update `sheets` table schema to include `table_name` and `col_names`**

Already included in Step 1.

**Step 5: Update `FileInfo` and `FileSample` structs if needed**

No changes needed - they stay the same for TUI compatibility.

### Task 2: Update `database.rs` - Search Logic

**Files:**
- Modify: `src/database.rs`

**Step 1: Rewrite `search()` method**

The new search builds a single SQL query per sheet:

```rust
pub fn search(&self, query: &SearchQuery) -> Result<(Vec<SearchResult>, SearchStats)> {
    let start = Instant::now();

    // Get all sheet metadata
    let mut stmt = self.conn.prepare(
        "SELECT s.sheet_id, s.sheet_name, s.table_name, s.col_names, f.file_name
         FROM sheets s JOIN files f ON s.file_id = f.file_id",
    )?;

    struct SheetMeta {
        sheet_id: i64,
        sheet_name: String,
        table_name: String,
        col_names: Vec<String>,
        file_name: String,
    }

    let sheets_info: Vec<SheetMeta> = stmt
        .query_map([], |row| {
            let col_names_str: String = row.get(3)?;
            let col_names: Vec<String> = if col_names_str.is_empty() {
                vec![]
            } else {
                col_names_str.split('\x1f').map(|s| s.to_string()).collect()
            };
            Ok(SheetMeta {
                sheet_id: row.get(0)?,
                sheet_name: row.get(1)?,
                table_name: row.get(2)?,
                col_names,
                file_name: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let total_rows_searched: usize = self.conn.query_row(
        "SELECT COALESCE(SUM(row_count), 0) FROM sheets",
        [],
        |row| row.get::<_, i64>(0),
    )? as usize;

    let mut results = Vec::new();
    let mut matches_per_sheet: HashMap<String, usize> = HashMap::new();

    for sheet_meta in &sheets_info {
        if sheet_meta.col_names.is_empty() {
            continue;
        }

        // Build WHERE clause against actual column names
        let (where_sql, search_values) = Self::build_wide_where_clause(query, &sheet_meta.col_names);

        let sql = format!(
            "SELECT rowid, {} FROM {} WHERE {}",
            sheet_meta.col_names.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", "),
            quote_ident(&sheet_meta.table_name),
            where_sql
        );

        let mut search_stmt = self.conn.prepare(&sql)?;
        let param_refs: Vec<&dyn duckdb::ToSql> = search_values.iter().map(|v| v as &dyn duckdb::ToSql).collect();

        let matched_rows: Vec<(i64, Vec<Option<String>>)> = search_stmt
            .query_map(param_refs.as_slice(), |row| {
                let rowid: i64 = row.get(0)?;
                let mut values = Vec::new();
                for i in 1..=sheet_meta.col_names.len() {
                    values.push(row.get::<_, Option<String>>(i)?);
                }
                Ok((rowid, values))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if matched_rows.is_empty() {
            continue;
        }

        matches_per_sheet.insert(sheet_meta.sheet_name.clone(), matched_rows.len());

        let search_lower = query.text.to_lowercase();

        for (_rowid, values) in matched_rows {
            let row_vec: Vec<String> = values.iter().map(|v| v.clone().unwrap_or_default()).collect();
            let col_names = sheet_meta.col_names.clone();

            // Determine which columns matched
            let matched_columns: Vec<usize> = match query.mode {
                SearchMode::FullText => {
                    row_vec.iter().enumerate()
                        .filter(|(_, v)| v.to_lowercase().contains(&search_lower))
                        .map(|(i, _)| i)
                        .collect()
                }
                SearchMode::ExactMatch => {
                    row_vec.iter().enumerate()
                        .filter(|(_, v)| v == &query.text)
                        .map(|(i, _)| i)
                        .collect()
                }
                SearchMode::Wildcard => {
                    // For wildcard, check if the value matches the pattern
                    // Simple approach: use the same LIKE logic
                    row_vec.iter().enumerate()
                        .filter(|(_, v)| Self::matches_wildcard(&query.text, v))
                        .map(|(i, _)| i)
                        .collect()
                }
            };

            // If column filter is set, only include matches in that column
            let matched_columns = if let Some(ref col) = query.column {
                matched_columns.into_iter()
                    .filter(|&i| col_names.get(i).map(|n| n == col).unwrap_or(false))
                    .collect()
            } else {
                matched_columns
            };

            results.push(SearchResult {
                sheet_name: sheet_meta.sheet_name.clone(),
                file_name: sheet_meta.file_name.clone(),
                row: row_vec,
                col_names,
                matched_columns,
            });
        }
    }

    let total_matches = results.len();
    let search_duration = start.elapsed();

    Ok((results, SearchStats {
        total_rows_searched,
        total_matches,
        matches_per_sheet,
        search_duration,
    }))
}
```

**Step 2: Add `build_wide_where_clause` method**

```rust
fn build_wide_where_clause(query: &SearchQuery, col_names: &[String]) -> (String, Vec<String>) {
    let mut parts = Vec::new();
    let mut values = Vec::new();

    let target_cols: Vec<&String> = if let Some(ref col) = query.column {
        col_names.iter().filter(|c| *c == col).collect()
    } else {
        col_names.iter().collect()
    };

    for col in target_cols {
        match query.mode {
            SearchMode::FullText => {
                parts.push(format!("{} ILIKE ?", quote_ident(col)));
                values.push(format!("%{}%", query.text));
            }
            SearchMode::ExactMatch => {
                parts.push(format!("{} = ?", quote_ident(col)));
                values.push(query.text.clone());
            }
            SearchMode::Wildcard => {
                parts.push(format!("{} LIKE ?", quote_ident(col)));
                values.push(query.text.clone());
            }
        }
    }

    let where_sql = if parts.is_empty() {
        "1=0".to_string() // no matching columns
    } else {
        parts.join(" OR ")
    };

    (where_sql, values)
}
```

**Step 3: Add `matches_wildcard` helper for post-filtering matched columns**

```rust
fn matches_wildcard(pattern: &str, value: &str) -> bool {
    // Convert SQL LIKE pattern to a simple check
    // % = any chars, _ = single char
    let regex_pattern = pattern
        .replace('%', ".*")
        .replace('_', ".");
    regex::Regex::new(&format!("^{}$", regex_pattern))
        .map(|re| re.is_match(value))
        .unwrap_or(false)
}
```

Wait - we don't want to add a regex dependency. Let's use DuckDB's LIKE for the SQL query (which we already do), and for post-filtering matched columns, we can just re-check with the same LIKE logic using a simple approach:

Actually, for determining which specific columns matched, we can simplify: since the SQL WHERE clause already filters, we just need to check each column's value against the same criteria. For FullText and ExactMatch, simple string ops work. For Wildcard, we can use DuckDB's built-in.

**Better approach:** Instead of post-filtering, use a smarter SQL that also returns which columns matched. But that's complex. The simplest approach:

For FullText: check `value.to_lowercase().contains(query.text.to_lowercase())`
For ExactMatch: check `value == query.text`
For Wildcard: We can ask DuckDB per-column with LIKE. OR: implement a simple wildcard matcher.

Simple wildcard matcher without regex dependency:

```rust
fn like_match(pattern: &str, text: &str) -> bool {
    fn match_inner(p: &[char], t: &[char]) -> bool {
        if p.is_empty() { return t.is_empty(); }
        if p[0] == '%' {
            // Try matching 0 or more chars
            for i in 0..=t.len() {
                if match_inner(&p[1..], &t[i..]) { return true; }
            }
            return false;
        }
        if t.is_empty() { return false; }
        if p[0] == '_' || p[0] == t[0] {
            return match_inner(&p[1..], &t[1..]);
        }
        false
    }
    match_inner(&pattern.chars().collect::<Vec<_>>(), &text.chars().collect::<Vec<_>>())
}
```

### Task 3: Update `clear()` and `list_files()`

**Files:**
- Modify: `src/database.rs`

**Step 1: Update `clear()` to drop wide tables**

```rust
pub fn clear(&mut self) -> Result<()> {
    // Get all table names before deleting metadata
    let mut stmt = self.conn.prepare("SELECT table_name FROM sheets")?;
    let table_names: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    for table_name in &table_names {
        self.conn.execute(&format!("DROP TABLE IF EXISTS {}", quote_ident(table_name)), [])?;
    }

    self.conn.execute("DELETE FROM sheets", [])?;
    self.conn.execute("DELETE FROM files", [])?;
    Ok(())
}
```

**Step 2: Update `list_files()` - add `table_name` and `col_names` to query but keep return type**

The `list_files()` return type stays `Vec<FileInfo>` so no change needed to the query since FileInfo doesn't include table_name. Just add the column to the SELECT but don't use it:

Actually `list_files()` doesn't need changes since it only reads `file_name`, `sheet_name`, `row_count`. The new columns (`table_name`, `col_names`) are only used by `search()`.

### Task 4: Update Tests

**Files:**
- Modify: `tests/integration_test.rs`

**Step 1: Run existing tests to see which pass/fail**

Run: `cargo test`
Expected: Some tests will fail because the internal DB schema changed.

**Step 2: Fix test assertions if needed**

The public API (`import_excel`, `search`) returns the same types (`FileInfo`, `SearchResult`, `SearchStats`), so tests should mostly work. Check if any test directly accesses the `cells` table (they don't - they only use the public API).

**Step 3: Add new test for wide table search**

```rust
#[test]
fn test_wide_table_search_multiple_sheets() {
    let mut db = search_excel::database::Database::new().expect("db new");
    db.import_excel(Path::new("test_data.xlsx"), |_, _| {}).expect("import");

    // Search across all sheets
    let query = search_excel::database::SearchQuery {
        text: "Engineering".into(),
        column: None,
        mode: search_excel::database::SearchMode::FullText,
    };
    let (results, stats) = db.search(&query).expect("search");
    assert_eq!(results.len(), 4);
    assert_eq!(stats.total_matches, 4);
    // Verify matched_columns is populated
    for result in &results {
        assert!(!result.matched_columns.is_empty());
    }
}
```

**Step 4: Run all tests**

Run: `cargo test`
Expected: ALL PASS

### Task 5: Update `app.rs` if needed

**Files:**
- Modify: `src/app.rs` (if any `cells` table references exist)

The app only uses `Database` through its public methods (`import_excel`, `search`, `list_files`, `clear`). No direct SQL is executed in app.rs. The only potential issue is if `Database::new()` is called and the `cells` table is referenced elsewhere.

Check: The `clear()` method in app.rs calls `db.clear()` which is handled in Task 3. No other changes needed.

### Task 6: Build & Verify

**Step 1: Run `cargo build`**

Run: `cargo build`
Expected: Clean build

**Step 2: Run `cargo test`**

Run: `cargo test`
Expected: All tests pass

**Step 3: Manual test with test data**

Run: `cargo run -- test_data.xlsx -q Engineering`
Expected: Results shown with matched cells highlighted in green.

---

## Summary of Changes

| File | Change |
|------|--------|
| `src/database.rs` | Replace `cells` EAV table with per-sheet wide tables. Rewrite `import_excel()`, `search()`, `clear()`. Add helpers: `sanitize_col_names()`, `quote_ident()`, `build_wide_where_clause()`, `like_match()`. |
| `src/app.rs` | No changes needed (uses Database public API only). |
| `src/excel.rs` | No changes needed. |
| `src/main.rs` | No changes needed. |
| `src/event.rs` | No changes needed. |
| `tests/integration_test.rs` | Add new test for wide table verification. |
