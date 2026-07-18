use std::io::Read;
use std::path::Path;

use crate::excel::SheetData;

pub fn parse_docx(path: &Path) -> anyhow::Result<Vec<SheetData>> {
    let file = std::fs::File::open(path)
        .map_err(|e| anyhow::anyhow!("failed to open docx '{}': {}", path.display(), e))?;
    let reader = std::io::BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| anyhow::anyhow!("invalid docx ZIP: {}", e))?;

    let xml = read_entry(&mut archive, "word/document.xml")?;
    let doc = roxmltree::Document::parse(&xml)
        .map_err(|e| anyhow::anyhow!("word/document.xml parse failed: {}", e))?;

    Ok(extract_tables(&doc))
}

fn read_entry<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> anyhow::Result<String> {
    let mut entry = archive
        .by_name(name)
        .map_err(|e| anyhow::anyhow!("missing '{}' in archive: {}", name, e))?;
    let mut content = String::new();
    let mut reader = std::io::BufReader::new(&mut entry);
    reader.read_to_string(&mut content)?;
    Ok(content)
}

fn extract_tables(doc: &roxmltree::Document) -> Vec<SheetData> {
    let root = doc.root_element();
    let mut tables = Vec::new();
    let mut table_idx = 0usize;

    for tbl in root.descendants().filter(|n| n.has_tag_name("tbl")) {
        if !is_top_level_table(tbl) {
            continue;
        }
        table_idx += 1;
        if let Some(sheet) = parse_table(tbl, table_idx) {
            tables.push(sheet);
        }
    }
    tables
}

fn is_top_level_table(tbl: roxmltree::Node) -> bool {
    let mut ancestor = tbl.parent();
    while let Some(a) = ancestor {
        if a.is_element() && a.has_tag_name("tbl") {
            return false;
        }
        ancestor = a.parent();
    }
    true
}

fn parse_table(tbl: roxmltree::Node, idx: usize) -> Option<SheetData> {
    let mut rows: Vec<Vec<String>> = Vec::new();

    for tr in tbl.children().filter(|n| n.is_element() && n.has_tag_name("tr")) {
        let row: Vec<String> = tr
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("tc"))
            .map(extract_cell_text)
            .collect();
        rows.push(row);
    }

    if rows.len() < 2 {
        return None;
    }

    let headers = rows.remove(0);

    Some(SheetData {
        name: format!("Table_{}", idx),
        headers,
        rows,
        col_widths: Vec::new(),
    })
}

fn extract_cell_text(tc: roxmltree::Node) -> String {
    let mut paragraphs: Vec<String> = Vec::new();
    for p in tc.children().filter(|n| n.is_element() && n.has_tag_name("p")) {
        let mut text = String::new();
        for descendant in p.descendants() {
            if !descendant.is_element() {
                continue;
            }
            if descendant.has_tag_name("t") {
                if let Some(t) = descendant.text() {
                    text.push_str(t);
                }
            } else if descendant.has_tag_name("tab") {
                text.push('\t');
            } else if descendant.has_tag_name("br") {
                text.push('\n');
            }
        }
        paragraphs.push(text.trim().to_string());
    }
    paragraphs.join("\n")
}
