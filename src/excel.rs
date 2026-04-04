use anyhow::Result;
use calamine::{open_workbook_auto, Data, Reader};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SheetData {
    pub name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
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

    let mut sheets_data = Vec::new();

    for sheet_name in &sheet_names {
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

        sheets_data.push(SheetData {
            name: sheet_name.clone(),
            headers,
            rows: data_rows,
        });
    }

    Ok(sheets_data)
}
