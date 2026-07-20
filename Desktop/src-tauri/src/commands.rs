use grep_excel_core::engine::{DefaultEngine, SearchEngine};
use grep_excel_core::types::*;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::Path;
use tauri::State;

pub struct AppState {
    engine: Mutex<DefaultEngine>,
}

impl AppState {
    pub fn new() -> Self {
        let engine = DefaultEngine::new().expect("engine init");
        Self {
            engine: Mutex::new(engine),
        }
    }
}

#[tauri::command]
pub fn import_file(path: String, state: State<'_, AppState>) -> Result<FileInfo, String> {
    let p = Path::new(&path);
    let mut engine = state.engine.lock();
    engine
        .import_excel(p, &|_, _| {})
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search(query: SearchQuery, state: State<'_, AppState>) -> Result<SearchResponse, String> {
    let engine = state.engine.lock();
    let (results, stats) = engine.search(&query).map_err(|e| e.to_string())?;

    let mut columns_by_sheet: HashMap<String, SheetColumnMeta> = HashMap::new();
    let mut slim_results = Vec::with_capacity(results.len());

    for r in results {
        columns_by_sheet
            .entry(r.sheet_name.clone())
            .or_insert(SheetColumnMeta {
                col_names: r.col_names,
                col_widths: r.col_widths,
            });
        slim_results.push(SearchResultSlim {
            sheet_name: r.sheet_name,
            file_name: r.file_name,
            row: r.row,
            matched_columns: r.matched_columns,
            row_index: r.row_index,
        });
    }

    Ok(SearchResponse {
        results: slim_results,
        stats,
        columns_by_sheet,
    })
}

#[derive(serde::Serialize)]
pub struct SearchResultSlim {
    sheet_name: String,
    file_name: String,
    row: Vec<String>,
    matched_columns: Vec<usize>,
    row_index: usize,
}

#[derive(serde::Serialize)]
pub struct SheetColumnMeta {
    col_names: Vec<String>,
    col_widths: Vec<f64>,
}

#[derive(serde::Serialize)]
pub struct SearchResponse {
    results: Vec<SearchResultSlim>,
    stats: SearchStats,
    columns_by_sheet: HashMap<String, SheetColumnMeta>,
}

#[tauri::command]
pub fn execute_sql(
    sql: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<SqlResult, String> {
    let engine = state.engine.lock();
    engine
        .execute_sql(&sql, limit.unwrap_or(1000))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_files(state: State<'_, AppState>) -> Vec<FileInfo> {
    let engine = state.engine.lock();
    engine.list_files()
}

#[tauri::command]
pub fn list_table_aliases(state: State<'_, AppState>) -> Vec<TableAliasInfo> {
    let engine = state.engine.lock();
    engine.list_table_aliases()
}

#[tauri::command]
pub fn get_metadata(
    file_name: String,
    state: State<'_, AppState>,
) -> Result<FileMetadataInfo, String> {
    let engine = state.engine.lock();
    engine.get_metadata(&file_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_sheet_sample(
    file_name: String,
    sheet_name: String,
    sample_size: Option<usize>,
    state: State<'_, AppState>,
) -> Result<SheetDataResult, String> {
    let engine = state.engine.lock();
    engine
        .get_sheet_sample(&file_name, &sheet_name, sample_size.unwrap_or(10))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_sheet_data(
    file_name: String,
    sheet_name: String,
    start_row: Option<usize>,
    end_row: Option<usize>,
    columns: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<SheetDataResult, String> {
    let engine = state.engine.lock();
    engine
        .get_sheet_data(
            &file_name,
            &sheet_name,
            start_row,
            end_row,
            columns.as_deref(),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_cell(
    file_name: String,
    sheet_name: String,
    row: usize,
    column: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut engine = state.engine.lock();
    engine
        .update_cell(&file_name, &sheet_name, row, &column, &value)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_data(state: State<'_, AppState>) -> Result<(), String> {
    let mut engine = state.engine.lock();
    engine.clear().map_err(|e| e.to_string())
}
