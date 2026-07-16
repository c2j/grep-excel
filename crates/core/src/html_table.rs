/// A single table extracted from an HTML file.
/// Mirrors the sheet-level abstraction used by Excel import.
#[derive(Debug, Clone)]
pub struct HtmlTable {
    /// Table name derived from summary attr, preceding heading, or index
    pub name: String,
    /// Column headers (from <th> elements in first row)
    pub headers: Vec<String>,
    /// Data rows (each row = Vec of cell text)
    pub rows: Vec<Vec<String>>,
}

/// Extract all <table> elements from an HTML string.
/// Returns a Vec of HtmlTable, one per <table> found.
pub fn extract_tables(html: &str) -> Result<Vec<HtmlTable>, String> {
    use scraper::element_ref::ElementRef;
    use scraper::{Html, Selector};

    let document = Html::parse_document(html);

    let th_selector =
        Selector::parse("th").map_err(|e| format!("Failed to parse th selector: {}", e))?;
    let tr_selector =
        Selector::parse("tr").map_err(|e| format!("Failed to parse tr selector: {}", e))?;
    let td_selector =
        Selector::parse("td").map_err(|e| format!("Failed to parse td selector: {}", e))?;

    let mut tables = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut table_count = 0;

    // Walk the DOM tree in document order (depth-first).
    // Track the most recent <h3> text as a candidate table name.
    for node in document.root_element().descendants() {
        let element = match ElementRef::wrap(node) {
            Some(el) => el,
            None => continue,
        };

        match element.value().name() as &str {
            "h3" => {
                let text = element.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    current_heading = Some(text);
                }
            }
            "table" => {
                table_count += 1;

                let name = element
                    .value()
                    .attr("summary")
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty())
                    .or_else(|| current_heading.clone())
                    .unwrap_or_else(|| format!("Table_{}", table_count));

                let mut headers = Vec::new();
                let mut rows = Vec::new();
                let mut header_row_seen = false;

                for tr_elem in element.select(&tr_selector) {
                    let th_cells: Vec<String> = tr_elem
                        .select(&th_selector)
                        .map(|th| th.text().collect::<String>().trim().to_string())
                        .collect();

                    if !th_cells.is_empty() && !header_row_seen {
                        headers = th_cells;
                        header_row_seen = true;
                        continue;
                    }

                    let cells: Vec<String> = tr_elem
                        .select(&td_selector)
                        .map(|td| td.text().collect::<String>().trim().to_string())
                        .collect();

                    if !cells.is_empty() {
                        if th_cells.len() == 1 {
                            let mut row = th_cells;
                            row.extend(cells);
                            rows.push(row);
                        } else {
                            rows.push(cells);
                        }
                    }
                }

                if headers.is_empty() && !rows.is_empty() {
                    headers = rows.remove(0);
                }

                tables.push(HtmlTable {
                    name,
                    headers,
                    rows,
                });
            }
            _ => {}
        }
    }

    Ok(tables)
}

/// Lightweight table metadata without materializing rows.
/// Used by `-t` mode to quickly show file schema for large HTML files.
#[derive(Debug, Clone)]
pub struct TableMetadata {
    /// Table name derived from summary attr, preceding heading, or index
    pub name: String,
    /// Column headers (from <th> elements in first row)
    pub headers: Vec<String>,
    /// Number of data rows (excludes header row)
    pub row_count: usize,
}

/// Extract table metadata (name, headers, row_count) without materializing any row data.
/// Mirrors the DOM-walk logic in `extract_tables` exactly: same selectors, name resolution,
/// header detection, and mixed th/td handling — but only increments a counter instead of
/// collecting row vectors.
pub fn extract_table_metadata(html: &str) -> Result<Vec<TableMetadata>, String> {
    use scraper::element_ref::ElementRef;
    use scraper::{Html, Selector};

    let document = Html::parse_document(html);

    let th_selector =
        Selector::parse("th").map_err(|e| format!("Failed to parse th selector: {}", e))?;
    let tr_selector =
        Selector::parse("tr").map_err(|e| format!("Failed to parse tr selector: {}", e))?;
    let td_selector =
        Selector::parse("td").map_err(|e| format!("Failed to parse td selector: {}", e))?;

    let mut tables = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut table_count = 0;

    for node in document.root_element().descendants() {
        let element = match ElementRef::wrap(node) {
            Some(el) => el,
            None => continue,
        };

        match element.value().name() as &str {
            "h3" => {
                let text = element.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    current_heading = Some(text);
                }
            }
            "table" => {
                table_count += 1;

                let name = element
                    .value()
                    .attr("summary")
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty())
                    .or_else(|| current_heading.clone())
                    .unwrap_or_else(|| format!("Table_{}", table_count));

                let mut headers = Vec::new();
                let mut row_count = 0;
                let mut header_row_seen = false;

                for tr_elem in element.select(&tr_selector) {
                    let th_cells: Vec<String> = tr_elem
                        .select(&th_selector)
                        .map(|th| th.text().collect::<String>().trim().to_string())
                        .collect();

                    if !th_cells.is_empty() && !header_row_seen {
                        headers = th_cells;
                        header_row_seen = true;
                        continue;
                    }

                    let cells: Vec<String> = tr_elem
                        .select(&td_selector)
                        .map(|td| td.text().collect::<String>().trim().to_string())
                        .collect();

                    if !cells.is_empty() {
                        if !header_row_seen {
                            // No <th> row found — first data row becomes headers
                            headers = cells;
                            header_row_seen = true;
                        } else {
                            row_count += 1;
                        }
                    }
                }

                tables.push(TableMetadata {
                    name,
                    headers,
                    row_count,
                });
            }
            _ => {}
        }
    }

    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_table() {
        let html = r#"<html><body>
            <h3>My Section</h3>
            <table summary="Test Table">
                <tr><th>Name</th><th>Value</th></tr>
                <tr><td>Alpha</td><td>100</td></tr>
                <tr><td>Beta</td><td>200</td></tr>
            </table>
        </body></html>"#;
        let tables = extract_tables(html).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "Test Table");
        assert_eq!(tables[0].headers, vec!["Name", "Value"]);
        assert_eq!(tables[0].rows.len(), 2);
    }

    #[test]
    fn test_summary_fallback_to_h3() {
        let html = r#"<html><body>
            <h3 id="Database_Stat">Database Stat</h3>
            <table>
                <tr><th>DB Name</th><th>Backends</th></tr>
                <tr><td>postgres</td><td>11</td></tr>
            </table>
        </body></html>"#;
        let tables = extract_tables(html).unwrap();
        assert_eq!(tables[0].name, "Database Stat");
    }

    #[test]
    fn test_wdr_style_mixed_th_td_rows() {
        let html = r#"<html><body>
            <table summary="Load Profile">
                <tr><th>Metric</th><th>Per Second</th><th>Per Transaction</th></tr>
                <tr><td class="wdrnc">DB Time(us)</td><td>5709</td><td>2045</td></tr>
            </table>
        </body></html>"#;
        let tables = extract_tables(html).unwrap();
        assert_eq!(tables[0].name, "Load Profile");
        assert_eq!(
            tables[0].headers,
            vec!["Metric", "Per Second", "Per Transaction"]
        );
        assert_eq!(tables[0].rows[0][0], "DB Time(us)");
    }

    #[test]
    fn test_no_tables() {
        let html = r#"<html><body><p>No tables here</p></body></html>"#;
        let tables = extract_tables(html).unwrap();
        assert!(tables.is_empty());
    }

    #[test]
    fn test_empty_html() {
        let tables = extract_tables("").unwrap();
        assert!(tables.is_empty());
    }

    #[test]
    fn test_multiple_tables() {
        let html = r#"<html><body>
            <h3>Section A</h3>
            <table summary="Table A"><tr><th>X</th></tr><tr><td>1</td></tr></table>
            <h3>Section B</h3>
            <table summary="Table B"><tr><th>Y</th></tr><tr><td>2</td></tr></table>
        </body></html>"#;
        let tables = extract_tables(html).unwrap();
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0].name, "Table A");
        assert_eq!(tables[1].name, "Table B");
    }

    #[test]
    fn test_extract_table_metadata_lightweight() {
        let html = r#"<html><body>
            <h3>My Section</h3>
            <table summary="Test Table">
                <tr><th>Name</th><th>Value</th></tr>
                <tr><td>Alpha</td><td>100</td></tr>
                <tr><td>Beta</td><td>200</td></tr>
                <tr><td>Gamma</td><td>300</td></tr>
            </table>
        </body></html>"#;
        let metadata = extract_table_metadata(html).unwrap();
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].name, "Test Table");
        assert_eq!(metadata[0].headers, vec!["Name", "Value"]);
        assert_eq!(metadata[0].row_count, 3);
    }
}
