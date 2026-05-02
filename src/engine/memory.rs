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
            // Filter by sheet name if specified
            if let Some(ref sheet_name) = query.sheet {
                if sheet.sheet_name != *sheet_name {
                    continue;
                }
            }

            total_rows_searched += sheet.rows.len();
            let mut sheet_matches = 0;

            for row in &sheet.rows {
                if results.len() >= query.limit {
                    truncated = true;
                    break;
                }

                let matched_columns = find_matched_columns(query, row, &sheet.headers);
                let is_match = !matched_columns.is_empty();

                // Invert mode: include rows that do NOT match
                if query.invert == is_match {
                    continue;
                }

                results.push(SearchResult {
                    sheet_name: sheet.sheet_name.clone(),
                    file_name: sheet.file_name.clone(),
                    row: row.clone(),
                    col_names: sheet.headers.clone(),
                    matched_columns: if query.invert { vec![] } else { matched_columns },
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

    fn execute_sql(&self, _sql: &str, _limit: usize) -> Result<crate::types::SqlResult> {
        anyhow::bail!(
            "SQL queries are not supported with the memory engine. \
             Rebuild with --features engine-duckdb or engine-sqlite."
        );
    }

    fn list_table_aliases(&self) -> Vec<crate::types::TableAliasInfo> {
        self.sheets
            .iter()
            .enumerate()
            .map(|(idx, s)| {
                let file_stem = std::path::Path::new(&s.file_name)
                    .file_stem()
                    .and_then(|st| st.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let alias = format!("{}.{}", file_stem, s.sheet_name);
                crate::types::TableAliasInfo {
                    table_name: format!("sheet_mem_{}", idx),
                    alias,
                    file_name: s.file_name.clone(),
                    sheet_name: s.sheet_name.clone(),
                    row_count: s.rows.len(),
                    columns: s.headers.clone(),
                }
            })
            .collect()
    }

    fn get_metadata(&self, file_name: &str) -> Result<FileMetadataInfo> {
        let sheets: Vec<&MemSheet> = self.sheets.iter()
            .filter(|s| s.file_name == file_name)
            .collect();

        if sheets.is_empty() {
            anyhow::bail!("File '{}' not found. Use list_files to see imported files.", file_name);
        }

        let sheet_infos: Vec<SheetMetadataInfo> = sheets.iter()
            .map(|s| SheetMetadataInfo {
                sheet_name: s.sheet_name.clone(),
                row_count: s.rows.len(),
                columns: s.headers.clone(),
            })
            .collect();

        Ok(FileMetadataInfo {
            file_name: file_name.to_string(),
            sheet_count: sheet_infos.len(),
            sheets: sheet_infos,
        })
    }

    fn get_sheet_sample(&self, file_name: &str, sheet_name: &str, sample_size: usize) -> Result<SheetDataResult> {
        let sheet = self.sheets.iter()
            .find(|s| s.file_name == file_name && s.sheet_name == sheet_name)
            .ok_or_else(|| anyhow::anyhow!(
                "Sheet '{}' in file '{}' not found. Use get_metadata to discover sheets.",
                sheet_name, file_name
            ))?;

        let total_rows = sheet.rows.len();
        let sample_size = sample_size.min(total_rows);

        let mut sampled = Vec::new();
        if sample_size > 0 && total_rows > 0 {
            if sample_size >= total_rows {
                sampled = sheet.rows.clone();
            } else {
                for i in 0..sample_size {
                    let idx = i * total_rows / sample_size;
                    sampled.push(sheet.rows[idx].clone());
                }
            }
        }

        Ok(SheetDataResult {
            file_name: file_name.to_string(),
            sheet_name: sheet_name.to_string(),
            columns: sheet.headers.clone(),
            rows: sampled,
            row_count: sample_size.min(total_rows),
            total_rows,
            truncated: sample_size < total_rows,
        })
    }

    fn get_sheet_data(
        &self,
        file_name: &str,
        sheet_name: &str,
        start_row: Option<usize>,
        end_row: Option<usize>,
        columns: Option<&[String]>,
    ) -> Result<SheetDataResult> {
        let sheet = self.sheets.iter()
            .find(|s| s.file_name == file_name && s.sheet_name == sheet_name)
            .ok_or_else(|| anyhow::anyhow!(
                "Sheet '{}' in file '{}' not found. Use get_metadata to discover sheets.",
                sheet_name, file_name
            ))?;

        let total_rows = sheet.rows.len();
        let start = start_row.unwrap_or(0).min(total_rows);
        let end = end_row.unwrap_or(total_rows).min(total_rows);

        let rows_slice = &sheet.rows[start..end];

        let (col_indices, result_columns): (Vec<usize>, Vec<String>) = if let Some(cols) = columns {
            let indices: Vec<usize> = cols.iter()
                .filter_map(|c| sheet.headers.iter().position(|h| h == c))
                .collect();
            let names: Vec<String> = indices.iter()
                .map(|&i| sheet.headers[i].clone())
                .collect();
            (indices, names)
        } else {
            let indices: Vec<usize> = (0..sheet.headers.len()).collect();
            (indices, sheet.headers.clone())
        };

        let result_rows: Vec<Vec<String>> = rows_slice.iter()
            .map(|row| {
                col_indices.iter()
                    .map(|&i| row.get(i).cloned().unwrap_or_default())
                    .collect()
            })
            .collect();

        let row_count = result_rows.len();

        Ok(SheetDataResult {
            file_name: file_name.to_string(),
            sheet_name: sheet_name.to_string(),
            columns: result_columns,
            rows: result_rows,
            row_count,
            total_rows,
            truncated: false,
        })
    }

    fn save_as(&self, _file_name: &str, _output_path: &Path) -> Result<()> {
        #[cfg(feature = "mcp-server")]
        {
            use crate::engine::write_xlsx;

            let sheets: Vec<&MemSheet> = self.sheets.iter()
                .filter(|s| s.file_name == _file_name)
                .collect();

            if sheets.is_empty() {
                anyhow::bail!("File '{}' not found. Use list_files to see imported files.", _file_name);
            }

            let sheet_data: Vec<(&str, &[String], &[Vec<String>])> = sheets.iter()
                .map(|s| (s.sheet_name.as_str(), &s.headers[..], &s.rows[..]))
                .collect();

            return write_xlsx(&sheet_data, _output_path);
        }
        #[cfg(not(feature = "mcp-server"))]
        anyhow::bail!("save_as requires the mcp-server feature")
    }

    fn update_cell(&mut self, file_name: &str, sheet_name: &str, row: usize, column: &str, value: &str) -> Result<()> {
        let sheet = self.sheets.iter_mut()
            .find(|s| s.file_name == file_name && s.sheet_name == sheet_name)
            .ok_or_else(|| anyhow::anyhow!(
                "Sheet '{}' in file '{}' not found. Use get_metadata to discover sheets.",
                sheet_name, file_name
            ))?;

        let col_idx = sheet.headers.iter()
            .position(|h| h == column)
            .ok_or_else(|| anyhow::anyhow!(
                "Column '{}' not found in sheet '{}'.", column, sheet_name
            ))?;

        if row >= sheet.rows.len() {
            anyhow::bail!("Row index {} is out of bounds (total rows: {}).", row, sheet.rows.len());
        }

        sheet.rows[row][col_idx] = value.to_string();
        Ok(())
    }

    fn update_cells(&mut self, file_name: &str, sheet_name: &str, updates: &[(usize, String, String)]) -> Result<usize> {
        let sheet = self.sheets.iter_mut()
            .find(|s| s.file_name == file_name && s.sheet_name == sheet_name)
            .ok_or_else(|| anyhow::anyhow!(
                "Sheet '{}' in file '{}' not found. Use get_metadata to discover sheets.",
                sheet_name, file_name
            ))?;

        let mut count = 0;
        for (row, column, value) in updates {
            if let Some(col_idx) = sheet.headers.iter().position(|h| h == column) {
                if *row < sheet.rows.len() {
                    sheet.rows[*row][col_idx] = value.clone();
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    fn insert_rows(&mut self, file_name: &str, sheet_name: &str, start_row: usize, rows: Vec<Vec<String>>) -> Result<()> {
        let sheet = self.sheets.iter_mut()
            .find(|s| s.file_name == file_name && s.sheet_name == sheet_name)
            .ok_or_else(|| anyhow::anyhow!(
                "Sheet '{}' in file '{}' not found. Use get_metadata to discover sheets.",
                sheet_name, file_name
            ))?;

        let col_count = sheet.headers.len();
        let mut padded_rows: Vec<Vec<String>> = rows.into_iter()
            .map(|mut row| {
                if row.len() < col_count {
                    row.resize(col_count, String::new());
                } else if row.len() > col_count {
                    row.truncate(col_count);
                }
                row
            })
            .collect();

        let insert_pos = start_row.min(sheet.rows.len());
        if insert_pos == sheet.rows.len() {
            sheet.rows.append(&mut padded_rows);
        } else {
            sheet.rows.splice(insert_pos..insert_pos, padded_rows);
        }
        Ok(())
    }

    fn delete_rows(&mut self, file_name: &str, sheet_name: &str, start_row: usize, count: usize) -> Result<usize> {
        let sheet = self.sheets.iter_mut()
            .find(|s| s.file_name == file_name && s.sheet_name == sheet_name)
            .ok_or_else(|| anyhow::anyhow!(
                "Sheet '{}' in file '{}' not found. Use get_metadata to discover sheets.",
                sheet_name, file_name
            ))?;

        if start_row >= sheet.rows.len() {
            return Ok(0);
        }

        let end = (start_row + count).min(sheet.rows.len());
        let deleted = end - start_row;
        sheet.rows.drain(start_row..end);
        Ok(deleted)
    }

    fn add_column(&mut self, file_name: &str, sheet_name: &str, column_name: &str, default_value: &str) -> Result<()> {
        let sheet = self.sheets.iter_mut()
            .find(|s| s.file_name == file_name && s.sheet_name == sheet_name)
            .ok_or_else(|| anyhow::anyhow!(
                "Sheet '{}' in file '{}' not found. Use get_metadata to discover sheets.",
                sheet_name, file_name
            ))?;

        if sheet.headers.iter().any(|h| h == column_name) {
            anyhow::bail!("Column '{}' already exists in sheet '{}'.", column_name, sheet_name);
        }

        sheet.headers.push(column_name.to_string());
        sheet.col_widths.push(10.0);
        for row in &mut sheet.rows {
            row.push(default_value.to_string());
        }
        Ok(())
    }

    fn rename_column(&mut self, file_name: &str, sheet_name: &str, old_name: &str, new_name: &str) -> Result<()> {
        let sheet = self.sheets.iter_mut()
            .find(|s| s.file_name == file_name && s.sheet_name == sheet_name)
            .ok_or_else(|| anyhow::anyhow!(
                "Sheet '{}' in file '{}' not found. Use get_metadata to discover sheets.",
                sheet_name, file_name
            ))?;

        let col_idx = sheet.headers.iter()
            .position(|h| h == old_name)
            .ok_or_else(|| anyhow::anyhow!(
                "Column '{}' not found in sheet '{}'.", old_name, sheet_name
            ))?;

        if old_name != new_name && sheet.headers.iter().any(|h| h == new_name) {
            anyhow::bail!("Column '{}' already exists in sheet '{}'.", new_name, sheet_name);
        }

        sheet.headers[col_idx] = new_name.to_string();
        Ok(())
    }
}
