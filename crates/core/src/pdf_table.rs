use std::path::Path;

use crate::excel::SheetData;

#[cfg(feature = "pdf-support")]
pub fn parse_pdf(path: &Path) -> anyhow::Result<Vec<SheetData>> {
    use pdfsink_rs::{Orientation, PdfDocument, TableSettings};
    use rayon::prelude::*;

    let pdf = PdfDocument::open(path)
        .map_err(|e| anyhow::anyhow!("failed to open PDF '{}': {}", path.display(), e))?;

    let pages = pdf.pages();

    // Phase 1: Quick pre-check — skip pages unlikely to have tables.
    // Lattice strategy needs at least 2 vertical + 2 horizontal edges
    // to form a 1×1 cell grid.
    let candidates: Vec<(usize, &pdfsink_rs::Page)> = pages
        .iter()
        .enumerate()
        .filter(|(_, page)| {
            let edges = page.edges();
            let v_edges = edges.iter().filter(|e| e.orientation == Orientation::Vertical).count();
            let h_edges = edges.iter().filter(|e| e.orientation == Orientation::Horizontal).count();
            v_edges >= 2 && h_edges >= 2
        })
        .collect();

    // Phase 2: Parallel table extraction on candidate pages only
    let results: Vec<Option<(usize, SheetData)>> = candidates
        .par_iter()
        .map(|(page_idx, page)| {
            let page_num = page_idx + 1;
            match page.extract_table(TableSettings::default()) {
                Ok(Some(table)) => {
                    let string_table: Vec<Vec<String>> = table
                        .into_iter()
                        .map(|row| row.into_iter().map(|c| c.unwrap_or_default()).collect())
                        .collect();

                    if string_table.len() >= 2 && !string_table[0].is_empty() {
                        Some((page_num, SheetData {
                            name: String::new(),
                            headers: string_table[0].clone(),
                            rows: string_table[1..].to_vec(),
                            col_widths: Vec::new(),
                        }))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        })
        .collect();

    let mut page_tables: Vec<(usize, SheetData)> = results.into_iter().flatten().collect();
    page_tables.sort_by_key(|(page_num, _)| *page_num);

    // Phase 3: Merge consecutive-page tables
    let merged = merge_consecutive_tables(page_tables);

    if merged.is_empty() {
        anyhow::bail!(
            "No tables extracted from PDF '{}'. The PDF may not contain bordered tables, or may be scanned (OCR not supported).",
            path.display()
        );
    }

    Ok(merged)
}

/// Merge tables that span consecutive pages into a single `SheetData`.
///
/// Two tables are merged when:
/// - They are on consecutive pages (page N and N+1).
/// - They have the same number of columns.
///
/// If the continuation page repeats the header row, that row is skipped.
fn merge_consecutive_tables(page_tables: Vec<(usize, SheetData)>) -> Vec<SheetData> {
    if page_tables.is_empty() {
        return Vec::new();
    }

    let mut merged: Vec<SheetData> = Vec::new();
    let mut current = page_tables[0].1.clone();
    let mut current_page = page_tables[0].0;

    for (page_num, table) in page_tables.into_iter().skip(1) {
        let same_table = page_num == current_page + 1
            && table.headers.len() == current.headers.len();

        if same_table {
            let data_rows = if first_row_matches_headers(&current.headers, &table.rows) {
                table.rows[1..].to_vec()
            } else {
                table.rows
            };
            current.rows.extend(data_rows);
            current_page = page_num;
        } else {
            merged.push(current);
            current = table;
            current_page = page_num;
        }
    }
    merged.push(current);

    for (i, table) in merged.iter_mut().enumerate() {
        table.name = format!("Table_{}", i + 1);
    }

    merged
}

/// Returns `true` if the first data row of a continuation table looks like
/// repeated headers (all cells match exactly).
fn first_row_matches_headers(headers: &[String], rows: &[Vec<String>]) -> bool {
    if rows.is_empty() || rows[0].len() != headers.len() {
        return false;
    }
    headers == rows[0].as_slice()
}

#[cfg(not(feature = "pdf-support"))]
pub fn parse_pdf(path: &Path) -> anyhow::Result<Vec<SheetData>> {
    let _ = path;
    anyhow::bail!(
        "PDF support is not enabled. Rebuild with --features pdf-support to enable PDF table extraction."
    )
}
