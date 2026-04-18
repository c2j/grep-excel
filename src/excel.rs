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

    let headers = all_rows[0].clone();
    let rows = all_rows[1..].to_vec();

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

        let rows: Vec<Vec<String>> = range
            .rows()
            .map(|row| row.iter().map(data_to_string).collect())
            .collect();

        if rows.is_empty() {
            continue;
        }

        let headers = rows[0].clone();
        let data_rows: Vec<Vec<String>> = rows[1..].to_vec();

        if data_rows.is_empty() {
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
            rows: data_rows,
            col_widths,
        });
    }

    Ok(sheets_data)
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

        let rows: Vec<Vec<String>> = range
            .rows()
            .map(|row| row.iter().map(data_to_string).collect())
            .collect();

        if rows.len() < 2 {
            continue;
        }

        let headers = rows[0].clone();
        let data_rows: Vec<Vec<String>> = rows[1..].to_vec();
        let col_widths = if has_xlsx_widths {
            xlsx_widths.get(sheet_idx).cloned().unwrap_or_default()
        } else {
            Vec::new()
        };

        let row_count = data_rows.len();
        let sheet_data = SheetData {
            name: sheet_name.clone(),
            headers,
            rows: data_rows,
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
