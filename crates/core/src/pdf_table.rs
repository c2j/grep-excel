use std::path::Path;

use crate::excel::SheetData;

#[cfg(feature = "pdf-support")]
pub fn parse_pdf(path: &Path) -> anyhow::Result<Vec<SheetData>> {
    use pdfsink_rs::{PdfDocument, TableSettings};

    let pdf = PdfDocument::open(path)
        .map_err(|e| anyhow::anyhow!("failed to open PDF '{}': {}", path.display(), e))?;

    let page_count = pdf.len();
    let mut all_tables: Vec<SheetData> = Vec::new();

    for page_num in 1..=page_count {
        let page = pdf
            .page(page_num)
            .map_err(|e| anyhow::anyhow!("failed to read page {} of '{}': {}", page_num, path.display(), e))?;

        // Try lattice strategy first (bordered tables, highest accuracy)
        if let Some(table) = page
            .extract_table(TableSettings::default())
            .map_err(|e| anyhow::anyhow!("table extraction failed on page {}: {}", page_num, e))?
        {
            let string_table = table
                .into_iter()
                .map(|row| row.into_iter().map(|c| c.unwrap_or_default()).collect::<Vec<_>>())
                .collect::<Vec<_>>();

            if string_table.len() >= 2 && !string_table[0].is_empty() {
                let headers = string_table[0].clone();
                let rows = string_table[1..].to_vec();
                let name = if page_count == 1 {
                    format!("Table_1")
                } else {
                    format!("Page_{}_Table_{}", page_num, all_tables.len() + 1)
                };
                all_tables.push(SheetData {
                    name,
                    headers,
                    rows,
                    col_widths: Vec::new(),
                });
            }
        }
    }

    if all_tables.is_empty() {
        anyhow::bail!(
            "No tables extracted from PDF '{}'. The PDF may not contain bordered tables, or may be scanned (OCR not supported).",
            path.display()
        );
    }

    Ok(all_tables)
}

#[cfg(not(feature = "pdf-support"))]
pub fn parse_pdf(path: &Path) -> anyhow::Result<Vec<SheetData>> {
    let _ = path;
    anyhow::bail!(
        "PDF support is not enabled. Rebuild with --features pdf-support to enable PDF table extraction."
    )
}
