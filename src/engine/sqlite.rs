use super::*;
use crate::excel::parse_file;
use anyhow::Result;
use rusqlite::functions::FunctionFlags;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

pub struct SqliteEngine {
    conn: Connection,
}

impl SqliteEngine {
    fn add_regexp_fn(conn: &Connection) -> Result<()> {
        conn.create_scalar_function(
            "regexp",
            2,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            |ctx| {
                let pattern: String = ctx.get(0)?;
                let text: String = ctx.get(1)?;
                let case_insensitive = format!("(?i){}", pattern);
                let re = regex::Regex::new(&case_insensitive)
                    .map_err(|e| rusqlite::Error::UserFunctionError(e.into()))?;
                Ok(re.is_match(&text))
            },
        )?;
        Ok(())
    }

    fn import_csv(&mut self, path: &Path, progress: &dyn Fn(usize, usize)) -> Result<FileInfo> {
        let sheets = parse_file(path)?;
        if sheets.is_empty() {
            anyhow::bail!("CSV file is empty");
        }

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        self.conn.execute(
            "INSERT INTO files (file_name) VALUES (?)",
            params![&file_name],
        )?;
        let file_id: i64 = self
            .conn
            .query_row("SELECT last_insert_rowid()", [], |row| row.get(0))?;

        let sheet = &sheets[0];
        let sheet_name = sheet.name.clone();
        let table_name = format!("sheet_{}_0", file_id);
        let col_names = sanitize_col_names(&sheet.headers);

        let col_defs: Vec<String> = col_names
            .iter()
            .map(|c| format!("{} TEXT", quote_ident(c)))
            .collect();
        self.conn.execute(
            &format!(
                "CREATE TABLE {} ({})",
                quote_ident(&table_name),
                col_defs.join(", ")
            ),
            [],
        )?;

        let placeholders: Vec<&str> = (0..col_names.len()).map(|_| "?").collect();
        let insert_sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            quote_ident(&table_name),
            col_names
                .iter()
                .map(|c| quote_ident(c))
                .collect::<Vec<_>>()
                .join(", "),
            placeholders.join(", ")
        );

        let row_count = sheet.rows.len();
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(&insert_sql)?;
            for row in &sheet.rows {
                let mut padded = row.clone();
                padded.resize(col_names.len(), String::new());
                let values: Vec<Box<dyn rusqlite::types::ToSql>> = padded
                    .into_iter()
                    .map(|s| Box::new(s) as Box<dyn rusqlite::types::ToSql>)
                    .collect();
                stmt.execute(rusqlite::params_from_iter(values))?;
            }
        }
        tx.commit()?;

        let sample_rows: Vec<Vec<String>> = {
            let col_list: String = col_names
                .iter()
                .map(|c| quote_ident(c))
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT {} FROM {} LIMIT 3",
                col_list,
                quote_ident(&table_name)
            );
            let mut stmt = self.conn.prepare(&sql)?;
            let n = col_names.len();
            let rows = stmt
                .query_map([], |row| {
                    let mut values = Vec::with_capacity(n);
                    for i in 0..n {
                        values.push(row.get::<_, Option<String>>(i)?.unwrap_or_default());
                    }
                    Ok(values)
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };

        progress(row_count, row_count);

        let col_names_str = col_names.join("\x1f");
        self.conn.execute(
            "INSERT INTO sheets (file_id, sheet_name, table_name, row_count, col_names, col_widths) VALUES (?, ?, ?, ?, ?, ?)",
            params![file_id, &sheet_name, &table_name, row_count as i32, &col_names_str, ""],
        )?;

        Ok(FileInfo {
            name: file_name,
            sheets: vec![(sheet_name.clone(), row_count)],
            total_rows: row_count,
            sample: if sample_rows.is_empty() {
                None
            } else {
                Some(FileSample {
                    sheet_name,
                    headers: col_names,
                    rows: sample_rows,
                })
            },
        })
    }

    fn import_excel_file(
        &mut self,
        path: &Path,
        progress: &dyn Fn(usize, usize),
    ) -> Result<FileInfo> {
        let sheets = parse_file(path)?;
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let total_rows: usize = sheets.iter().map(|s| s.rows.len()).sum();
        let sample = sheets.first().map(|s| FileSample {
            sheet_name: s.name.clone(),
            headers: s.headers.clone(),
            rows: s.rows.iter().take(3).cloned().collect(),
        });

        self.conn.execute(
            "INSERT INTO files (file_name) VALUES (?)",
            params![&file_name],
        )?;
        let file_id: i64 = self
            .conn
            .query_row("SELECT last_insert_rowid()", [], |row| row.get(0))?;

        let mut sheet_info = Vec::new();
        let mut processed_rows = 0;

        for (sheet_idx, sheet) in sheets.into_iter().enumerate() {
            let row_count = sheet.rows.len() as i32;
            let col_names = sanitize_col_names(&sheet.headers);
            let table_name = format!("sheet_{}_{}", file_id, sheet_idx);

            let col_defs: Vec<String> = col_names
                .iter()
                .map(|c| format!("{} TEXT", quote_ident(c)))
                .collect();
            self.conn.execute(
                &format!(
                    "CREATE TABLE {} ({})",
                    quote_ident(&table_name),
                    col_defs.join(", ")
                ),
                [],
            )?;

            let placeholders: Vec<&str> = (0..col_names.len()).map(|_| "?").collect();
            let insert_sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                quote_ident(&table_name),
                col_names
                    .iter()
                    .map(|c| quote_ident(c))
                    .collect::<Vec<_>>()
                    .join(", "),
                placeholders.join(", ")
            );

            let tx = self.conn.transaction()?;
            {
                let mut stmt = tx.prepare(&insert_sql)?;
                for row in &sheet.rows {
                    let mut padded = row.clone();
                    padded.resize(col_names.len(), String::new());
                    let values: Vec<Box<dyn rusqlite::types::ToSql>> = padded
                        .into_iter()
                        .map(|s| Box::new(s) as Box<dyn rusqlite::types::ToSql>)
                        .collect();
                    stmt.execute(rusqlite::params_from_iter(values))?;
                    processed_rows += 1;
                    progress(processed_rows, total_rows);
                }
            }
            tx.commit()?;

            let col_names_str = col_names.join("\x1f");
            let col_widths_str = sheet
                .col_widths
                .iter()
                .map(|w| format!("{}", w))
                .collect::<Vec<_>>()
                .join("\x1f");
            self.conn.execute(
                "INSERT INTO sheets (file_id, sheet_name, table_name, row_count, col_names, col_widths) VALUES (?, ?, ?, ?, ?, ?)",
                params![file_id, &sheet.name, &table_name, row_count, &col_names_str, &col_widths_str],
            )?;

            sheet_info.push((sheet.name, row_count as usize));
        }

        Ok(FileInfo {
            name: file_name,
            sheets: sheet_info,
            total_rows,
            sample,
        })
    }

    fn build_where_clause(query: &SearchQuery, col_names: &[String]) -> (String, Vec<String>) {
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
                    parts.push(format!("{} LIKE ?", quote_ident(col)));
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
                SearchMode::Regex => {
                    parts.push(format!("regexp(?, {})", quote_ident(col)));
                    values.push(format!("(?i){}", query.text));
                }
            }
        }

        let where_sql = if parts.is_empty() {
            "1=0".to_string()
        } else {
            parts.join(" OR ")
        };
        (where_sql, values)
    }
}

impl SearchEngine for SqliteEngine {
    fn new() -> Result<Self> {
        let conn = Connection::open_in_memory()?;

        conn.execute_batch(
            "CREATE TABLE files (
                file_id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_name TEXT NOT NULL,
                imported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE sheets (
                sheet_id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id INTEGER NOT NULL REFERENCES files(file_id),
                sheet_name TEXT NOT NULL,
                table_name TEXT NOT NULL,
                row_count INTEGER DEFAULT 0,
                col_names TEXT DEFAULT '',
                col_widths TEXT DEFAULT ''
            );",
        )?;

        Self::add_regexp_fn(&conn)?;

        Ok(SqliteEngine { conn })
    }

    fn import_excel(&mut self, path: &Path, progress: &dyn Fn(usize, usize)) -> Result<FileInfo> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if ext == "csv" {
            return self.import_csv(path, progress);
        }

        self.import_excel_file(path, progress)
    }

    fn search(&self, query: &SearchQuery) -> Result<(Vec<SearchResult>, SearchStats)> {
        let start = Instant::now();

        let mut stmt = self.conn.prepare(
            "SELECT s.sheet_name, s.table_name, s.col_names, s.col_widths, f.file_name
             FROM sheets s JOIN files f ON s.file_id = f.file_id",
        )?;

        struct SheetMeta {
            sheet_name: String,
            table_name: String,
            col_names: Vec<String>,
            col_widths: Vec<f64>,
            file_name: String,
        }

        let sheets_info: Vec<SheetMeta> = stmt
            .query_map([], |row| {
                let col_names_str: String = row.get(2)?;
                let col_names: Vec<String> = if col_names_str.is_empty() {
                    vec![]
                } else {
                    col_names_str.split('\x1f').map(|s| s.to_string()).collect()
                };
                let col_widths_str: String = row.get(3)?;
                let col_widths: Vec<f64> = if col_widths_str.is_empty() {
                    vec![]
                } else {
                    col_widths_str
                        .split('\x1f')
                        .filter_map(|s| s.parse::<f64>().ok())
                        .collect()
                };
                Ok(SheetMeta {
                    sheet_name: row.get(0)?,
                    table_name: row.get(1)?,
                    col_names,
                    col_widths,
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
        let mut truncated = false;

        for meta in &sheets_info {
            if results.len() >= query.limit {
                truncated = true;
                break;
            }
            if meta.col_names.is_empty() {
                continue;
            }

            let (where_sql, search_values) = Self::build_where_clause(query, &meta.col_names);
            if where_sql == "1=0" {
                continue;
            }

            let remaining = query.limit.saturating_sub(results.len());
            let col_list: String = meta
                .col_names
                .iter()
                .map(|c| quote_ident(c))
                .collect::<Vec<_>>()
                .join(", ");
            let limit_clause = if remaining < i64::MAX as usize {
                format!(" LIMIT {}", remaining)
            } else {
                String::new()
            };
            let sql = format!(
                "SELECT {} FROM {} WHERE {}{}",
                col_list,
                quote_ident(&meta.table_name),
                where_sql,
                limit_clause
            );

            let mut search_stmt = self.conn.prepare(&sql)?;
            let param_refs: Vec<Box<dyn rusqlite::types::ToSql>> = search_values
                .iter()
                .map(|v| Box::new(v.clone()) as Box<dyn rusqlite::types::ToSql>)
                .collect();

            let matched_rows: Vec<Vec<Option<String>>> = search_stmt
                .query_map(rusqlite::params_from_iter(param_refs), |row| {
                    let mut values = Vec::new();
                    for i in 0..meta.col_names.len() {
                        values.push(row.get::<_, Option<String>>(i)?);
                    }
                    Ok(values)
                })?
                .collect::<Result<Vec<_>, _>>()?;

            if matched_rows.is_empty() {
                continue;
            }

            matches_per_sheet.insert(meta.sheet_name.clone(), matched_rows.len());

            for values in matched_rows {
                if results.len() >= query.limit {
                    truncated = true;
                    break;
                }
                let row_vec: Vec<String> = values
                    .iter()
                    .map(|v| v.clone().unwrap_or_default())
                    .collect();
                let col_names = meta.col_names.clone();
                let matched_columns = find_matched_columns(query, &row_vec, &col_names);

                results.push(SearchResult {
                    sheet_name: meta.sheet_name.clone(),
                    file_name: meta.file_name.clone(),
                    row: row_vec,
                    col_names,
                    matched_columns,
                    col_widths: meta.col_widths.clone(),
                });
            }
        }

        let total_matches = results.len();
        let search_duration = start.elapsed();

        Ok((
            results,
            SearchStats {
                total_rows_searched,
                total_matches,
                matches_per_sheet,
                search_duration,
                truncated,
            },
        ))
    }

    fn list_files(&self) -> Vec<FileInfo> {
        let mut stmt = match self.conn.prepare(
            "SELECT f.file_name, s.sheet_name, s.row_count
             FROM files f
             LEFT JOIN sheets s ON f.file_id = s.file_id
             ORDER BY f.file_id, s.sheet_id",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let rows: Vec<(String, Option<String>, Option<i32>)> = match stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i32>>(2)?,
            ))
        }) {
            Ok(mapped) => mapped.filter_map(|r| r.ok()).collect(),
            Err(_) => return Vec::new(),
        };

        let mut files_map: HashMap<String, FileInfo> = HashMap::new();

        for (file_name, sheet_name, row_count) in rows {
            let entry = files_map.entry(file_name.clone()).or_insert(FileInfo {
                name: file_name,
                sheets: Vec::new(),
                total_rows: 0,
                sample: None,
            });

            if let (Some(name), Some(count)) = (sheet_name, row_count) {
                entry.sheets.push((name, count as usize));
                entry.total_rows += count as usize;
            }
        }

        files_map.into_values().collect()
    }

    fn clear(&mut self) -> Result<()> {
        let mut stmt = self.conn.prepare("SELECT table_name FROM sheets")?;
        let table_names: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        for table_name in &table_names {
            self.conn.execute(
                &format!("DROP TABLE IF EXISTS {}", quote_ident(table_name)),
                [],
            )?;
        }

        self.conn.execute("DELETE FROM sheets", [])?;
        self.conn.execute("DELETE FROM files", [])?;
        Ok(())
    }
}
