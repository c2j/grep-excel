use anyhow::Result;
use duckdb::{params, Connection};
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::excel::parse_file;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchMode {
    FullText,
    ExactMatch,
    Wildcard,
    Regex,
}

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub text: String,
    pub column: Option<String>,
    pub mode: SearchMode,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub sheet_name: String,
    pub file_name: String,
    pub row: Vec<String>,
    pub col_names: Vec<String>,
    pub matched_columns: Vec<usize>,
    pub col_widths: Vec<f64>,
}

#[derive(Debug, Clone)]
pub struct SearchStats {
    pub total_rows_searched: usize,
    pub total_matches: usize,
    pub matches_per_sheet: HashMap<String, usize>,
    pub search_duration: Duration,
    pub truncated: bool,
}

#[derive(Debug, Clone)]
pub struct FileSample {
    pub sheet_name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub sheets: Vec<(String, usize)>,
    pub total_rows: usize,
    pub sample: Option<FileSample>,
}

pub struct Database {
    conn: Connection,
}

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
            let count = seen
                .entry(base.clone())
                .and_modify(|c| *c += 1)
                .or_insert(1);
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

impl Database {
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
                col_names TEXT DEFAULT '',
                col_widths TEXT DEFAULT ''
            );",
        )?;

        Ok(Database { conn })
    }

    pub fn import_excel(
        &mut self,
        path: &Path,
        progress_callback: impl Fn(usize, usize),
    ) -> Result<FileInfo> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if ext == "csv" {
            return self.import_csv_direct(path, progress_callback);
        }

        self.import_excel_sheets(path, progress_callback)
    }

    /// Import CSV files using DuckDB's native read_csv_auto — much faster than
    /// parsing with the csv crate and inserting row-by-row via Appender.
    fn import_csv_direct(
        &mut self,
        path: &Path,
        progress_callback: impl Fn(usize, usize),
    ) -> Result<FileInfo> {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let sheet_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("csv")
            .to_string();

        self.conn.execute(
            "INSERT INTO files (file_name) VALUES (?)",
            params![&file_name],
        )?;
        let file_id: i64 = self
            .conn
            .query_row("SELECT currval('file_id_seq')", [], |row| row.get(0))?;

        let table_name = format!("sheet_{}_0", file_id);
        let path_str = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid file path encoding"))?;

        let create_sql = format!(
            "CREATE TABLE {} AS SELECT * FROM read_csv_auto('{}', header=true, all_varchar=true)",
            quote_ident(&table_name),
            path_str.replace('\'', "''")
        );
        self.conn.execute(&create_sql, [])?;

        let row_count: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM {}", quote_ident(&table_name)),
            [],
            |row| row.get(0),
        )?;

        let col_names: Vec<String> = {
            let mut stmt = self.conn.prepare(
                "SELECT column_name FROM information_schema.columns WHERE table_name = ? ORDER BY ordinal_position"
            )?;
            stmt.query_map(params![&table_name], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?
        };

        let sample_rows = self.get_sample_rows(&table_name, &col_names, 3)?;

        progress_callback(row_count as usize, row_count as usize);

        let col_names_str = col_names.join("\x1f");
        self.conn.execute(
            "INSERT INTO sheets (file_id, sheet_name, table_name, row_count, col_names, col_widths) VALUES (?, ?, ?, ?, ?, ?)",
            params![file_id, &sheet_name, &table_name, row_count as i32, &col_names_str, ""],
        )?;

        Ok(FileInfo {
            name: file_name,
            sheets: vec![(sheet_name.clone(), row_count as usize)],
            total_rows: row_count as usize,
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

    fn get_sample_rows(
        &self,
        table_name: &str,
        col_names: &[String],
        limit: usize,
    ) -> Result<Vec<Vec<String>>> {
        if col_names.is_empty() {
            return Ok(Vec::new());
        }
        let col_list: String = col_names
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT {} FROM {} LIMIT {}",
            col_list,
            quote_ident(table_name),
            limit
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let col_count = col_names.len();
        let rows: Vec<Vec<String>> = stmt
            .query_map([], |row| {
                let mut values = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    values.push(row.get::<_, Option<String>>(i)?.unwrap_or_default());
                }
                Ok(values)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Import Excel (non-CSV) files using calamine + DuckDB Appender.
    fn import_excel_sheets(
        &mut self,
        path: &Path,
        progress_callback: impl Fn(usize, usize),
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
            .query_row("SELECT currval('file_id_seq')", [], |row| row.get(0))?;

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
            let create_sql = format!(
                "CREATE TABLE {} ({})",
                quote_ident(&table_name),
                col_defs.join(", ")
            );
            self.conn.execute(&create_sql, [])?;

            let tx = self.conn.transaction()?;
            {
                let mut appender = tx.appender(&table_name)?;
                for row in &sheet.rows {
                    let mut padded_row = row.clone();
                    padded_row.resize(col_names.len(), String::new());
                    let param_refs: Vec<&dyn duckdb::ToSql> =
                        padded_row.iter().map(|s| s as &dyn duckdb::ToSql).collect();
                    appender.append_row(param_refs.as_slice())?;
                    processed_rows += 1;
                    progress_callback(processed_rows, total_rows);
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

            sheet_info.push((sheet.name, sheet.rows.len()));
        }

        Ok(FileInfo {
            name: file_name,
            sheets: sheet_info,
            total_rows,
            sample,
        })
    }

    pub fn search(&self, query: &SearchQuery) -> Result<(Vec<SearchResult>, SearchStats)> {
        let start = Instant::now();

        let mut stmt = self.conn.prepare(
            "SELECT s.sheet_id, s.sheet_name, s.table_name, s.col_names, s.col_widths, f.file_name
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
                let col_names_str: String = row.get(3)?;
                let col_names: Vec<String> = if col_names_str.is_empty() {
                    vec![]
                } else {
                    col_names_str.split('\x1f').map(|s| s.to_string()).collect()
                };
                let col_widths_str: String = row.get(4)?;
                let col_widths: Vec<f64> = if col_widths_str.is_empty() {
                    vec![]
                } else {
                    col_widths_str
                        .split('\x1f')
                        .filter_map(|s| s.parse::<f64>().ok())
                        .collect()
                };
                Ok(SheetMeta {
                    sheet_name: row.get(1)?,
                    table_name: row.get(2)?,
                    col_names,
                    col_widths,
                    file_name: row.get(5)?,
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

            let (where_sql, search_values) = Self::build_wide_where_clause(query, &meta.col_names);

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
            let param_refs: Vec<&dyn duckdb::ToSql> = search_values
                .iter()
                .map(|v| v as &dyn duckdb::ToSql)
                .collect();

            let matched_rows: Vec<Vec<Option<String>>> = search_stmt
                .query_map(param_refs.as_slice(), |row| {
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

                let matched_columns = Self::find_matched_columns(query, &row_vec, &col_names);

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
                SearchMode::Regex => {
                    parts.push(format!(
                        "regexp_matches(CAST({} AS VARCHAR), ?)",
                        quote_ident(col)
                    ));
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

    fn find_matched_columns(
        query: &SearchQuery,
        row: &[String],
        col_names: &[String],
    ) -> Vec<usize> {
        let target_cols: Vec<usize> = if let Some(ref col) = query.column {
            col_names
                .iter()
                .enumerate()
                .filter(|(_, name)| *name == col)
                .map(|(i, _)| i)
                .collect()
        } else {
            (0..row.len()).collect()
        };

        target_cols
            .into_iter()
            .filter(|&i| {
                let value = row.get(i).map(|s| s.as_str()).unwrap_or("");
                match query.mode {
                    SearchMode::FullText => {
                        value.to_lowercase().contains(&query.text.to_lowercase())
                    }
                    SearchMode::ExactMatch => value == query.text,
                    SearchMode::Wildcard => like_match(&query.text, value),
                    SearchMode::Regex => match regex::Regex::new(&format!("(?i){}", query.text)) {
                        Ok(re) => re.is_match(value),
                        Err(_) => false,
                    },
                }
            })
            .collect()
    }

    pub fn list_files(&self) -> Vec<FileInfo> {
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

    pub fn clear(&mut self) -> Result<()> {
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

pub fn export_results_csv(results: &[SearchResult], path: &Path) -> Result<()> {
    if results.is_empty() {
        anyhow::bail!("No results to export");
    }

    let mut all_cols: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for result in results {
        for col in &result.col_names {
            if seen.insert(col.clone()) {
                all_cols.push(col.clone());
            }
        }
    }

    let mut wtr = csv::Writer::from_path(path)?;

    let mut header = vec!["file".to_string(), "sheet".to_string()];
    header.extend(all_cols.clone());
    wtr.write_record(&header)?;

    for result in results {
        let mut row = vec![result.file_name.clone(), result.sheet_name.clone()];
        for col in &all_cols {
            let value = result
                .col_names
                .iter()
                .position(|c| c == col)
                .and_then(|i| result.row.get(i))
                .cloned()
                .unwrap_or_default();
            row.push(value);
        }
        wtr.write_record(&row)?;
    }

    wtr.flush()?;
    Ok(())
}

fn like_match(pattern: &str, text: &str) -> bool {
    fn match_inner(p: &[char], t: &[char]) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }
        if p[0] == '%' {
            if p.len() == 1 {
                return true;
            }
            for i in 0..=t.len() {
                if match_inner(&p[1..], &t[i..]) {
                    return true;
                }
            }
            return false;
        }
        if t.is_empty() {
            return false;
        }
        if p[0] == '_' || p[0].to_ascii_lowercase() == t[0].to_ascii_lowercase() {
            return match_inner(&p[1..], &t[1..]);
        }
        false
    }
    match_inner(
        &pattern.chars().collect::<Vec<_>>(),
        &text.chars().collect::<Vec<_>>(),
    )
}
