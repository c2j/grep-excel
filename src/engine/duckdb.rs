#![cfg(feature = "engine-duckdb")]

use super::*;
use crate::excel::for_each_sheet;
use anyhow::Result;
use ::duckdb::{params, Connection};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

struct SheetQueryMeta {
    table_name: String,
    col_names: Vec<String>,
    row_count: usize,
}

pub struct DuckDbEngine {
    conn: Connection,
}

impl SearchEngine for DuckDbEngine {
    fn new() -> Result<Self>
    where
        Self: Sized,
    {
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

        Ok(DuckDbEngine { conn })
    }

    fn import_excel(&mut self, path: &Path, progress: &dyn Fn(usize, usize)) -> Result<FileInfo> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if ext == "csv" {
            return self.import_csv_direct(path, progress);
        }

        self.import_excel_sheets(path, progress)
    }

    fn search(&self, query: &SearchQuery) -> Result<(Vec<SearchResult>, SearchStats)> {
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

        let sheets_info: Vec<SheetMeta> = {
            let mapped = stmt.query_map([], |row: &::duckdb::Row| {
                let col_names_str: String = row.get(3)?;
                let col_names: Vec<String> = if col_names_str.is_empty() {
                    vec![]
                } else {
                    col_names_str
                        .split('\x1f')
                        .map(|s: &str| s.to_string())
                        .collect()
                };
                let col_widths_str: String = row.get(4)?;
                let col_widths: Vec<f64> = if col_widths_str.is_empty() {
                    vec![]
                } else {
                    col_widths_str
                        .split('\x1f')
                        .filter_map(|s: &str| s.parse::<f64>().ok())
                        .collect()
                };
                Ok(SheetMeta {
                    sheet_name: row.get(1)?,
                    table_name: row.get(2)?,
                    col_names,
                    col_widths,
                    file_name: row.get(5)?,
                })
            })?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };

        let total_rows_searched: usize = self.conn.query_row(
            "SELECT COALESCE(SUM(row_count), 0) FROM sheets",
            [],
            |row: &::duckdb::Row| row.get::<_, i64>(0),
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

            let (where_sql, search_values) = Self::build_wide_where_clause(query, &meta.col_names);

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
                if query.invert {
                    "1=1".to_string()
                } else {
                    "1=0".to_string()
                }
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
            let param_refs: Vec<&dyn::duckdb::ToSql> = search_values
                .iter()
                .map(|v| v as &dyn::duckdb::ToSql)
                .collect();

            let matched_rows: Vec<Vec<Option<String>>> = {
                let mapped =
                    search_stmt.query_map(param_refs.as_slice(), |row: &::duckdb::Row| {
                        let mut values = Vec::new();
                        for i in 0..meta.col_names.len() {
                            values.push(row.get::<_, Option<String>>(i)?);
                        }
                        Ok(values)
                    })?;
                mapped.collect::<Result<Vec<_>, _>>()?
            };

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
                    .map(|v: &Option<String>| v.clone().unwrap_or_default())
                    .collect();
                let col_names = meta.col_names.clone();

                let matched_columns = super::find_matched_columns(query, &row_vec, &col_names);

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

        let rows: Vec<(String, Option<String>, Option<i32>)> =
            match stmt.query_map([], |row: &::duckdb::Row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i32>>(2)?,
                ))
            }) {
                Ok(mapped) => mapped
                    .filter_map(|r: Result<_, ::duckdb::Error>| r.ok())
                    .collect(),
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
        let table_names: Vec<String> = {
            let mapped = stmt.query_map([], |row: &::duckdb::Row| row.get::<_, String>(0))?;
            mapped
                .filter_map(|r: Result<String, ::duckdb::Error>| r.ok())
                .collect()
        };

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

        fn execute_sql(&self, sql: &str, limit: usize) -> Result<crate::types::SqlResult> {
            super::validate_sql(sql)?;
            let start = std::time::Instant::now();

            let limited_sql = format!("SELECT * FROM ({}) LIMIT {}", sql, limit);
            let mut stmt = self.conn.prepare(&limited_sql)?;
            let mut rows = stmt.query([])?;
            let columns = rows.as_ref().unwrap().column_names();
            let col_count = columns.len();

            let mut result_rows = Vec::new();
            while let Some(row) = rows.next()? {
                let mut values = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    values.push(row.get::<_, Option<String>>(i)?.unwrap_or_default());
                }
                result_rows.push(values);
            }

            let row_count = result_rows.len();
            let truncated = row_count >= limit;
            let duration = start.elapsed();

            Ok(crate::types::SqlResult {
                columns,
                rows: result_rows,
                row_count,
                truncated,
                duration,
            })
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
                "SELECT {} FROM {} USING SAMPLE {}",
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
}

impl DuckDbEngine {
    fn import_csv_direct(
        &mut self,
        path: &Path,
        progress_callback: &dyn Fn(usize, usize),
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
        let file_id: i64 = self.conn.query_row(
            "SELECT currval('file_id_seq')",
            [],
            |row: &::duckdb::Row| row.get::<_, i64>(0),
        )?;

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
            |row: &::duckdb::Row| row.get::<_, i64>(0),
        )?;

        let col_names: Vec<String> = {
            let mut stmt = self.conn.prepare(
                "SELECT column_name FROM information_schema.columns WHERE table_name = ? ORDER BY ordinal_position"
            )?;
            let mapped = stmt.query_map(params![&table_name], |row: &::duckdb::Row| {
                row.get::<_, String>(0)
            })?;
            mapped.collect::<Result<Vec<_>, _>>()?
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
        let rows: Vec<Vec<String>> = {
            let mapped = stmt.query_map([], |row: &::duckdb::Row| {
                let mut values = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    values.push(row.get::<_, Option<String>>(i)?.unwrap_or_default());
                }
                Ok(values)
            })?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    fn import_excel_sheets(
        &mut self,
        path: &Path,
        progress_callback: &dyn Fn(usize, usize),
    ) -> Result<FileInfo> {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        self.conn.execute(
            "INSERT INTO files (file_name) VALUES (?)",
            params![&file_name],
        )?;

        let file_id: i64 = self.conn.query_row(
            "SELECT currval('file_id_seq')",
            [],
            |row: &::duckdb::Row| row.get::<_, i64>(0),
        )?;

        let mut sheet_info: Vec<(String, usize)> = Vec::new();
        let mut sample: Option<FileSample> = None;
        let mut processed_rows: usize = 0;
        let mut total_rows: usize = 0;

        let file_id_capture = file_id;
        let conn = &mut self.conn;
        let sample_capture = &mut sample;
        let sheet_info_capture = &mut sheet_info;
        let processed_rows_capture = &mut processed_rows;
        let total_rows_capture = &mut total_rows;
        let progress_callback_capture = progress_callback;

        for_each_sheet(path, |sheet_data, sheet_idx| {
            let row_count = sheet_data.rows.len();
            let col_names = sanitize_col_names(&sheet_data.headers);
            let table_name = format!("sheet_{}_{}", file_id_capture, sheet_idx);

            let col_defs: Vec<String> = col_names
                .iter()
                .map(|c| format!("{} TEXT", quote_ident(c)))
                .collect();
            let create_sql = format!(
                "CREATE TABLE {} ({})",
                quote_ident(&table_name),
                col_defs.join(", ")
            );
            conn.execute(&create_sql, [])?;

            *total_rows_capture += row_count;

            let tx = conn.transaction()?;
            {
                let mut appender = tx.appender(&table_name)?;
                for row in &sheet_data.rows {
                    let mut padded_row = row.clone();
                    padded_row.resize(col_names.len(), String::new());
                    let param_refs: Vec<&dyn::duckdb::ToSql> = padded_row
                        .iter()
                        .map(|s| s as &dyn::duckdb::ToSql)
                        .collect();
                    appender.append_row(param_refs.as_slice())?;
                    *processed_rows_capture += 1;
                    progress_callback_capture(*processed_rows_capture, *total_rows_capture);
                }
            }
            tx.commit()?;

            let col_names_str = col_names.join("\x1f");
            let col_widths_str = sheet_data
                .col_widths
                .iter()
                .map(|w| format!("{}", w))
                .collect::<Vec<_>>()
                .join("\x1f");
            conn.execute(
                "INSERT INTO sheets (file_id, sheet_name, table_name, row_count, col_names, col_widths) VALUES (?, ?, ?, ?, ?, ?)",
                params![file_id_capture, &sheet_data.name, &table_name, row_count as i32, &col_names_str, &col_widths_str],
            )?;

            for col_name in &col_names {
                let safe_name = col_name.replace(|c: char| !c.is_alphanumeric() && c != '_', "_");
                let index_name = format!("idx_{}_{}", table_name, safe_name);
                let _ = conn.execute(
                    &format!(
                        "CREATE INDEX IF NOT EXISTS \"{}\" ON {} ({})",
                        index_name,
                        quote_ident(&table_name),
                        quote_ident(col_name)
                    ),
                    [],
                );
            }

            if sample_capture.is_none() {
                *sample_capture = Some(FileSample {
                    sheet_name: sheet_data.name.clone(),
                    headers: sheet_data.headers.clone(),
                    rows: sheet_data.rows.iter().take(3).cloned().collect(),
                });
            }

            sheet_info_capture.push((sheet_data.name, row_count));

            Ok(())
        })?;

        if total_rows > 0 {
            progress_callback(processed_rows, total_rows);
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

}
