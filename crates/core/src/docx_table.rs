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

struct CellMeta {
    text: String,
    grid_span: usize,
    v_merge_restart: bool,
    v_merge_continue: bool,
}

fn parse_table(tbl: roxmltree::Node, idx: usize) -> Option<SheetData> {
    let mut raw_rows: Vec<Vec<CellMeta>> = Vec::new();

    for tr in tbl.children().filter(|n| n.is_element() && n.has_tag_name("tr")) {
        let mut row: Vec<CellMeta> = Vec::new();
        for tc in tr.children().filter(|n| n.is_element() && n.has_tag_name("tc")) {
            let text = extract_cell_text(tc);
            let (grid_span, v_merge_restart, v_merge_continue) = parse_tc_pr(tc);
            row.push(CellMeta {
                text,
                grid_span,
                v_merge_restart,
                v_merge_continue,
            });
        }
        raw_rows.push(row);
    }

    if raw_rows.len() < 2 {
        return None;
    }

    apply_vmerge(&mut raw_rows);

    let mut all_rows: Vec<Vec<String>> = Vec::new();
    for row_meta in &raw_rows {
        let mut row = Vec::new();
        for cell in row_meta {
            for _ in 0..cell.grid_span {
                row.push(cell.text.clone());
            }
        }
        all_rows.push(row);
    }

    let headers = all_rows.remove(0);

    Some(SheetData {
        name: format!("Table_{}", idx),
        headers,
        rows: all_rows,
        col_widths: Vec::new(),
    })
}

fn parse_tc_pr(tc: roxmltree::Node) -> (usize, bool, bool) {
    let mut grid_span = 1usize;
    let mut v_merge_restart = false;
    let mut v_merge_continue = false;

    if let Some(tc_pr) = tc
        .children()
        .find(|n| n.is_element() && n.has_tag_name("tcPr"))
    {
        for child in tc_pr.children().filter(|n| n.is_element()) {
            if child.has_tag_name("gridSpan") {
                if let Some(v) = find_attr(child, "val").and_then(|v| v.parse::<usize>().ok()) {
                    grid_span = v.max(1);
                }
            } else if child.has_tag_name("vMerge") {
                match find_attr(child, "val") {
                    Some("restart") => v_merge_restart = true,
                    None => v_merge_continue = true,
                    _ => {}
                }
            }
        }
    }

    (grid_span, v_merge_restart, v_merge_continue)
}

fn apply_vmerge(rows: &mut [Vec<CellMeta>]) {
    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    for col in 0..col_count {
        let mut anchor_text: Option<String> = None;
        for row in rows.iter_mut() {
            if col >= row.len() {
                continue;
            }
            let cell = &mut row[col];
            if cell.v_merge_restart {
                anchor_text = Some(cell.text.clone());
            } else if cell.v_merge_continue {
                if let Some(ref anchor) = anchor_text {
                    cell.text = anchor.clone();
                }
            } else {
                anchor_text = None;
            }
        }
    }
}

fn find_attr<'a>(el: roxmltree::Node<'a, 'a>, local_name: &str) -> Option<&'a str> {
    el.attributes()
        .find(|a| {
            let n = a.name();
            n == local_name || n.ends_with(&format!(":{}", local_name))
        })
        .map(|a| a.value())
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
