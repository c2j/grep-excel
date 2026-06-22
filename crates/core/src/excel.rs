use anyhow::Result;
use calamine::{open_workbook_auto, Data, Reader};
use std::path::Path;

pub fn parse_file(path: &Path) -> Result<Vec<SheetData>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "csv" {
        parse_csv(path)
    } else {
        parse_excel(path)
    }
}

fn parse_csv(path: &Path) -> Result<Vec<SheetData>> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("csv")
        .to_string();

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(path)?;

    let mut all_rows: Vec<Vec<String>> = Vec::new();
    for result in rdr.records() {
        let record = result?;
        all_rows.push(record.iter().map(|s| s.to_string()).collect());
    }

    if all_rows.is_empty() {
        return Ok(Vec::new());
    }

    // Extract header row in-place; remaining rows are moved (no full-data clone).
    let headers = all_rows.remove(0);

    if all_rows.is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![SheetData {
        name,
        headers,
        rows: all_rows,
        col_widths: Vec::new(),
    }])
}

#[derive(Debug, Clone)]
pub struct SheetData {
    pub name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub col_widths: Vec<f64>,
}

fn data_to_string(data: &Data) -> String {
    match data {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => dt.to_string(),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("{:?}", e),
    }
}

pub fn parse_excel(path: &Path) -> Result<Vec<SheetData>> {
    let mut workbook = open_workbook_auto(path)?;
    let sheet_names = workbook.sheet_names().to_vec();

    let xlsx_widths = parse_xlsx_col_widths(path);
    let has_xlsx_widths = !xlsx_widths.is_empty();

    let mut sheets_data = Vec::new();

    for (sheet_idx, sheet_name) in sheet_names.iter().enumerate() {
        let range = match workbook.worksheet_range(sheet_name) {
            Ok(range) => range,
            Err(_) => continue,
        };

        let mut rows: Vec<Vec<String>> = range
            .rows()
            .map(|row| row.iter().map(data_to_string).collect())
            .collect();

        if rows.is_empty() {
            continue;
        }

        // Extract header row in-place; remaining rows are moved (no full-data clone).
        let headers = rows.remove(0);

        if rows.is_empty() {
            continue;
        }

        let col_widths = if has_xlsx_widths {
            xlsx_widths.get(sheet_idx).cloned().unwrap_or_default()
        } else {
            Vec::new()
        };

        sheets_data.push(SheetData {
            name: sheet_name.clone(),
            headers,
            rows,
            col_widths,
        });
    }

    Ok(sheets_data)
}

/// Lightweight sheet metadata (no row data loaded).
#[derive(Debug, Clone)]
pub struct SheetMetadata {
    pub name: String,
    pub headers: Vec<String>,
    pub row_count: usize,
}

/// Read sheet metadata (names, headers, row counts) without materializing row data.
/// Used by `--list-tables` for large files where full import is wasteful.
pub fn parse_file_metadata(path: &Path) -> Result<Vec<SheetMetadata>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "csv" {
        parse_csv_metadata(path)
    } else {
        parse_excel_metadata(path)
    }
}

fn parse_csv_metadata(path: &Path) -> Result<Vec<SheetMetadata>> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("csv")
        .to_string();

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(path)?;

    let mut headers: Vec<String> = Vec::new();
    let mut row_count: usize = 0;

    for result in rdr.records() {
        let record = result?;
        if headers.is_empty() {
            headers = record.iter().map(|s| s.to_string()).collect();
        } else {
            row_count += 1;
        }
    }

    if headers.is_empty() || row_count == 0 {
        return Ok(Vec::new());
    }

    Ok(vec![SheetMetadata {
        name,
        headers,
        row_count,
    }])
}

fn parse_excel_metadata(path: &Path) -> Result<Vec<SheetMetadata>> {
    let mut workbook = open_workbook_auto(path)?;
    let sheet_names = workbook.sheet_names().to_vec();

    let mut result = Vec::new();

    for sheet_name in &sheet_names {
        let range = match workbook.worksheet_range(sheet_name) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let (total_rows_u32, _) = range.get_size();
        let total_rows = total_rows_u32 as usize;

        if total_rows < 2 {
            continue;
        }

        let headers: Vec<String> = match range.rows().next() {
            Some(row) => row.iter().map(data_to_string).collect(),
            None => continue,
        };

        if headers.is_empty() {
            continue;
        }

        result.push(SheetMetadata {
            name: sheet_name.clone(),
            headers,
            row_count: total_rows - 1,
        });
    }

    Ok(result)
}

/// Process Excel sheets one at a time via callback, reducing peak memory.
/// The callback receives each SheetData and its index. The SheetData is dropped
/// after the callback returns, before the next sheet is loaded.
pub fn for_each_sheet<F>(path: &Path, mut handler: F) -> Result<Vec<(String, usize)>>
where
    F: FnMut(SheetData, usize) -> Result<()>,
{
    let mut workbook = open_workbook_auto(path)?;
    let sheet_names = workbook.sheet_names().to_vec();
    let xlsx_widths = parse_xlsx_col_widths(path);
    let has_xlsx_widths = !xlsx_widths.is_empty();

    let mut sheet_info = Vec::new();

    for (sheet_idx, sheet_name) in sheet_names.iter().enumerate() {
        let range = match workbook.worksheet_range(sheet_name) {
            Ok(range) => range,
            Err(_) => continue,
        };

        let mut rows: Vec<Vec<String>> = range
            .rows()
            .map(|row| row.iter().map(data_to_string).collect())
            .collect();

        if rows.len() < 2 {
            continue;
        }

        // Extract header row in-place; remaining rows are moved (no full-data clone).
        let headers = rows.remove(0);
        let col_widths = if has_xlsx_widths {
            xlsx_widths.get(sheet_idx).cloned().unwrap_or_default()
        } else {
            Vec::new()
        };

        let row_count = rows.len();
        let sheet_data = SheetData {
            name: sheet_name.clone(),
            headers,
            rows,
            col_widths,
        };

        handler(sheet_data, sheet_idx)?;
        sheet_info.push((sheet_name.clone(), row_count));
    }

    Ok(sheet_info)
}

fn parse_xlsx_col_widths(path: &Path) -> Vec<Vec<f64>> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let mut archive = match zip::ZipArchive::new(std::io::BufReader::new(file)) {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };

    let mut sheet_widths = Vec::new();
    let mut sheet_idx = 1;
    loop {
        let entry_name = format!("xl/worksheets/sheet{}.xml", sheet_idx);
        match archive.by_name(&entry_name) {
            Ok(mut file) => {
                let mut content = String::new();
                if std::io::Read::read_to_string(&mut file, &mut content).is_ok() {
                    sheet_widths.push(extract_col_widths_from_xml(&content));
                }
                sheet_idx += 1;
            }
            Err(_) => break,
        }
    }
    sheet_widths
}

fn extract_col_widths_from_xml(content: &str) -> Vec<f64> {
    const DEFAULT_WIDTH: f64 = 8.43;

    let cols_start = match content.find("<cols>") {
        Some(i) => i,
        None => return Vec::new(),
    };
    let cols_end = match content.find("</cols>") {
        Some(i) => i,
        None => return Vec::new(),
    };
    let section = &content[cols_start..cols_end + 7];

    let mut widths: Vec<f64> = Vec::new();
    let mut pos = 0;
    while pos < section.len() {
        let remaining = &section[pos..];
        let tag_start = match remaining.find("<col ") {
            Some(i) => i,
            None => break,
        };
        let abs_start = pos + tag_start;
        let after_col = &section[abs_start + 5..];
        let tag_end = match after_col.find('/').or_else(|| after_col.find('>')) {
            Some(i) => abs_start + 5 + i + 1,
            None => break,
        };
        let tag = &section[abs_start..tag_end];

        let min: usize = extract_xml_attr(tag, "min")
            .and_then(|s: &str| s.parse::<usize>().ok())
            .unwrap_or(1);
        let max: usize = extract_xml_attr(tag, "max")
            .and_then(|s: &str| s.parse::<usize>().ok())
            .unwrap_or(min);
        let width: f64 = extract_xml_attr(tag, "width")
            .and_then(|s: &str| s.parse::<f64>().ok())
            .unwrap_or(DEFAULT_WIDTH);
        let hidden: usize = extract_xml_attr(tag, "hidden")
            .and_then(|s: &str| s.parse::<usize>().ok())
            .unwrap_or(0);

        if max > widths.len() {
            widths.resize(max, DEFAULT_WIDTH);
        }
        let effective_width = if hidden == 1 { 0.0 } else { width };
        for i in (min - 1)..max {
            widths[i] = effective_width;
        }

        pos = tag_end;
    }

    widths
}

fn extract_xml_attr<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
    let pattern = format!("{}=\"", attr);
    let start = tag.find(&pattern)?;
    let value_start = start + pattern.len();
    let value_end = tag[value_start..].find('"')?;
    Some(&tag[value_start..value_start + value_end])
}

/// Try to parse a damaged xlsx by reading the ZIP and XML directly.
/// Handles cases where calamine fails due to ZIP central directory issues
/// or XML parsing strictness, as long as the underlying ZIP entries are readable.
pub fn parse_file_repair(path: &Path) -> Result<Vec<SheetData>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "csv" {
        return parse_csv(path);
    }

    parse_xlsx_repair(path)
}

/// Streaming variant of repair parse: processes one sheet at a time via callback.
pub fn for_each_sheet_repair<F>(path: &Path, mut handler: F) -> Result<Vec<(String, usize)>>
where
    F: FnMut(SheetData, usize) -> Result<()>,
{
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "csv" {
        let sheets = parse_csv(path)?;
        let mut info = Vec::new();
        for (idx, sheet) in sheets.into_iter().enumerate() {
            let row_count = sheet.rows.len();
            handler(sheet, idx)?;
            info.push((format!("csv_{}", idx), row_count));
        }
        return Ok(info);
    }

    for_each_xlsx_repair(path, handler)
}

fn parse_xlsx_repair(path: &Path) -> Result<Vec<SheetData>> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| anyhow::anyhow!("无法打开xlsx文件进行修复: {}", e))?;

    let shared_strings = read_shared_strings(&mut archive).unwrap_or_default();
    let sheets = read_workbook_sheets(&mut archive)?;
    let xlsx_widths = parse_xlsx_col_widths(path);
    let has_xlsx_widths = !xlsx_widths.is_empty();

    let mut result = Vec::new();
    for (sheet_idx, (name, sheet_path)) in sheets.iter().enumerate() {
        match read_sheet_xml(&mut archive, sheet_path, name, &shared_strings) {
            Ok(sheet_data) => {
                let col_widths = if has_xlsx_widths {
                    xlsx_widths.get(sheet_idx).cloned().unwrap_or_default()
                } else {
                    Vec::new()
                };
                result.push(SheetData {
                    name: sheet_data.name,
                    headers: sheet_data.headers,
                    rows: sheet_data.rows,
                    col_widths,
                });
            }
            Err(e) => {
                eprintln!("  修复警告: 跳过损坏的sheet '{}': {}", name, e);
            }
        }
    }

    if result.is_empty() {
        anyhow::bail!("修复失败: 无法从文件中读取任何有效sheet");
    }

    Ok(result)
}

fn for_each_xlsx_repair<F>(path: &Path, mut handler: F) -> Result<Vec<(String, usize)>>
where
    F: FnMut(SheetData, usize) -> Result<()>,
{
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| anyhow::anyhow!("无法打开xlsx文件进行修复: {}", e))?;

    let shared_strings = read_shared_strings(&mut archive).unwrap_or_default();
    let sheets = read_workbook_sheets(&mut archive)?;
    let xlsx_widths = parse_xlsx_col_widths(path);
    let has_xlsx_widths = !xlsx_widths.is_empty();

    let mut sheet_info = Vec::new();
    for (sheet_idx, (name, sheet_path)) in sheets.iter().enumerate() {
        match read_sheet_xml(&mut archive, sheet_path, name, &shared_strings) {
            Ok(sheet_data) => {
                let row_count = sheet_data.rows.len();
                if sheet_data.headers.is_empty() || row_count == 0 {
                    eprintln!("  修复警告: sheet '{}' 为空, 已跳过", name);
                    continue;
                }
                let col_widths = if has_xlsx_widths {
                    xlsx_widths.get(sheet_idx).cloned().unwrap_or_default()
                } else {
                    Vec::new()
                };
                let data = SheetData {
                    name: sheet_data.name,
                    headers: sheet_data.headers,
                    rows: sheet_data.rows,
                    col_widths,
                };
                handler(data, sheet_idx)?;
                sheet_info.push((name.clone(), row_count));
            }
            Err(e) => {
                eprintln!("  修复警告: 跳过损坏的sheet '{}': {}", name, e);
            }
        }
    }

    if sheet_info.is_empty() {
        anyhow::bail!("修复失败: 无法从文件中读取任何有效sheet");
    }

    Ok(sheet_info)
}

fn read_shared_strings(
    archive: &mut zip::ZipArchive<std::io::BufReader<std::fs::File>>,
) -> Result<Vec<String>> {
    let entry = match archive.by_name("xl/sharedStrings.xml") {
        Ok(e) => e,
        Err(_) => return Ok(Vec::new()),
    };

    let xml = {
        let mut content = String::new();
        let mut reader = std::io::BufReader::new(entry);
        std::io::Read::read_to_string(&mut reader, &mut content)?;
        content
    };

    let doc = roxmltree::Document::parse(&xml)
        .map_err(|e| anyhow::anyhow!("sharedStrings.xml 解析失败: {}", e))?;

    let mut strings = Vec::new();
    for si in doc
        .root_element()
        .descendants()
        .filter(|n| n.has_tag_name("si"))
    {
        // <si><t xml:space="preserve">text</t></si>  or  <si><r><t>text</t></r></si>
        let text: String = si
            .descendants()
            .filter(|n| n.has_tag_name("t"))
            .filter_map(|n| n.text())
            .collect();
        strings.push(text);
    }

    Ok(strings)
}

fn read_workbook_sheets(
    archive: &mut zip::ZipArchive<std::io::BufReader<std::fs::File>>,
) -> Result<Vec<(String, String)>> {
    let wb_entry = archive
        .by_name("xl/workbook.xml")
        .map_err(|e| anyhow::anyhow!("无法读取 xl/workbook.xml: {}", e))?;
    let wb_xml = {
        let mut content = String::new();
        let mut reader = std::io::BufReader::new(wb_entry);
        std::io::Read::read_to_string(&mut reader, &mut content)?;
        content
    };
    let wb_doc = roxmltree::Document::parse(&wb_xml)
        .map_err(|e| anyhow::anyhow!("workbook.xml 解析失败: {}", e))?;

    let mut sheet_refs: Vec<(String, String)> = Vec::new();
    for sheet_node in wb_doc
        .root_element()
        .descendants()
        .filter(|n| n.has_tag_name("sheet"))
    {
        let name = sheet_node
            .attribute("name")
            .unwrap_or("Unknown")
            .to_string();
        let rid = sheet_node
            .attributes()
            .find(|a| a.name().contains("id"))
            .map(|a| a.value().to_string())
            .unwrap_or_default();
        if !rid.is_empty() {
            sheet_refs.push((name, rid));
        }
    }

    let rels_map = read_workbook_rels(archive).unwrap_or_default();

    let mut sheets: Vec<(String, String)> = Vec::new();
    for (name, rid) in sheet_refs {
        let path = rels_map
            .get(&rid)
            .cloned()
            .unwrap_or_else(|| format!("xl/worksheets/sheet{}.xml", sheets.len() + 1));
        let full_path = if path.starts_with("xl/") {
            path
        } else {
            format!("xl/{}", path)
        };
        sheets.push((name, full_path));
    }

    if sheets.is_empty() {
        anyhow::bail!("workbook.xml 中未找到任何sheet定义");
    }

    Ok(sheets)
}

fn read_workbook_rels(
    archive: &mut zip::ZipArchive<std::io::BufReader<std::fs::File>>,
) -> Result<std::collections::HashMap<String, String>> {
    let entry = match archive.by_name("xl/_rels/workbook.xml.rels") {
        Ok(e) => e,
        Err(_) => return Ok(std::collections::HashMap::new()),
    };

    let xml = {
        let mut content = String::new();
        let mut reader = std::io::BufReader::new(entry);
        std::io::Read::read_to_string(&mut reader, &mut content)?;
        content
    };

    let doc = roxmltree::Document::parse(&xml)
        .map_err(|e| anyhow::anyhow!("workbook.xml.rels 解析失败: {}", e))?;

    let mut map = std::collections::HashMap::new();
    for rel in doc
        .root_element()
        .descendants()
        .filter(|n| n.has_tag_name("Relationship"))
    {
        let id = rel.attribute("Id").unwrap_or("").to_string();
        let target = rel.attribute("Target").unwrap_or("").to_string();
        if !id.is_empty() && !target.is_empty() {
            map.insert(id, target);
        }
    }

    Ok(map)
}

fn read_sheet_xml(
    archive: &mut zip::ZipArchive<std::io::BufReader<std::fs::File>>,
    sheet_path: &str,
    sheet_name: &str,
    shared_strings: &[String],
) -> Result<SheetData> {
    let entry = archive
        .by_name(sheet_path)
        .map_err(|e| anyhow::anyhow!("无法读取 {}: {}", sheet_path, e))?;

    let xml = {
        let mut content = String::new();
        let mut reader = std::io::BufReader::new(entry);
        std::io::Read::read_to_string(&mut reader, &mut content)?;
        content
    };

    let doc = roxmltree::Document::parse(&xml)
        .map_err(|e| anyhow::anyhow!("{} 解析失败: {}", sheet_path, e))?;

    let mut all_rows: Vec<Vec<String>> = Vec::new();

    for row_node in doc
        .root_element()
        .descendants()
        .filter(|n| n.has_tag_name("row"))
    {
        let mut cells: Vec<(usize, String)> = Vec::new();

        for cell_node in row_node.children().filter(|n| n.has_tag_name("c")) {
            let r_attr = cell_node.attribute("r").unwrap_or("");
            let col_idx = col_letter_to_index(extract_col_letters(r_attr));
            let t_attr = cell_node.attribute("t").unwrap_or("");

            let value = if t_attr == "s" {
                // Shared string reference
                cell_node
                    .children()
                    .find(|n| n.has_tag_name("v"))
                    .and_then(|v| v.text())
                    .and_then(|idx| idx.parse::<usize>().ok())
                    .and_then(|idx| shared_strings.get(idx).cloned())
                    .unwrap_or_default()
            } else if t_attr == "inlineStr" {
                // Inline string
                cell_node
                    .children()
                    .find(|n| n.has_tag_name("is"))
                    .map(|is| {
                        is.descendants()
                            .filter(|n| n.has_tag_name("t"))
                            .filter_map(|n| n.text())
                            .collect::<String>()
                    })
                    .unwrap_or_default()
            } else if t_attr == "b" {
                // Boolean
                cell_node
                    .children()
                    .find(|n| n.has_tag_name("v"))
                    .and_then(|v| v.text())
                    .map(|s| match s {
                        "1" | "true" => "TRUE".to_string(),
                        _ => "FALSE".to_string(),
                    })
                    .unwrap_or_default()
            } else {
                // Number, date, or plain string
                cell_node
                    .children()
                    .find(|n| n.has_tag_name("v"))
                    .and_then(|v| v.text())
                    .unwrap_or("")
                    .to_string()
            };

            cells.push((col_idx, value));
        }

        if !cells.is_empty() {
            let max_col = cells.iter().map(|(i, _)| *i).max().unwrap_or(0);
            let mut full_row = vec![String::new(); max_col + 1];
            for (col_idx, value) in cells {
                full_row[col_idx] = value;
            }
            all_rows.push(full_row);
        }
    }

    if all_rows.len() < 2 {
        return Ok(SheetData {
            name: sheet_name.to_string(),
            headers: Vec::new(),
            rows: Vec::new(),
            col_widths: Vec::new(),
        });
    }

    // Extract header row in-place; remaining rows are moved (no full-data clone).
    let headers = all_rows.remove(0);

    Ok(SheetData {
        name: sheet_name.to_string(),
        headers,
        rows: all_rows,
        col_widths: Vec::new(),
    })
}

/// Convert Excel column letters to 0-based index: A=0, B=1, ..., Z=25, AA=26, ...
fn col_letter_to_index(col: &str) -> usize {
    let mut result = 0usize;
    for c in col.chars() {
        if c.is_ascii_uppercase() {
            result = result * 26 + (c as usize - 'A' as usize + 1);
        }
    }
    result.saturating_sub(1)
}

/// Extract column letters from a cell reference like "A1" -> "A", "AB42" -> "AB"
fn extract_col_letters(cell_ref: &str) -> &str {
    let end = cell_ref
        .find(|c: char| c.is_ascii_digit())
        .unwrap_or(cell_ref.len());
    &cell_ref[..end]
}
