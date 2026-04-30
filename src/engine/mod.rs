pub use crate::types::*;
use anyhow::Result;
use std::path::Path;

pub trait SearchEngine: Send {
    fn new() -> Result<Self>
    where
        Self: Sized;
    fn import_excel(
        &mut self,
        path: &Path,
        progress: &dyn Fn(usize, usize),
    ) -> Result<FileInfo>;
    fn search(&self, query: &SearchQuery) -> Result<(Vec<SearchResult>, SearchStats)>;
    fn list_files(&self) -> Vec<FileInfo>;
    fn clear(&mut self) -> Result<()>;
    fn execute_sql(&self, sql: &str, limit: usize) -> Result<crate::types::SqlResult>;
    fn list_table_aliases(&self) -> Vec<crate::types::TableAliasInfo>;

    #[cfg(feature = "mcp-server")]
    fn get_metadata(&self, file_name: &str) -> Result<FileMetadataInfo>;
    #[cfg(feature = "mcp-server")]
    fn get_sheet_sample(&self, file_name: &str, sheet_name: &str, sample_size: usize) -> Result<SheetDataResult>;
    #[cfg(feature = "mcp-server")]
    fn get_sheet_data(
        &self,
        file_name: &str,
        sheet_name: &str,
        start_row: Option<usize>,
        end_row: Option<usize>,
        columns: Option<&[String]>,
    ) -> Result<SheetDataResult>;
    #[cfg(feature = "mcp-server")]
    fn save_as(&self, file_name: &str, output_path: &Path) -> Result<()>;
}

// ── Shared helpers ──────────────────────────────────────────────────────────

pub fn find_matched_columns(
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
                SearchMode::FullText => value
                    .to_lowercase()
                    .contains(&query.text.to_lowercase()),
                SearchMode::ExactMatch => value == query.text,
                SearchMode::Wildcard => like_match(&query.text, value),
                SearchMode::Regex => regex::Regex::new(&format!("(?i){}", query.text))
                    .map(|re| re.is_match(value))
                    .unwrap_or(false),
            }
        })
        .collect()
}

pub fn like_match(pattern: &str, text: &str) -> bool {
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

pub(crate) fn sanitize_col_names(headers: &[String]) -> Vec<String> {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
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

pub(crate) fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

pub(crate) fn sanitize_schema_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    match sanitized.to_lowercase().as_str() {
        "main" | "information_schema" | "pg_catalog" | "sys" => {
            format!("file_{}", sanitized)
        }
        _ => sanitized,
    }
}

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
    // Reject multi-statement attempts (semicolon could chain commands)
    if sql.contains(';') {
        anyhow::bail!("Multiple SQL statements are not allowed. Remove semicolons.");
    }
    // Reject common DDL/DML keywords (check with leading space or at start)
    let upper_with_space = format!(" {} ", upper);
    for forbidden in &[
        " INSERT ", " UPDATE ", " DELETE ", " DROP ", " CREATE ", " ALTER ",
        " ATTACH ", " DETACH ", " COPY ", " EXPORT ", " PRAGMA ",
        " TRUNCATE ", " GRANT ", " REVOKE ", " VACUUM ", " REINDEX ",
        " CALL ", " LOAD ", " INSTALL ", " ANALYZE ",
    ] {
        if upper_with_space.contains(forbidden) {
            anyhow::bail!(
                "Forbidden keyword found: {}. Only SELECT queries are allowed.",
                forbidden.trim()
            );
        }
    }
    Ok(())
}

// ── Conditional engine selection ────────────────────────────────────────────

#[cfg(feature = "engine-duckdb")]
mod duckdb;

#[cfg(feature = "engine-sqlite")]
mod sqlite;

#[cfg(feature = "engine-memory")]
mod memory;

#[cfg(feature = "engine-duckdb")]
pub use duckdb::DuckDbEngine as DefaultEngine;

#[cfg(all(feature = "engine-sqlite", not(feature = "engine-duckdb")))]
pub use sqlite::SqliteEngine as DefaultEngine;

#[cfg(all(feature = "engine-memory", not(any(feature = "engine-duckdb", feature = "engine-sqlite"))))]
pub use memory::MemEngine as DefaultEngine;

#[cfg(not(any(
    feature = "engine-duckdb",
    feature = "engine-sqlite",
    feature = "engine-memory"
)))]
compile_error!("Enable one engine feature: engine-duckdb, engine-sqlite, or engine-memory");

#[cfg(feature = "mcp-server")]
pub fn write_xlsx(
    sheets: &[(&str, &[String], &[Vec<String>])],
    output_path: &Path,
) -> Result<()> {
    use rust_xlsxwriter::Workbook;

    let mut workbook = Workbook::new();

    for (sheet_name, headers, rows) in sheets {
        let worksheet = workbook.add_worksheet()
            .set_name(*sheet_name)
            .map_err(|e| anyhow::anyhow!("Failed to create sheet '{}': {}", sheet_name, e))?;

        for (col_idx, header) in headers.iter().enumerate() {
            worksheet.write_string(0, col_idx as u16, header)
                .map_err(|e| anyhow::anyhow!("Write error: {}", e))?;
        }

        for (row_idx, row) in rows.iter().enumerate() {
            for (col_idx, value) in row.iter().enumerate() {
                if let Ok(num) = value.parse::<f64>() {
                    worksheet.write_number((row_idx + 1) as u32, col_idx as u16, num)
                        .map_err(|e| anyhow::anyhow!("Write error: {}", e))?;
                } else {
                    worksheet.write_string((row_idx + 1) as u32, col_idx as u16, value)
                        .map_err(|e| anyhow::anyhow!("Write error: {}", e))?;
                }
            }
        }
    }

    workbook.save(output_path)
        .map_err(|e| anyhow::anyhow!("Failed to save xlsx: {}", e))?;

    Ok(())
}

pub fn find_match_spans(mode: SearchMode, query: &str, value: &str) -> Vec<(usize, usize)> {
    if query.is_empty() || value.is_empty() {
        return vec![];
    }
    match mode {
        SearchMode::FullText => {
            let escaped = regex::escape(query);
            regex::Regex::new(&format!("(?i){}", escaped))
                .map(|re| re.find_iter(value).map(|m| (m.start(), m.end())).collect())
                .unwrap_or_default()
        }
        SearchMode::Regex => regex::Regex::new(&format!("(?i){}", query))
            .map(|re| re.find_iter(value).map(|m| (m.start(), m.end())).collect())
            .unwrap_or_default(),
        SearchMode::ExactMatch => {
            if value == query {
                vec![(0, value.len())]
            } else {
                vec![]
            }
        }
        SearchMode::Wildcard => vec![(0, value.len())],
    }
}
