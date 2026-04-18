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

// ── Conditional engine selection ────────────────────────────────────────────

#[cfg(feature = "engine-duckdb")]
mod duckdb;

#[cfg(feature = "engine-sqlite")]
mod sqlite;

#[cfg(feature = "engine-memory")]
mod memory;

#[cfg(feature = "engine-duckdb")]
pub use duckdb::DuckDbEngine as DefaultEngine;

#[cfg(feature = "engine-sqlite")]
pub use sqlite::SqliteEngine as DefaultEngine;

#[cfg(feature = "engine-memory")]
pub use memory::MemEngine as DefaultEngine;

#[cfg(not(any(
    feature = "engine-duckdb",
    feature = "engine-sqlite",
    feature = "engine-memory"
)))]
compile_error!("Enable one engine feature: engine-duckdb, engine-sqlite, or engine-memory");

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
