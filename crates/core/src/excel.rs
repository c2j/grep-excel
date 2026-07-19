use crate::format::FileFormat;
use anyhow::Result;
use calamine::{open_workbook_auto, Data, Dimensions, Reader, Sheets};
use chrono::{Datelike, Duration};
use std::collections::BTreeMap;
use std::path::Path;

/// Read a file to String, handling non-UTF-8 encodings.
///
/// Strategy:
/// 1. Try UTF-8 (most common)
/// 2. For HTML files, detect charset from `<meta>` tags
/// 3. Try common CJK encodings (GBK, GB18030, Big5, etc.)
/// 4. Fall back to UTF-8 lossy
pub fn read_file_auto_encoding(path: &Path) -> Result<String> {
    let raw = std::fs::read(path)
        .map_err(|e| anyhow::anyhow!("Failed to read file '{}': {}", path.display(), e))?;

    // 1. Try UTF-8 first (recover bytes on failure to avoid a clone)
    let bytes = match String::from_utf8(raw) {
        Ok(s) => return Ok(s),
        Err(e) => e.into_bytes(),
    };

    // 2. For HTML files, try to detect charset from <meta> tags
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let is_html = ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm");
    if is_html {
        if let Some(encoding_name) = detect_html_charset(&bytes) {
            if let Some(encoding) = encoding_rs::Encoding::for_label(encoding_name.as_bytes()) {
                let (cow, _, had_errors) = encoding.decode(&bytes);
                if !had_errors {
                    return Ok(cow.into_owned());
                }
            }
        }
    }

    // 3. Try common encodings in order of likelihood
    let fallback_labels = [
        "gbk",
        "gb18030",
        "big5",
        "shift_jis",
        "euc-jp",
        "euc-kr",
        "windows-1252",
        "iso-8859-1",
    ];
    for label in &fallback_labels {
        if let Some(encoding) = encoding_rs::Encoding::for_label(label.as_bytes()) {
            let (cow, _, had_errors) = encoding.decode(&bytes);
            if !had_errors {
                return Ok(cow.into_owned());
            }
        }
    }

    // 4. Last resort: lossy UTF-8
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Detect charset from HTML `<meta>` tags in raw bytes.
/// Scans the first 4096 bytes (meta tags are in the head, ASCII-safe).
fn detect_html_charset(bytes: &[u8]) -> Option<String> {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r#"(?i)charset\s*=\s*["']?\s*([a-zA-Z0-9_-]+)"#).unwrap()
    });
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    re.captures(&head)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_lowercase())
}

/// Parse a file with an optional explicit format override.
/// When `format` is None, auto-detects from extension (same as `parse_file`).
pub fn parse_file_as(path: &Path, format: Option<FileFormat>) -> Result<Vec<SheetData>> {
    #[cfg(feature = "archive-support")]
    {
        if format.is_none() {
            if let Some(archive_format) = crate::archive::detect_archive(path) {
                return parse_archive(path, archive_format);
            }
        }
    }

    let fmt = format.unwrap_or_else(|| FileFormat::from_path(path).unwrap_or(FileFormat::Excel));

    match fmt {
        FileFormat::Csv => parse_delimited(path, b','),
        FileFormat::Tsv => parse_delimited(path, b'\t'),
        FileFormat::Html => parse_html(path),
        FileFormat::Text | FileFormat::Markdown => parse_text(path),
        FileFormat::Dbf => parse_dbf(path),
        FileFormat::Xml => parse_xml(path),
        FileFormat::Docx => parse_docx(path),
        FileFormat::Pptx => parse_pptx(path),
        FileFormat::Excel => parse_excel(path),
    }
}

pub fn parse_file(path: &Path) -> Result<Vec<SheetData>> {
    parse_file_as(path, None)
}

fn parse_html(path: &Path) -> Result<Vec<SheetData>> {
    use crate::html_table;

    let content = read_file_auto_encoding(path)?;
    let tables = html_table::extract_tables(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse HTML file '{}': {}", path.display(), e))?;

    Ok(tables
        .into_iter()
        .map(|t| SheetData {
            name: t.name,
            headers: t.headers,
            rows: t.rows,
            col_widths: Vec::new(),
        })
        .collect())
}

fn parse_text(path: &Path) -> Result<Vec<SheetData>> {
    use crate::text_table;

    let content = read_file_auto_encoding(path)?;
    let tables = text_table::extract_tables(path, &content)
        .map_err(|e| anyhow::anyhow!("Failed to parse text file '{}': {}", path.display(), e))?;

    Ok(tables
        .into_iter()
        .map(|t| SheetData {
            name: t.name,
            headers: t.headers,
            rows: t.rows,
            col_widths: Vec::new(),
        })
        .collect())
}

/// Parse a delimiter-separated file (CSV, TSV, etc.)
fn parse_delimited(path: &Path, delimiter: u8) -> Result<Vec<SheetData>> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("data")
        .to_string();

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(delimiter)
        .from_path(path)?;

    let mut all_rows: Vec<Vec<String>> = Vec::new();
    for result in rdr.records() {
        let record = result?;
        all_rows.push(record.iter().map(|s| s.to_string()).collect());
    }

    if all_rows.len() < 2 {
        return Ok(Vec::new());
    }

    let headers = all_rows.remove(0);
    let rows = all_rows;

    Ok(vec![SheetData {
        name,
        headers,
        rows,
        col_widths: Vec::new(),
    }])
}

fn parse_csv(path: &Path) -> Result<Vec<SheetData>> {
    parse_delimited(path, b',')
}

fn parse_tsv(path: &Path) -> Result<Vec<SheetData>> {
    parse_delimited(path, b'\t')
}

/// Parse metadata for a delimiter-separated file (CSV, TSV, etc.)
fn parse_delimited_metadata(path: &Path, delimiter: u8) -> Result<Vec<SheetMetadata>> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("data")
        .to_string();

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(delimiter)
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

fn parse_dbf(path: &Path) -> Result<Vec<SheetData>> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("dbf")
        .to_string();

    let mut reader = dbase::Reader::from_path(path)
        .map_err(|e| anyhow::anyhow!("Failed to open DBF file '{}': {}", path.display(), e))?;

    // Get field definitions in file order before reading records
    // (reader.fields() borrows, so clone before calling read())
    let field_infos: Vec<dbase::FieldInfo> = reader.fields().to_vec();
    let headers: Vec<String> = field_infos.iter().map(|f| f.name().to_string()).collect();

    if headers.is_empty() {
        return Ok(Vec::new());
    }

    let records: Vec<dbase::Record> = reader.read().map_err(|e| {
        anyhow::anyhow!(
            "Failed to read DBF records from '{}': {}",
            path.display(),
            e
        )
    })?;

    // Convert each record to Vec<String> using header order for lookup.
    // Record uses an unordered map internally, so we look up by field name
    // rather than relying on iteration order.
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(records.len());
    for record in &records {
        let mut row: Vec<String> = Vec::with_capacity(headers.len());
        for header in &headers {
            match record.get(header) {
                Some(value) => row.push(dbf_value_to_string(value)),
                None => row.push(String::new()),
            }
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

/// Convert a dBase FieldValue to a display string.
fn dbf_value_to_string(value: &dbase::FieldValue) -> String {
    use dbase::FieldValue;
    match value {
        FieldValue::Character(opt) => opt.clone().unwrap_or_default(),
        FieldValue::Numeric(opt) => opt
            .map(|n| {
                // Show as integer if it's a whole number, else as float
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", n as i64)
                } else {
                    n.to_string()
                }
            })
            .unwrap_or_default(),
        FieldValue::Float(opt) => opt.map(|n| n.to_string()).unwrap_or_default(),
        FieldValue::Integer(i) => i.to_string(),
        FieldValue::Double(f) => f.to_string(),
        FieldValue::Currency(f) => format!("{:.2}", f),
        FieldValue::Logical(opt) => opt
            .map(|b| {
                if b {
                    "TRUE".to_string()
                } else {
                    "FALSE".to_string()
                }
            })
            .unwrap_or_default(),
        FieldValue::Date(opt) => opt
            .map(|d| format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day()))
            .unwrap_or_default(),
        FieldValue::DateTime(dt) => {
            let date = dt.date();
            let time = dt.time();
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                date.year(),
                date.month(),
                date.day(),
                time.hours(),
                time.minutes(),
                time.seconds()
            )
        }
        FieldValue::Memo(s) => s.clone(),
    }
}

fn parse_xml(path: &Path) -> Result<Vec<SheetData>> {
    crate::xml_table::parse_xml_table(path)
}

fn parse_docx(path: &Path) -> Result<Vec<SheetData>> {
    crate::docx_table::parse_docx(path)
}

fn parse_pptx(path: &Path) -> Result<Vec<SheetData>> {
    crate::pptx_table::parse_pptx(path)
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

fn worksheet_merge_cells_auto<RS>(workbook: &mut Sheets<RS>, name: &str) -> Vec<Dimensions>
where
    RS: std::io::Read + std::io::Seek,
{
    match workbook {
        Sheets::Xlsx(xlsx) => match xlsx.worksheet_merge_cells(name) {
            Some(Ok(dims)) => dims,
            _ => Vec::new(),
        },
        Sheets::Xls(xls) => xls.worksheet_merge_cells(name).unwrap_or_default(),
        Sheets::Xlsb(_) | Sheets::Ods(_) => Vec::new(),
    }
}

/// `range_start` is absolute sheet coords; `raw_rows[i]` is relative to that origin.
/// Relative row 0 is the header and is never overwritten.
fn apply_merged_cells_data(
    raw_rows: &mut [Vec<Data>],
    merged: &[Dimensions],
    range_start: (u32, u32),
) {
    if raw_rows.is_empty() || merged.is_empty() {
        return;
    }

    for dim in merged {
        if dim.start.0 < range_start.0 || dim.start.1 < range_start.1 {
            continue;
        }
        let rel_ar = (dim.start.0 - range_start.0) as usize;
        let rel_ac = (dim.start.1 - range_start.1) as usize;
        if rel_ar >= raw_rows.len() {
            continue;
        }
        if rel_ac >= raw_rows[rel_ar].len() {
            raw_rows[rel_ar].resize(rel_ac + 1, Data::Empty);
        }
        let anchor = raw_rows[rel_ar][rel_ac].clone();

        for r in dim.start.0..=dim.end.0 {
            for c in dim.start.1..=dim.end.1 {
                if (r, c) == (dim.start.0, dim.start.1) {
                    continue;
                }
                if r < range_start.0 || c < range_start.1 {
                    continue;
                }
                let rr = (r - range_start.0) as usize;
                let cc = (c - range_start.1) as usize;
                if rr == 0 {
                    continue;
                }
                if rr >= raw_rows.len() {
                    continue;
                }
                if cc >= raw_rows[rr].len() {
                    raw_rows[rr].resize(cc + 1, Data::Empty);
                }
                raw_rows[rr][cc] = anchor.clone();
            }
        }
    }
}

/// Absolute 0-based sheet coords; row 0 (header) is never overwritten.
fn apply_merged_cells_strings(rows: &mut [Vec<String>], merged: &[(u32, u32, u32, u32)]) {
    if rows.is_empty() || merged.is_empty() {
        return;
    }

    for &(sr, sc, er, ec) in merged {
        let ar = sr as usize;
        let ac = sc as usize;
        if ar >= rows.len() {
            continue;
        }
        if ac >= rows[ar].len() {
            rows[ar].resize(ac + 1, String::new());
        }
        let anchor = rows[ar][ac].clone();

        for r in sr..=er {
            for c in sc..=ec {
                if (r, c) == (sr, sc) {
                    continue;
                }
                let rr = r as usize;
                let cc = c as usize;
                if rr == 0 {
                    continue;
                }
                if rr >= rows.len() {
                    continue;
                }
                if cc >= rows[rr].len() {
                    rows[rr].resize(cc + 1, String::new());
                }
                rows[rr][cc] = anchor.clone();
            }
        }
    }
}

fn parse_cell_ref_abs(cell: &str) -> Option<(u32, u32)> {
    let col_letters = extract_col_letters(cell);
    if col_letters.is_empty() || !col_letters.chars().all(|c| c.is_ascii_uppercase()) {
        return None;
    }
    let row_str = &cell[col_letters.len()..];
    if row_str.is_empty() {
        return None;
    }
    let row_1based: u32 = row_str.parse().ok()?;
    if row_1based == 0 {
        return None;
    }
    Some((row_1based - 1, col_letter_to_index(col_letters) as u32))
}

fn parse_merge_cell_ref(ref_str: &str) -> Option<(u32, u32, u32, u32)> {
    let ref_str = ref_str.trim();
    if ref_str.is_empty() {
        return None;
    }
    if let Some((start, end)) = ref_str.split_once(':') {
        let (sr, sc) = parse_cell_ref_abs(start)?;
        let (er, ec) = parse_cell_ref_abs(end)?;
        Some((sr, sc, er, ec))
    } else {
        let (r, c) = parse_cell_ref_abs(ref_str)?;
        Some((r, c, r, c))
    }
}

fn parse_merge_cells_from_xml(doc: &roxmltree::Document) -> Vec<(u32, u32, u32, u32)> {
    doc.descendants()
        .filter(|n| n.has_tag_name("mergeCell"))
        .filter_map(|n| n.attribute("ref").and_then(parse_merge_cell_ref))
        .collect()
}

/// Excel max rows is 1_048_576. Cap densify so a corrupted `r` attribute cannot
/// force multi-million empty-row allocations on the repair path.
const MAX_REPAIR_DENSE_ROWS: usize = 1_048_576;

fn densify_repair_rows(mut row_map: BTreeMap<usize, Vec<String>>) -> Vec<Vec<String>> {
    let Some(&max_row) = row_map.keys().next_back() else {
        return Vec::new();
    };
    if max_row >= MAX_REPAIR_DENSE_ROWS {
        return row_map.into_values().collect();
    }
    let mut dense = Vec::with_capacity(max_row + 1);
    for i in 0..=max_row {
        dense.push(row_map.remove(&i).unwrap_or_default());
    }
    dense
}

// ── Date column detection helpers ────────────────────────────────────────────

/// Keywords that suggest a column contains date values.
const DATE_COLUMN_KEYWORDS: &[&str] = &[
    "date",
    "time",
    "datetime",
    "timestamp",
    "日期",
    "时间",
    "生日",
    "日付",
    "fecha",
    "data",
    "datum",
    "dob",
    "birth",
    "created",
    "updated",
    "modified",
    "begin",
    "start",
    "end",
    "deadline",
];

fn is_date_column_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    DATE_COLUMN_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

/// Convert an Excel serial date number to a "YYYYMMDD" formatted string.
/// Uses the same algorithm as calamine's `ExcelDateTime::as_datetime()`:
/// epoch = 1899-12-30, with the Lotus 1-2-3 leap year bug (day 60 = Feb 29, 1900).
fn excel_serial_to_date_string(serial: f64) -> Option<String> {
    if serial < 1.0 || serial > 100_000.0 {
        return None;
    }
    // Skip values with significant fractional parts (likely non-date numbers)
    let frac = serial - serial.trunc();
    if frac >= 0.001 {
        return None;
    }
    let ms_multiplier: f64 = 24.0 * 60.0 * 60.0 * 1000.0;
    // Excel incorrectly treats 1900 as a leap year.
    // For values >= 60, offset by 1 to compensate.
    let adjusted = if serial >= 60.0 { serial } else { serial + 1.0 };
    let ms = (adjusted * ms_multiplier).round() as i64;
    let epoch = chrono::NaiveDate::from_ymd_opt(1899, 12, 30)?;
    let date = epoch + Duration::milliseconds(ms);
    Some(format!(
        "{:04}{:02}{:02}",
        date.year(),
        date.month(),
        date.day()
    ))
}

/// Like `data_to_string`, but for `Data::DateTime` uses calamine's
/// `as_datetime()` to produce a human-readable date string, and for
/// `Data::Float` tries date conversion.
fn date_aware_to_string(data: &Data) -> String {
    match data {
        Data::DateTime(dt) => dt
            .as_datetime()
            .map(|ndt| format!("{:04}{:02}{:02}", ndt.year(), ndt.month(), ndt.day()))
            .unwrap_or_else(|| dt.to_string()),
        Data::Float(f) => excel_serial_to_date_string(*f).unwrap_or_else(|| f.to_string()),
        other => data_to_string(other),
    }
}

/// Detect which columns contain date values by analyzing raw `Data` rows.
/// Uses two signals:
///   1. Primary: at least one cell in the column is `Data::DateTime`
///      (calamine recognized a date format). Very high confidence.
///   2. Fallback: column name matches date keywords AND the majority of
///      `Data::Float` values in the column look like Excel serial dates.
fn detect_date_columns_from_data(raw_rows: &[Vec<Data>], headers: &[String]) -> Vec<bool> {
    let col_count = raw_rows
        .first()
        .map(|r| r.len())
        .unwrap_or(0)
        .min(headers.len());
    let mut result = vec![false; col_count];

    for col_idx in 0..col_count {
        // Signal 1: calamine already found a DateTime in this column
        let has_calamine_date = raw_rows.iter().any(|row| {
            row.get(col_idx)
                .map_or(false, |d| matches!(d, Data::DateTime(_)))
        });
        if has_calamine_date {
            result[col_idx] = true;
            continue;
        }

        // Signal 2: column name + value heuristic
        let header = headers.get(col_idx).map(|h| h.as_str()).unwrap_or("");
        if !is_date_column_name(header) {
            continue;
        }

        // Count how many Float values in this column look like dates
        let (date_like, total_floats) =
            raw_rows
                .iter()
                .fold((0usize, 0usize), |(dl, tf), row| match row.get(col_idx) {
                    Some(Data::Float(f)) => {
                        let is_date = excel_serial_to_date_string(*f).is_some();
                        (dl + is_date as usize, tf + 1)
                    }
                    _ => (dl, tf),
                });

        // Require at least 2 match candidates and >50% of floats are date-like
        if date_like >= 2 && total_floats > 0 && date_like * 2 > total_floats {
            result[col_idx] = true;
        }
    }

    result
}

/// Post-process rows to convert Excel serial numbers to date strings in
/// columns whose headers match date keywords. Used by the repair path
/// which reads raw XML values (no type information).
fn convert_date_columns_in_place(headers: &[String], rows: &mut [Vec<String>]) {
    for (col_idx, header) in headers.iter().enumerate() {
        if !is_date_column_name(header) {
            continue;
        }
        let date_count = rows
            .iter()
            .filter(|row| {
                row.get(col_idx)
                    .and_then(|v| v.parse::<f64>().ok())
                    .and_then(|f| excel_serial_to_date_string(f))
                    .is_some()
            })
            .count();
        if date_count < 2 || date_count * 2 <= rows.len() {
            continue;
        }
        for row in rows.iter_mut() {
            if let Some(value) = row.get_mut(col_idx) {
                if let Ok(serial) = value.parse::<f64>() {
                    if let Some(date_str) = excel_serial_to_date_string(serial) {
                        *value = date_str;
                    }
                }
            }
        }
    }
}

// ── Excel parsing (calamine) ─────────────────────────────────────────────────

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

        let mut raw_rows: Vec<Vec<Data>> = range
            .rows()
            .map(|row| row.iter().cloned().collect())
            .collect();

        if raw_rows.len() < 2 {
            continue;
        }

        let range_start = range.start().unwrap_or((0, 0));
        let merged = worksheet_merge_cells_auto(&mut workbook, sheet_name);
        apply_merged_cells_data(&mut raw_rows, &merged, range_start);

        let headers_for_detection: Vec<String> = raw_rows[0].iter().map(data_to_string).collect();
        let date_cols = detect_date_columns_from_data(&raw_rows, &headers_for_detection);

        let mut rows: Vec<Vec<String>> = raw_rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .enumerate()
                    .map(|(col_idx, data)| {
                        if date_cols.get(col_idx).copied().unwrap_or(false) {
                            date_aware_to_string(&data)
                        } else {
                            data_to_string(&data)
                        }
                    })
                    .collect()
            })
            .collect();

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
    #[cfg(feature = "archive-support")]
    {
        if let Some(format) = crate::archive::detect_archive(path) {
            return parse_archive_metadata(path, format);
        }
    }

    match FileFormat::from_path(path) {
        Some(FileFormat::Csv) => parse_delimited_metadata(path, b','),
        Some(FileFormat::Tsv) => parse_delimited_metadata(path, b'\t'),
        Some(FileFormat::Html) => parse_html_metadata(path),
        Some(FileFormat::Text) | Some(FileFormat::Markdown) => parse_text_metadata(path),
        Some(FileFormat::Dbf)
        | Some(FileFormat::Xml)
        | Some(FileFormat::Docx)
        | Some(FileFormat::Pptx) => metadata_from_full_parse(path),
        Some(FileFormat::Excel) => parse_excel_metadata(path),
        None => parse_excel_metadata(path),
    }
}

fn metadata_from_full_parse(path: &Path) -> Result<Vec<SheetMetadata>> {
    let sheets = parse_file(path)?;
    Ok(sheets
        .into_iter()
        .map(|s| SheetMetadata {
            name: s.name,
            headers: s.headers,
            row_count: s.rows.len(),
        })
        .collect())
}

fn parse_html_metadata(path: &Path) -> Result<Vec<SheetMetadata>> {
    use crate::html_table;

    let content = read_file_auto_encoding(path)?;
    let tables = html_table::extract_table_metadata(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse HTML file '{}': {}", path.display(), e))?;

    Ok(tables
        .into_iter()
        .map(|t| SheetMetadata {
            name: t.name,
            headers: t.headers,
            row_count: t.row_count,
        })
        .collect())
}

fn parse_text_metadata(path: &Path) -> Result<Vec<SheetMetadata>> {
    use crate::text_table;

    let content = read_file_auto_encoding(path)?;
    let tables = text_table::extract_tables_metadata(path, &content)
        .map_err(|e| anyhow::anyhow!("Failed to parse text file '{}': {}", path.display(), e))?;

    Ok(tables
        .into_iter()
        .map(|t| SheetMetadata {
            name: t.name,
            headers: t.headers,
            row_count: t.row_count,
        })
        .collect())
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
    match FileFormat::from_path(path) {
        Some(FileFormat::Html) => {
            let sheets = parse_html(path)?;
            let mut info = Vec::new();
            for (idx, sheet) in sheets.into_iter().enumerate() {
                let row_count = sheet.rows.len();
                handler(sheet, idx)?;
                info.push((format!("html_{}", idx), row_count));
            }
            Ok(info)
        }
        Some(FileFormat::Text) | Some(FileFormat::Markdown) => {
            let sheets = parse_text(path)?;
            let mut info = Vec::new();
            for (idx, sheet) in sheets.into_iter().enumerate() {
                let row_count = sheet.rows.len();
                handler(sheet, idx)?;
                info.push((format!("text_{}", idx), row_count));
            }
            Ok(info)
        }
        Some(FileFormat::Excel) | None => for_each_excel_sheet(path, handler),
        _ => {
            let sheets = parse_file(path)?;
            let mut info = Vec::new();
            for (idx, sheet) in sheets.into_iter().enumerate() {
                let row_count = sheet.rows.len();
                handler(sheet, idx)?;
                info.push((format!("sheet_{}", idx), row_count));
            }
            Ok(info)
        }
    }
}

/// Calamine-based sheet iteration (Excel/ODS fallback).
fn for_each_excel_sheet<F>(path: &Path, mut handler: F) -> Result<Vec<(String, usize)>>
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

        let mut raw_rows: Vec<Vec<Data>> = range
            .rows()
            .map(|row| row.iter().cloned().collect())
            .collect();

        if raw_rows.len() < 2 {
            continue;
        }

        let range_start = range.start().unwrap_or((0, 0));
        let merged = worksheet_merge_cells_auto(&mut workbook, sheet_name);
        apply_merged_cells_data(&mut raw_rows, &merged, range_start);

        let headers_for_detection: Vec<String> = raw_rows[0].iter().map(data_to_string).collect();
        let date_cols = detect_date_columns_from_data(&raw_rows, &headers_for_detection);

        let mut rows: Vec<Vec<String>> = raw_rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .enumerate()
                    .map(|(col_idx, data)| {
                        if date_cols.get(col_idx).copied().unwrap_or(false) {
                            date_aware_to_string(&data)
                        } else {
                            data_to_string(&data)
                        }
                    })
                    .collect()
            })
            .collect();

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
    match FileFormat::from_path(path) {
        Some(FileFormat::Csv) => parse_csv(path),
        Some(FileFormat::Tsv) => parse_tsv(path),
        Some(FileFormat::Html) => parse_html(path),
        Some(FileFormat::Text) | Some(FileFormat::Markdown) => parse_text(path),
        Some(FileFormat::Docx) => parse_docx(path),
        Some(FileFormat::Pptx) => parse_pptx(path),
        _ => parse_xlsx_repair(path),
    }
}

/// Streaming variant of repair parse: processes one sheet at a time via callback.
pub fn for_each_sheet_repair<F>(path: &Path, mut handler: F) -> Result<Vec<(String, usize)>>
where
    F: FnMut(SheetData, usize) -> Result<()>,
{
    match FileFormat::from_path(path) {
        Some(FileFormat::Csv)
        | Some(FileFormat::Tsv)
        | Some(FileFormat::Dbf)
        | Some(FileFormat::Xml)
        | Some(FileFormat::Docx)
        | Some(FileFormat::Pptx) => {
            let sheets = parse_file(path)?;
            let mut info = Vec::new();
            for (idx, sheet) in sheets.into_iter().enumerate() {
                let row_count = sheet.rows.len();
                handler(sheet, idx)?;
                info.push((format!("sheet_{}", idx), row_count));
            }
            Ok(info)
        }
        Some(FileFormat::Html) => {
            let sheets = parse_html(path)?;
            let mut info = Vec::new();
            for (idx, sheet) in sheets.into_iter().enumerate() {
                let row_count = sheet.rows.len();
                handler(sheet, idx)?;
                info.push((format!("html_{}", idx), row_count));
            }
            Ok(info)
        }
        Some(FileFormat::Text) | Some(FileFormat::Markdown) => {
            let sheets = parse_text(path)?;
            let mut info = Vec::new();
            for (idx, sheet) in sheets.into_iter().enumerate() {
                let row_count = sheet.rows.len();
                handler(sheet, idx)?;
                info.push((format!("text_{}", idx), row_count));
            }
            Ok(info)
        }
        _ => for_each_xlsx_repair(path, handler),
    }
}

fn parse_xlsx_repair(path: &Path) -> Result<Vec<SheetData>> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| anyhow::anyhow!("无法打开xlsx文件进行修复: {}", e))?;

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

#[cfg(feature = "archive-support")]
fn parse_archive(path: &Path, format: crate::archive::ArchiveFormat) -> Result<Vec<SheetData>> {
    use crate::archive::{extract_entry, is_table_entry, list_entries};

    let entries = list_entries(path, format)?;
    let table_entries: Vec<_> = entries
        .iter()
        .filter(|e| e.is_file && is_table_entry(&e.path))
        .collect();

    if table_entries.is_empty() {
        return Err(anyhow::anyhow!(
            "Archive '{}' contains no recognizable table files (supported: {:?})",
            path.display(),
            crate::archive::TABLE_EXTENSIONS
        ));
    }

    let mut all_sheets = Vec::new();
    for entry in &table_entries {
        let tmp_path = extract_entry(path, &entry.path, format)?;
        match parse_file(&tmp_path) {
            Ok(mut sheets) => {
                for sheet in &mut sheets {
                    sheet.name = format!("{}::{}", entry.path, sheet.name);
                }
                all_sheets.extend(sheets);
            }
            Err(e) => {
                eprintln!(
                    "Warning: failed to parse '{}' from archive: {}",
                    entry.path, e
                );
            }
        }
        let _ = std::fs::remove_file(&tmp_path);
    }

    if all_sheets.is_empty() {
        return Err(anyhow::anyhow!(
            "No table files could be parsed from archive '{}'",
            path.display()
        ));
    }

    Ok(all_sheets)
}

#[cfg(feature = "archive-support")]
fn parse_archive_metadata(
    path: &Path,
    format: crate::archive::ArchiveFormat,
) -> Result<Vec<SheetMetadata>> {
    use crate::archive::{extract_entry, is_table_entry, list_entries};

    let entries = list_entries(path, format)?;
    let table_entries: Vec<_> = entries
        .iter()
        .filter(|e| e.is_file && is_table_entry(&e.path))
        .collect();

    if table_entries.is_empty() {
        return Err(anyhow::anyhow!(
            "Archive '{}' contains no recognizable table files",
            path.display()
        ));
    }

    let mut all_metadata = Vec::new();
    for entry in &table_entries {
        let tmp_path = extract_entry(path, &entry.path, format)?;
        match parse_file_metadata(&tmp_path) {
            Ok(mut metas) => {
                for meta in &mut metas {
                    meta.name = format!("{}::{}", entry.path, meta.name);
                }
                all_metadata.extend(metas);
            }
            Err(e) => {
                eprintln!(
                    "Warning: failed to read metadata for '{}' from archive: {}",
                    entry.path, e
                );
            }
        }
        let _ = std::fs::remove_file(&tmp_path);
    }

    Ok(all_metadata)
}

fn for_each_xlsx_repair<F>(path: &Path, mut handler: F) -> Result<Vec<(String, usize)>>
where
    F: FnMut(SheetData, usize) -> Result<()>,
{
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| anyhow::anyhow!("无法打开xlsx文件进行修复: {}", e))?;

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

    let mut row_map: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut next_seq = 0usize;

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
            let abs_row = if let Some(r) = row_node
                .attribute("r")
                .and_then(|s| s.parse::<usize>().ok())
            {
                let idx = r.saturating_sub(1);
                next_seq = next_seq.max(idx + 1);
                idx
            } else {
                let idx = next_seq;
                next_seq += 1;
                idx
            };
            row_map.insert(abs_row, full_row);
        }
    }

    let mut all_rows = densify_repair_rows(row_map);

    if all_rows.len() < 2 {
        return Ok(SheetData {
            name: sheet_name.to_string(),
            headers: Vec::new(),
            rows: Vec::new(),
            col_widths: Vec::new(),
        });
    }

    let merged = parse_merge_cells_from_xml(&doc);
    apply_merged_cells_strings(&mut all_rows, &merged);

    let headers = all_rows.remove(0);
    convert_date_columns_in_place(&headers, &mut all_rows);

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

#[cfg(test)]
mod merged_cell_tests {
    use super::*;

    fn s(v: &str) -> Data {
        Data::String(v.to_string())
    }

    fn empty_grid(rows: usize, cols: usize) -> Vec<Vec<Data>> {
        vec![vec![Data::Empty; cols]; rows]
    }

    #[test]
    fn parse_merge_ref_range_and_single() {
        assert_eq!(parse_merge_cell_ref("A1:B2"), Some((0, 0, 1, 1)));
        assert_eq!(parse_merge_cell_ref("C5"), Some((4, 2, 4, 2)));
        assert_eq!(parse_merge_cell_ref("AA10:AB12"), Some((9, 26, 11, 27)));
        assert_eq!(parse_merge_cell_ref(""), None);
        assert_eq!(parse_merge_cell_ref("1A"), None);
        assert_eq!(parse_merge_cell_ref("A"), None);
    }

    #[test]
    fn vertical_merge_fills_data_rows() {
        let mut grid = empty_grid(5, 2);
        grid[0][0] = s("Region");
        grid[0][1] = s("City");
        grid[1][0] = s("华北");
        grid[1][1] = s("北京");
        grid[2][1] = s("天津");
        grid[3][1] = s("石家庄");
        grid[4][1] = s("唐山");

        let merged = [Dimensions::new((1, 0), (4, 0))];
        apply_merged_cells_data(&mut grid, &merged, (0, 0));

        assert_eq!(grid[1][0], s("华北"));
        assert_eq!(grid[2][0], s("华北"));
        assert_eq!(grid[3][0], s("华北"));
        assert_eq!(grid[4][0], s("华北"));
        assert_eq!(grid[2][1], s("天津"));
    }

    #[test]
    fn horizontal_and_2d_merge() {
        let mut grid = empty_grid(4, 4);
        grid[0][0] = s("H");
        grid[1][1] = s("X");
        apply_merged_cells_data(
            &mut grid,
            &[
                Dimensions::new((1, 1), (1, 3)),
                Dimensions::new((2, 1), (3, 2)),
            ],
            (0, 0),
        );
        assert_eq!(grid[1][2], s("X"));
        assert_eq!(grid[1][3], s("X"));
        grid[2][1] = s("Y");
        apply_merged_cells_data(&mut grid, &[Dimensions::new((2, 1), (3, 2))], (0, 0));
        assert_eq!(grid[2][2], s("Y"));
        assert_eq!(grid[3][1], s("Y"));
        assert_eq!(grid[3][2], s("Y"));
    }

    #[test]
    fn header_row_never_overwritten() {
        let mut grid = empty_grid(3, 3);
        grid[0][0] = s("keep");
        grid[1][0] = s("val");
        apply_merged_cells_data(&mut grid, &[Dimensions::new((0, 0), (0, 2))], (0, 0));
        assert_eq!(grid[0][1], Data::Empty);
        assert_eq!(grid[0][2], Data::Empty);

        apply_merged_cells_data(&mut grid, &[Dimensions::new((0, 0), (2, 0))], (0, 0));
        assert_eq!(grid[0][0], s("keep"));
        assert_eq!(grid[1][0], s("keep"));
        assert_eq!(grid[2][0], s("keep"));
    }

    #[test]
    fn empty_merged_and_oob_safe() {
        let mut grid = empty_grid(2, 2);
        grid[1][0] = s("a");
        let before = grid.clone();
        apply_merged_cells_data(&mut grid, &[], (0, 0));
        assert_eq!(grid, before);
        apply_merged_cells_data(&mut grid, &[Dimensions::new((10, 10), (12, 12))], (0, 0));
        assert_eq!(grid[1][0], s("a"));
    }

    #[test]
    fn range_start_offset() {
        let mut grid = empty_grid(3, 2);
        grid[0][0] = s("H");
        grid[1][0] = s("V");
        apply_merged_cells_data(&mut grid, &[Dimensions::new((6, 3), (7, 3))], (5, 3));
        assert_eq!(grid[1][0], s("V"));
        assert_eq!(grid[2][0], s("V"));
    }

    #[test]
    fn strings_fill_skips_header() {
        let mut rows = vec![
            vec!["H".into(), "C".into()],
            vec!["华北".into(), "北京".into()],
            vec![String::new(), "天津".into()],
            vec![String::new(), "石家庄".into()],
        ];
        apply_merged_cells_strings(&mut rows, &[(1, 0, 3, 0)]);
        assert_eq!(rows[2][0], "华北");
        assert_eq!(rows[3][0], "华北");
        assert_eq!(rows[0][0], "H");
    }

    #[test]
    fn densify_fills_gaps_within_cap() {
        let mut map = BTreeMap::new();
        map.insert(0, vec!["h".into()]);
        map.insert(2, vec!["r2".into()]);
        let rows = densify_repair_rows(map);
        assert_eq!(rows.len(), 3);
        assert!(rows[1].is_empty());
        assert_eq!(rows[2], vec!["r2".to_string()]);
    }

    #[test]
    fn densify_pathological_r_packs_without_allocating_millions() {
        let mut map = BTreeMap::new();
        map.insert(0, vec!["h".into()]);
        map.insert(MAX_REPAIR_DENSE_ROWS, vec!["far".into()]);
        let rows = densify_repair_rows(map);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec!["h".to_string()]);
        assert_eq!(rows[1], vec!["far".to_string()]);
    }
}
