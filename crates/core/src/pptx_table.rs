use std::io::Read;
use std::path::Path;

use crate::excel::SheetData;

pub fn parse_pptx(path: &Path) -> anyhow::Result<Vec<SheetData>> {
    let file = std::fs::File::open(path)
        .map_err(|e| anyhow::anyhow!("failed to open pptx '{}': {}", path.display(), e))?;
    let reader = std::io::BufReader::new(file);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| anyhow::anyhow!("invalid pptx ZIP: {}", e))?;

    let mut slides = list_slides(&mut archive);
    slides.sort_by_key(|(n, _)| *n);

    let mut result = Vec::new();
    for (slide_num, entry_name) in &slides {
        let xml = read_entry(&mut archive, entry_name)?;
        let doc = roxmltree::Document::parse(&xml)
            .map_err(|e| anyhow::anyhow!("{} parse failed: {}", entry_name, e))?;
        let mut tables = extract_tables_from_slide(&doc, *slide_num);
        result.append(&mut tables);
    }

    Ok(result)
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

fn list_slides(
    archive: &mut zip::ZipArchive<std::io::BufReader<std::fs::File>>,
) -> Vec<(usize, String)> {
    (0..archive.len())
        .filter_map(|i| {
            let entry = archive.by_index(i).ok()?;
            let name = entry.name();
            let num_str = name
                .strip_prefix("ppt/slides/slide")
                .and_then(|s| s.strip_suffix(".xml"))?;
            if num_str.is_empty() || !num_str.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let n: usize = num_str.parse().ok()?;
            Some((n, name.to_string()))
        })
        .collect()
}

fn extract_tables_from_slide(doc: &roxmltree::Document, slide_num: usize) -> Vec<SheetData> {
    let mut tables = Vec::new();
    let mut table_idx = 0usize;

    for tbl in doc
        .root_element()
        .descendants()
        .filter(|n| n.has_tag_name("tbl"))
    {
        if !is_top_level_pptx_table(tbl) {
            continue;
        }
        table_idx += 1;
        let name = format!("Slide_{}_Table_{}", slide_num, table_idx);
        if let Some(sheet) = parse_pptx_table(tbl, name) {
            tables.push(sheet);
        }
    }
    tables
}

fn is_top_level_pptx_table(tbl: roxmltree::Node) -> bool {
    let mut ancestor = tbl.parent();
    while let Some(a) = ancestor {
        if a.is_element() && a.has_tag_name("tbl") {
            return false;
        }
        ancestor = a.parent();
    }
    true
}

struct PptxCellMeta {
    text: String,
    grid_span: usize,
    row_span: usize,
    h_merge: bool,
    v_merge: bool,
}

fn parse_pptx_table(tbl: roxmltree::Node, name: String) -> Option<SheetData> {
    let mut raw_rows: Vec<Vec<PptxCellMeta>> = Vec::new();

    for tr in tbl
        .children()
        .filter(|n| n.is_element() && n.has_tag_name("tr"))
    {
        let mut row: Vec<PptxCellMeta> = Vec::new();
        for tc in tr
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("tc"))
        {
            let text = extract_pptx_cell_text(tc);
            let grid_span = parse_pptx_attr_usize(tc, "gridSpan").unwrap_or(1).max(1);
            let row_span = parse_pptx_attr_usize(tc, "rowSpan").unwrap_or(1).max(1);
            let h_merge = parse_pptx_attr(tc, "hMerge") == Some("1");
            let v_merge = parse_pptx_attr(tc, "vMerge") == Some("1");
            row.push(PptxCellMeta {
                text,
                grid_span,
                row_span,
                h_merge,
                v_merge,
            });
        }
        raw_rows.push(row);
    }

    if raw_rows.len() < 2 {
        return None;
    }

    apply_pptx_merges(&mut raw_rows);

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
        name,
        headers,
        rows: all_rows,
        col_widths: Vec::new(),
    })
}

fn apply_pptx_merges(rows: &mut [Vec<PptxCellMeta>]) {
    fill_hmerge_from_left(rows);
    fill_vmerge_from_above(rows);
    expand_row_spans(rows);
}

fn fill_hmerge_from_left(rows: &mut [Vec<PptxCellMeta>]) {
    for row in rows.iter_mut() {
        let mut prev_text: Option<String> = None;
        for cell in row.iter_mut() {
            if cell.h_merge {
                if let Some(ref anchor) = prev_text {
                    cell.text = anchor.clone();
                }
            } else {
                prev_text = Some(cell.text.clone());
            }
        }
    }
}

fn fill_vmerge_from_above(rows: &mut [Vec<PptxCellMeta>]) {
    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    for col in 0..col_count {
        let mut anchor_text: Option<String> = None;
        for row in rows.iter_mut() {
            if col >= row.len() {
                continue;
            }
            let cell = &mut row[col];
            if cell.v_merge {
                if let Some(ref anchor) = anchor_text {
                    cell.text = anchor.clone();
                }
            } else {
                anchor_text = Some(cell.text.clone());
            }
        }
    }
}

fn expand_row_spans(rows: &mut [Vec<PptxCellMeta>]) {
    for row_idx in 0..rows.len() {
        let mut col_idx = 0usize;
        while col_idx < rows[row_idx].len() {
            let rs = rows[row_idx][col_idx].row_span;
            if rs > 1 {
                let text = rows[row_idx][col_idx].text.clone();
                let gs = rows[row_idx][col_idx].grid_span;
                for offset in 1..rs {
                    let target = row_idx + offset;
                    if target >= rows.len() {
                        break;
                    }
                    rows[target].insert(
                        col_idx,
                        PptxCellMeta {
                            text: text.clone(),
                            grid_span: gs,
                            row_span: 1,
                            h_merge: false,
                            v_merge: false,
                        },
                    );
                }
            }
            col_idx += 1;
        }
    }
}

fn parse_pptx_attr<'a>(el: roxmltree::Node<'a, 'a>, local_name: &'a str) -> Option<&'a str> {
    el.attributes()
        .find(|a| {
            let n = a.name();
            n == local_name || n.ends_with(&format!(":{}", local_name))
        })
        .map(|a| a.value())
}

fn parse_pptx_attr_usize(el: roxmltree::Node, local_name: &str) -> Option<usize> {
    parse_pptx_attr(el, local_name).and_then(|v| v.parse::<usize>().ok())
}

fn extract_pptx_cell_text(tc: roxmltree::Node) -> String {
    if let Some(tx_body) = tc
        .children()
        .find(|n| n.is_element() && n.has_tag_name("txBody"))
    {
        let mut paragraphs: Vec<String> = Vec::new();
        for p in tx_body
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("p"))
        {
            let mut text = String::new();
            for descendant in p.descendants() {
                if !descendant.is_element() {
                    continue;
                }
                if descendant.has_tag_name("t") {
                    if let Some(t) = descendant.text() {
                        text.push_str(t);
                    }
                } else if descendant.has_tag_name("br") {
                    text.push('\n');
                }
            }
            paragraphs.push(text.trim().to_string());
        }
        paragraphs.join("\n")
    } else {
        String::new()
    }
}
