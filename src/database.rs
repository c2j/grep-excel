use anyhow::Result;
use duckdb::{params, Connection};
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::excel::parse_excel;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchMode {
    FullText,
    ExactMatch,
    Wildcard,
}

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub text: String,
    pub column: Option<String>,
    pub mode: SearchMode,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub sheet_name: String,
    pub file_name: String,
    pub row: Vec<String>,
    pub matched_columns: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct SearchStats {
    pub total_rows_searched: usize,
    pub total_matches: usize,
    pub matches_per_sheet: HashMap<String, usize>,
    pub search_duration: Duration,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub sheets: Vec<String>,
    pub total_rows: usize,
}

pub struct Database {
    conn: Connection,
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
                row_count INTEGER DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS cells (
                sheet_id INTEGER NOT NULL,
                row_idx INTEGER NOT NULL,
                col_idx INTEGER NOT NULL,
                col_name TEXT,
                cell_value TEXT,
                PRIMARY KEY (sheet_id, row_idx, col_idx)
            );",
        )?;

        Ok(Database { conn })
    }

    pub fn import_excel(
        &mut self,
        path: &Path,
        progress_callback: impl Fn(usize, usize),
    ) -> Result<FileInfo> {
        let sheets = parse_excel(path)?;

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let total_rows: usize = sheets.iter().map(|s| s.rows.len()).sum();

        self.conn.execute(
            "INSERT INTO files (file_name) VALUES (?)",
            params![&file_name],
        )?;

        let file_id: i64 = self
            .conn
            .query_row("SELECT currval('file_id_seq')", [], |row| row.get(0))?;

        let mut sheet_names = Vec::new();
        let mut processed_rows = 0;

        for sheet in sheets {
            let row_count = sheet.rows.len() as i32;

            self.conn.execute(
                "INSERT INTO sheets (file_id, sheet_name, row_count) VALUES (?, ?, ?)",
                params![file_id, &sheet.name, row_count],
            )?;

            let sheet_id: i64 =
                self.conn
                    .query_row("SELECT currval('sheet_id_seq')", [], |row| row.get(0))?;

            sheet_names.push(sheet.name.clone());

            let tx = self.conn.transaction()?;
            {
                let mut appender = tx.appender("cells")?;

                for (row_idx, row) in sheet.rows.iter().enumerate() {
                    for (col_idx, cell_value) in row.iter().enumerate() {
                        let col_name = sheet.headers.get(col_idx).cloned().unwrap_or_default();

                        appender.append_row(params![
                            sheet_id,
                            row_idx as i32,
                            col_idx as i32,
                            col_name,
                            cell_value,
                        ])?;
                    }

                    processed_rows += 1;
                    progress_callback(processed_rows, total_rows);
                }
            }
            tx.commit()?;
        }

        Ok(FileInfo {
            name: file_name,
            sheets: sheet_names,
            total_rows,
        })
    }

    pub fn search(&self, query: &SearchQuery) -> Result<(Vec<SearchResult>, SearchStats)> {
        let start = Instant::now();

        let mut stmt = self.conn.prepare(
            "SELECT s.sheet_id, s.sheet_name, f.file_name 
             FROM sheets s 
             JOIN files f ON s.file_id = f.file_id",
        )?;

        let sheets_info: Vec<(i64, String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        let total_rows_searched: usize = self.conn.query_row(
            "SELECT COALESCE(SUM(row_count), 0) FROM sheets",
            [],
            |row| row.get::<_, i64>(0),
        )? as usize;

        let (where_clause, search_values) = Self::build_where_clause(query);

        let mut results = Vec::new();
        let mut matches_per_sheet: HashMap<String, usize> = HashMap::new();

        for (sheet_id, sheet_name, file_name) in &sheets_info {
            let matching_rows_sql = format!(
                "SELECT DISTINCT row_idx FROM cells WHERE sheet_id = ? AND ({}) ORDER BY row_idx",
                where_clause
            );

            let mut matching_rows_stmt = self.conn.prepare(&matching_rows_sql)?;

            let mut pv: Vec<&dyn duckdb::ToSql> = vec![sheet_id];
            for v in &search_values {
                pv.push(v);
            }

            let row_indices: Vec<i32> = matching_rows_stmt
                .query_map(pv.as_slice(), |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;

            if row_indices.is_empty() {
                continue;
            }

            matches_per_sheet.insert(sheet_name.clone(), row_indices.len());

            for row_idx in row_indices {
                let mut row_data_stmt = self.conn.prepare(
                    "SELECT col_idx, cell_value FROM cells WHERE sheet_id = ? AND row_idx = ? ORDER BY col_idx",
                )?;
                let cells: Vec<(i32, String)> = row_data_stmt
                    .query_map(params![sheet_id, row_idx], |row| {
                        Ok((row.get(0)?, row.get(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;

                let max_col_idx = cells.iter().map(|(idx, _)| *idx).max().unwrap_or(0);
                let mut row_vec = vec![String::new(); (max_col_idx + 1) as usize];

                for (col_idx, value) in cells {
                    row_vec[col_idx as usize] = value;
                }

                let matched_cols_sql = format!(
                    "SELECT col_idx FROM cells WHERE sheet_id = ? AND row_idx = ? AND ({})",
                    where_clause
                );
                let mut matched_cols_stmt = self.conn.prepare(&matched_cols_sql)?;

                let mut mcp: Vec<&dyn duckdb::ToSql> = vec![sheet_id, &row_idx];
                for v in &search_values {
                    mcp.push(v);
                }

                let matched_columns: Vec<usize> = matched_cols_stmt
                    .query_map(mcp.as_slice(), |row| {
                        row.get::<_, i32>(0).map(|v| v as usize)
                    })?
                    .collect::<Result<Vec<_>, _>>()?;

                results.push(SearchResult {
                    sheet_name: sheet_name.clone(),
                    file_name: file_name.clone(),
                    row: row_vec,
                    matched_columns,
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
            },
        ))
    }

    fn build_where_clause(query: &SearchQuery) -> (String, Vec<String>) {
        let mut parts = Vec::new();
        let mut values = Vec::new();

        match query.mode {
            SearchMode::FullText => {
                let pattern = format!("%{}%", query.text);
                if let Some(ref col) = query.column {
                    parts.push("col_name = ? AND cell_value ILIKE ?".to_string());
                    values.push(col.clone());
                    values.push(pattern);
                } else {
                    parts.push("cell_value ILIKE ?".to_string());
                    values.push(pattern);
                }
            }
            SearchMode::ExactMatch => {
                if let Some(ref col) = query.column {
                    parts.push("col_name = ? AND cell_value = ?".to_string());
                    values.push(col.clone());
                    values.push(query.text.clone());
                } else {
                    parts.push("cell_value = ?".to_string());
                    values.push(query.text.clone());
                }
            }
            SearchMode::Wildcard => {
                if let Some(ref col) = query.column {
                    parts.push("col_name = ? AND cell_value LIKE ?".to_string());
                    values.push(col.clone());
                    values.push(query.text.clone());
                } else {
                    parts.push("cell_value LIKE ?".to_string());
                    values.push(query.text.clone());
                }
            }
        }

        (parts.join(" OR "), values)
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
            });

            if let (Some(name), Some(count)) = (sheet_name, row_count) {
                entry.sheets.push(name);
                entry.total_rows += count as usize;
            }
        }

        files_map.into_values().collect()
    }

    pub fn clear(&mut self) -> Result<()> {
        self.conn.execute("DELETE FROM cells", [])?;
        self.conn.execute("DELETE FROM sheets", [])?;
        self.conn.execute("DELETE FROM files", [])?;
        Ok(())
    }
}
