use super::*;
use crate::excel::parse_file;
use anyhow::Result;
use rusqlite::functions::FunctionFlags;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

struct SheetQueryMeta {
    table_name: String,
    col_names: Vec<String>,
    row_count: usize,
}

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

        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let dotted_alias = format!("{}.{}", file_stem, sheet_name);
        let _ = self.conn.execute(
            &format!(
                "CREATE VIEW IF NOT EXISTS {} AS SELECT * FROM {}",
                quote_ident(&dotted_alias),
                quote_ident(&table_name),
            ),
            [],
        );

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

            let file_stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let dotted_alias = format!("{}.{}", file_stem, sheet.name);
            let _ = self.conn.execute(
                &format!(
                    "CREATE VIEW IF NOT EXISTS {} AS SELECT * FROM {}",
                    quote_ident(&dotted_alias),
                    quote_ident(&table_name),
                ),
                [],
            );

            sheet_info.push((sheet.name, row_count as usize));
        }

        Ok(FileInfo {
            name: file_name,
            sheets: sheet_info,
            total_rows,
            sample,
        })
    }

    fn get_sheet_metadata_query(&self, file_name: &str, sheet_name: &str) -> Result<SheetQueryMeta> {
        let result = self.conn.query_row(
            "SELECT s.table_name, s.col_names, s.row_count
             FROM sheets s JOIN files f ON s.file_id = f.file_id
             WHERE f.file_name = ? AND s.sheet_name = ?",
            params![file_name, sheet_name],
            |row| {
                let table_name: String = row.get(0)?;
                let col_names_str: String = row.get(1)?;
                let col_names: Vec<String> = if col_names_str.is_empty() {
                    vec![]
                } else {
                    col_names_str.split('\x1f').map(|s| s.to_string()).collect()
                };
                let row_count: i32 = row.get(2)?;
                Ok((table_name, col_names, row_count as usize))
            }
        );

        match result {
            Ok((table_name, col_names, row_count)) => Ok(SheetQueryMeta { table_name, col_names, row_count }),
            Err(_) => anyhow::bail!(
                "Sheet '{}' in file '{}' not found. Use get_metadata to discover sheets.",
                sheet_name, file_name
            ),
        }
    }

    fn query_rows(&self, sql: &str, col_names: &[String]) -> Result<Vec<Vec<String>>> {
        let mut stmt = self.conn.prepare(sql)?;
        let col_count = col_names.len();
        let rows: Vec<Vec<String>> = stmt.query_map([], |row| {
            let mut values = Vec::with_capacity(col_count);
            for i in 0..col_count {
                values.push(row.get::<_, Option<String>>(i)?.unwrap_or_default());
            }
            Ok(values)
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
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
            if let Some(ref sheet_name) = query.sheet {
                if meta.sheet_name != *sheet_name {
                    continue;
                }
            }

            let (where_sql, search_values) = Self::build_where_clause(query, &meta.col_names);
            if where_sql == "1=0" && !query.invert {
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
            let effective_where = if where_sql == "1=0" {
                if query.invert { "1=1".to_string() } else { "1=0".to_string() }
            } else if query.invert {
                format!("NOT ({})", where_sql)
            } else {
                where_sql
            };
            let sql = format!(
                "SELECT {} FROM {} WHERE {}{}",
                col_list,
                quote_ident(&meta.table_name),
                effective_where,
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
        {
            let mut stmt = self.conn.prepare(
                "SELECT name FROM sqlite_master WHERE type='view'",
            )?;
            let views: Vec<String> = stmt
                .query_map([], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            for view in &views {
                let _ = self.conn.execute(
                    &format!("DROP VIEW IF EXISTS {}", quote_ident(view)),
                    [],
                );
            }
        }

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
        let _ = self.conn.execute("DELETE FROM sqlite_sequence WHERE name IN ('files', 'sheets')", []);
        Ok(())
    }

    fn execute_sql(&self, sql: &str, limit: usize) -> Result<crate::types::SqlResult> {
        super::validate_sql(sql)?;
        let start = std::time::Instant::now();

        let limited_sql = format!("SELECT * FROM ({}) LIMIT {}", sql, limit);
        let mut stmt = self.conn.prepare(&limited_sql)?;
        let col_count = stmt.column_count();

        let columns: Vec<String> = (0..col_count)
            .map(|i| stmt.column_name(i).map(|name| name.to_string()))
            .collect::<Result<Vec<_>, _>>()?;

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

        Ok(crate::types::SqlResult {
            columns,
            rows,
            row_count,
            truncated,
            duration,
        })
    }

    fn list_table_aliases(&self) -> Vec<crate::types::TableAliasInfo> {
        let mut stmt = match self.conn.prepare(
            "SELECT s.table_name, s.sheet_name, s.row_count, s.col_names, f.file_name
             FROM sheets s JOIN files f ON s.file_id = f.file_id
             ORDER BY f.file_id, s.sheet_id"
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let rows: Vec<(String, String, i32, String, String)> = match stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        }) {
            Ok(mapped) => mapped.filter_map(|r| r.ok()).collect(),
            Err(_) => return Vec::new(),
        };

        rows.into_iter().map(|(table_name, sheet_name, row_count, col_names_str, file_name)| {
            let columns: Vec<String> = if col_names_str.is_empty() {
                vec![]
            } else {
                col_names_str.split('\x1f').map(|s| s.to_string()).collect()
            };
            let file_stem = std::path::Path::new(&file_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let alias = format!("{}.{}", file_stem, sheet_name);
            crate::types::TableAliasInfo {
                table_name,
                alias,
                file_name,
                sheet_name,
                row_count: row_count as usize,
                columns,
            }
        }).collect()
    }

    #[cfg(feature = "mcp-server")]
    fn get_metadata(&self, file_name: &str) -> Result<FileMetadataInfo> {
        let mut stmt = self.conn.prepare(
            "SELECT s.sheet_name, s.row_count, s.col_names
             FROM sheets s JOIN files f ON s.file_id = f.file_id
             WHERE f.file_name = ?
             ORDER BY s.sheet_id"
        )?;

        let sheet_infos: Vec<SheetMetadataInfo> = stmt.query_map(params![file_name], |row| {
            let sheet_name: String = row.get(0)?;
            let row_count: i32 = row.get(1)?;
            let col_names_str: String = row.get(2)?;
            let columns: Vec<String> = if col_names_str.is_empty() {
                vec![]
            } else {
                col_names_str.split('\x1f').map(|s| s.to_string()).collect()
            };
            Ok(SheetMetadataInfo {
                sheet_name,
                row_count: row_count as usize,
                columns,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        if sheet_infos.is_empty() {
            anyhow::bail!("File '{}' not found. Use list_files to see imported files.", file_name);
        }

        Ok(FileMetadataInfo {
            file_name: file_name.to_string(),
            sheet_count: sheet_infos.len(),
            sheets: sheet_infos,
        })
    }

    #[cfg(feature = "mcp-server")]
    fn get_sheet_sample(&self, file_name: &str, sheet_name: &str, sample_size: usize) -> Result<SheetDataResult> {
        let meta = self.get_sheet_metadata_query(file_name, sheet_name)?;

        let col_list: String = meta.col_names.iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "SELECT {} FROM {} ORDER BY RANDOM() LIMIT {}",
            col_list,
            quote_ident(&meta.table_name),
            sample_size
        );

        let rows = self.query_rows(&sql, &meta.col_names)?;
        let total_rows = meta.row_count;
        let row_count = rows.len();

        Ok(SheetDataResult {
            file_name: file_name.to_string(),
            sheet_name: sheet_name.to_string(),
            columns: meta.col_names,
            rows,
            row_count,
            total_rows,
            truncated: row_count < total_rows,
        })
    }

    #[cfg(feature = "mcp-server")]
    fn get_sheet_data(
        &self,
        file_name: &str,
        sheet_name: &str,
        start_row: Option<usize>,
        end_row: Option<usize>,
        columns: Option<&[String]>,
    ) -> Result<SheetDataResult> {
        let meta = self.get_sheet_metadata_query(file_name, sheet_name)?;

        let selected_cols: Vec<String> = if let Some(cols) = columns {
            cols.to_vec()
        } else {
            meta.col_names.clone()
        };

        let col_indices: Vec<usize> = selected_cols.iter()
            .filter_map(|c| meta.col_names.iter().position(|h| h == c))
            .collect();
        let col_names: Vec<String> = col_indices.iter()
            .map(|&i| meta.col_names[i].clone())
            .collect();

        let col_list: String = col_names.iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");

        let start = start_row.unwrap_or(0);
        let limit = end_row.unwrap_or(meta.row_count).saturating_sub(start);

        let sql = format!(
            "SELECT {} FROM {} LIMIT {} OFFSET {}",
            col_list,
            quote_ident(&meta.table_name),
            limit,
            start
        );

        let rows = self.query_rows(&sql, &col_names)?;
        let total_rows = meta.row_count;

        Ok(SheetDataResult {
            file_name: file_name.to_string(),
            sheet_name: sheet_name.to_string(),
            columns: col_names,
            rows,
            row_count: rows.len(),
            total_rows,
            truncated: false,
        })
    }

    #[cfg(feature = "mcp-server")]
    fn save_as(&self, file_name: &str, output_path: &Path) -> Result<()> {
        use crate::engine::write_xlsx;

        let mut stmt = self.conn.prepare(
            "SELECT s.sheet_name, s.table_name, s.col_names, s.row_count
             FROM sheets s JOIN files f ON s.file_id = f.file_id
             WHERE f.file_name = ?
             ORDER BY s.sheet_id"
        )?;

        let sheet_rows: Vec<(String, String, Vec<String>)> = stmt.query_map(params![file_name], |row| {
            let sheet_name: String = row.get(0)?;
            let table_name: String = row.get(1)?;
            let col_names_str: String = row.get(2)?;
            let col_names: Vec<String> = if col_names_str.is_empty() {
                vec![]
            } else {
                col_names_str.split('\x1f').map(|s| s.to_string()).collect()
            };
            Ok((sheet_name, table_name, col_names))
        })?.collect::<Result<Vec<_>, _>>()?;

        if sheet_rows.is_empty() {
            anyhow::bail!("File '{}' not found. Use list_files to see imported files.", file_name);
        }

        let mut sheets_data: Vec<(String, Vec<String>, Vec<Vec<String>>)> = Vec::new();
        for (sheet_name, table_name, col_names) in &sheet_rows {
            let col_list: String = col_names.iter()
                .map(|c| quote_ident(c))
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!("SELECT {} FROM {}", col_list, quote_ident(table_name));
            let rows = self.query_rows(&sql, col_names)?;
            sheets_data.push((sheet_name.clone(), col_names.clone(), rows));
        }

        let refs: Vec<(&str, &[String], &[Vec<String>])> = sheets_data.iter()
            .map(|(name, headers, rows)| (name.as_str(), &headers[..], &rows[..]))
            .collect();

        write_xlsx(&refs, output_path)
    }

    #[cfg(feature = "mcp-server")]
    fn update_cell(&mut self, file_name: &str, sheet_name: &str, row: usize, column: &str, value: &str) -> Result<()> {
        let meta = self.get_sheet_metadata_query(file_name, sheet_name)?;
        let col_idx = meta.col_names.iter().position(|h| h == column)
            .ok_or_else(|| anyhow::anyhow!(
                "Column '{}' not found. Available columns: {}",
                column, meta.col_names.join(", ")
            ))?;
        let quoted_col = quote_ident(&meta.col_names[col_idx]);
        let sql = format!("UPDATE {} SET {} = ? WHERE rowid = ?", quote_ident(&meta.table_name), quoted_col);
        let affected = self.conn.execute(&sql, params![value, (row + 1) as i64])?;
        if affected == 0 {
            anyhow::bail!("Row {} out of range (sheet has {} rows)", row, meta.row_count);
        }
        Ok(())
    }

    #[cfg(feature = "mcp-server")]
    fn update_cells(&mut self, file_name: &str, sheet_name: &str, updates: &[(usize, String, String)]) -> Result<usize> {
        let meta = self.get_sheet_metadata_query(file_name, sheet_name)?;
        let mut count = 0usize;
        for (row, column, value) in updates {
            if let Some(col_idx) = meta.col_names.iter().position(|h| h == column) {
                let quoted_col = quote_ident(&meta.col_names[col_idx]);
                let sql = format!("UPDATE {} SET {} = ? WHERE rowid = ?", quote_ident(&meta.table_name), quoted_col);
                if self.conn.execute(&sql, params![value, (*row + 1) as i64])? > 0 {
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    #[cfg(feature = "mcp-server")]
    fn insert_rows(&mut self, file_name: &str, sheet_name: &str, start_row: usize, rows: Vec<Vec<String>>) -> Result<()> {
        let meta = self.get_sheet_metadata_query(file_name, sheet_name)?;
        let total = meta.row_count;
        let start = start_row.min(total);
        let col_count = meta.col_names.len();
        let col_list = meta.col_names.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ");

        let temp_table = format!("{}_edit_temp", meta.table_name);

        let _ = self.conn.execute(&format!("DROP TABLE IF EXISTS {}", quote_ident(&temp_table)), []);

        let col_defs: Vec<String> = meta.col_names.iter().map(|c| format!("{} TEXT", quote_ident(c))).collect();
        self.conn.execute(&format!("CREATE TABLE {} ({})", quote_ident(&temp_table), col_defs.join(", ")), [])?;

        if start > 0 {
            self.conn.execute(&format!(
                "INSERT INTO {} ({}) SELECT {} FROM {} WHERE rowid <= {}",
                quote_ident(&temp_table), col_list, col_list, quote_ident(&meta.table_name), start
            ), [])?;
        }

        let placeholders: Vec<&str> = (0..col_count).map(|_| "?").collect();
        let insert_sql = format!("INSERT INTO {} ({}) VALUES ({})", quote_ident(&temp_table), col_list, placeholders.join(", "));
        for row in &rows {
            let mut padded = row.clone();
            padded.resize(col_count, String::new());
            let values: Vec<Box<dyn rusqlite::types::ToSql>> = padded.into_iter()
                .map(|s| Box::new(s) as Box<dyn rusqlite::types::ToSql>)
                .collect();
            self.conn.execute(&insert_sql, rusqlite::params_from_iter(values))?;
        }

        if start < total {
            self.conn.execute(&format!(
                "INSERT INTO {} ({}) SELECT {} FROM {} WHERE rowid > {}",
                quote_ident(&temp_table), col_list, col_list, quote_ident(&meta.table_name), start
            ), [])?;
        }

        self.conn.execute(&format!("DROP TABLE {}", quote_ident(&meta.table_name)), [])?;
        self.conn.execute(&format!("ALTER TABLE {} RENAME TO {}", quote_ident(&temp_table), quote_ident(&meta.table_name)), [])?;

        let new_count = total + rows.len();
        self.conn.execute("UPDATE sheets SET row_count = ? WHERE table_name = ?", params![new_count as i32, &meta.table_name])?;

        Ok(())
    }

    #[cfg(feature = "mcp-server")]
    fn delete_rows(&mut self, file_name: &str, sheet_name: &str, start_row: usize, count: usize) -> Result<usize> {
        let meta = self.get_sheet_metadata_query(file_name, sheet_name)?;
        if start_row >= meta.row_count {
            return Ok(0);
        }
        let end = (start_row + count).min(meta.row_count);
        let actual_count = end - start_row;

        // SQLite rowid is 1-based; our start_row is 0-based
        let sql = format!(
            "DELETE FROM {} WHERE rowid > ? AND rowid <= ?",
            quote_ident(&meta.table_name)
        );
        self.conn.execute(&sql, params![start_row as i64, end as i64])?;

        let new_count = meta.row_count - actual_count;
        self.conn.execute("UPDATE sheets SET row_count = ? WHERE table_name = ?", params![new_count as i32, &meta.table_name])?;

        Ok(actual_count)
    }

    #[cfg(feature = "mcp-server")]
    fn add_column(&mut self, file_name: &str, sheet_name: &str, column_name: &str, default_value: &str) -> Result<()> {
        let meta = self.get_sheet_metadata_query(file_name, sheet_name)?;
        if meta.col_names.iter().any(|h| h == column_name) {
            anyhow::bail!("Column '{}' already exists in sheet '{}'", column_name, sheet_name);
        }

        let quoted_col = quote_ident(column_name);
        self.conn.execute(&format!(
            "ALTER TABLE {} ADD COLUMN {} TEXT",
            quote_ident(&meta.table_name), quoted_col
        ), [])?;

        if !default_value.is_empty() {
            self.conn.execute(&format!(
                "UPDATE {} SET {} = ? WHERE {} IS NULL",
                quote_ident(&meta.table_name), quoted_col, quoted_col
            ), params![default_value])?;
        }

        let mut new_col_names = meta.col_names.clone();
        new_col_names.push(column_name.to_string());
        let col_names_str = new_col_names.join("\x1f");
        self.conn.execute("UPDATE sheets SET col_names = ? WHERE table_name = ?", params![&col_names_str, &meta.table_name])?;

        Ok(())
    }

    #[cfg(feature = "mcp-server")]
    fn rename_column(&mut self, file_name: &str, sheet_name: &str, old_name: &str, new_name: &str) -> Result<()> {
        let meta = self.get_sheet_metadata_query(file_name, sheet_name)?;
        let col_idx = meta.col_names.iter().position(|h| h == old_name)
            .ok_or_else(|| anyhow::anyhow!(
                "Column '{}' not found. Available columns: {}",
                old_name, meta.col_names.join(", ")
            ))?;

        if old_name != new_name && meta.col_names.iter().any(|h| h == new_name) {
            anyhow::bail!("Column '{}' already exists in sheet '{}'", new_name, sheet_name);
        }

        self.conn.execute(&format!(
            "ALTER TABLE {} RENAME COLUMN {} TO {}",
            quote_ident(&meta.table_name), quote_ident(old_name), quote_ident(new_name)
        ), [])?;

        let mut new_col_names = meta.col_names.clone();
        new_col_names[col_idx] = new_name.to_string();
        let col_names_str = new_col_names.join("\x1f");
        self.conn.execute("UPDATE sheets SET col_names = ? WHERE table_name = ?", params![&col_names_str, &meta.table_name])?;

        Ok(())
    }
}
