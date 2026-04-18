use super::*;
use crate::excel::parse_file;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

struct MemSheet {
    file_name: String,
    sheet_name: String,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    col_widths: Vec<f64>,
}

pub struct MemEngine {
    sheets: Vec<MemSheet>,
}

impl SearchEngine for MemEngine {
    fn new() -> Result<Self> {
        Ok(MemEngine { sheets: Vec::new() })
    }

    fn import_excel(&mut self, path: &Path, progress: &dyn Fn(usize, usize)) -> Result<FileInfo> {
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

        let mut sheet_info = Vec::new();
        for sheet in sheets {
            sheet_info.push((sheet.name.clone(), sheet.rows.len()));
            self.sheets.push(MemSheet {
                file_name: file_name.clone(),
                sheet_name: sheet.name,
                headers: sheet.headers,
                rows: sheet.rows,
                col_widths: sheet.col_widths,
            });
        }

        progress(total_rows, total_rows);

        Ok(FileInfo {
            name: file_name,
            sheets: sheet_info,
            total_rows,
            sample,
        })
    }

    fn search(&self, query: &SearchQuery) -> Result<(Vec<SearchResult>, SearchStats)> {
        let start = Instant::now();
        let mut results = Vec::new();
        let mut matches_per_sheet: HashMap<String, usize> = HashMap::new();
        let mut total_rows_searched: usize = 0;
        let mut truncated = false;

        for sheet in &self.sheets {
            if results.len() >= query.limit {
                truncated = true;
                break;
            }
            if sheet.headers.is_empty() {
                continue;
            }

            total_rows_searched += sheet.rows.len();
            let mut sheet_matches = 0;

            for row in &sheet.rows {
                if results.len() >= query.limit {
                    truncated = true;
                    break;
                }

                let matched_columns = find_matched_columns(query, row, &sheet.headers);
                if matched_columns.is_empty() {
                    continue;
                }

                results.push(SearchResult {
                    sheet_name: sheet.sheet_name.clone(),
                    file_name: sheet.file_name.clone(),
                    row: row.clone(),
                    col_names: sheet.headers.clone(),
                    matched_columns,
                    col_widths: sheet.col_widths.clone(),
                });
                sheet_matches += 1;
            }

            if sheet_matches > 0 {
                matches_per_sheet.insert(sheet.sheet_name.clone(), sheet_matches);
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
        let mut files_map: HashMap<String, FileInfo> = HashMap::new();

        for sheet in &self.sheets {
            let entry = files_map
                .entry(sheet.file_name.clone())
                .or_insert_with(|| FileInfo {
                    name: sheet.file_name.clone(),
                    sheets: Vec::new(),
                    total_rows: 0,
                    sample: None,
                });
            entry
                .sheets
                .push((sheet.sheet_name.clone(), sheet.rows.len()));
            entry.total_rows += sheet.rows.len();
        }

        if let Some(first) = self.sheets.first() {
            if let Some(entry) = files_map.get_mut(&first.file_name) {
                entry.sample = Some(FileSample {
                    sheet_name: first.sheet_name.clone(),
                    headers: first.headers.clone(),
                    rows: first.rows.iter().take(3).cloned().collect(),
                });
            }
        }

        files_map.into_values().collect()
    }

    fn clear(&mut self) -> Result<()> {
        self.sheets.clear();
        Ok(())
    }
}
