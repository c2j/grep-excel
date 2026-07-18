use std::path::Path;
use crate::excel::SheetData;

pub fn parse_xml_table(path: &Path) -> anyhow::Result<Vec<SheetData>> {
    let content = crate::excel::read_file_auto_encoding(path)?;
    let doc = roxmltree::Document::parse(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse XML '{}': {}", path.display(), e))?;

    let root = doc.root_element();
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("xml")
        .to_string();

    // Find first element child of root
    let mut children = root.children().filter(|n| n.is_element());
    let first_child = match children.next() {
        Some(c) => c,
        None => return Ok(Vec::new()),
    };

    // Gather siblings with same tag name as first child
    let row_tag = first_child.tag_name().name();
    let row_elements: Vec<roxmltree::Node> = std::iter::once(first_child)
        .chain(children)
        .filter(|n| n.is_element() && n.tag_name().name() == row_tag)
        .collect();

    if row_elements.is_empty() {
        return Ok(Vec::new());
    }

    // Headers = child element names from first row
    let headers: Vec<String> = row_elements[0]
        .children()
        .filter(|n| n.is_element())
        .map(|n| n.tag_name().name().to_string())
        .collect();

    if headers.is_empty() {
        // Fallback: row elements have text content, not child elements
        let rows: Vec<Vec<String>> = row_elements
            .iter()
            .map(|el| vec![el.text().unwrap_or("").trim().to_string()])
            .filter(|r| !r[0].is_empty())
            .collect();
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        return Ok(vec![SheetData {
            name,
            headers: vec!["value".to_string()],
            rows,
            col_widths: Vec::new(),
        }]);
    }

    // Data rows
    let mut rows: Vec<Vec<String>> = Vec::new();
    for row_el in &row_elements {
        let mut row: Vec<String> = Vec::with_capacity(headers.len());
        for header in &headers {
            let value = row_el
                .children()
                .find(|n| n.is_element() && n.tag_name().name() == header.as_str())
                .and_then(|n| n.text())
                .map(|t| t.trim().to_string())
                .unwrap_or_default();
            row.push(value);
        }
        if !row.iter().all(|c| c.is_empty()) {
            rows.push(row);
        }
    }

    if rows.is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![SheetData {
        name,
        headers,
        rows,
        col_widths: Vec::new(),
    }])
}
