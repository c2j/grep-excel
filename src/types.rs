use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SqlResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub truncated: bool,
    pub duration: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchMode {
    FullText,
    ExactMatch,
    Wildcard,
    Regex,
}

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub text: String,
    pub column: Option<String>,
    pub mode: SearchMode,
    pub limit: usize,
    pub sheet: Option<String>,
    pub invert: bool,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub sheet_name: String,
    pub file_name: String,
    pub row: Vec<String>,
    pub col_names: Vec<String>,
    pub matched_columns: Vec<usize>,
    pub col_widths: Vec<f64>,
}

#[derive(Debug, Clone)]
pub struct SearchStats {
    pub total_rows_searched: usize,
    pub total_matches: usize,
    pub matches_per_sheet: HashMap<String, usize>,
    pub search_duration: Duration,
    pub truncated: bool,
}

#[derive(Debug, Clone)]
pub struct AggregateStats {
    pub column: String,
    pub counts: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct FileSample {
    pub sheet_name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub sheets: Vec<(String, usize)>,
    pub total_rows: usize,
    pub sample: Option<FileSample>,
}

#[derive(Debug, Clone)]
pub struct SheetMetadataInfo {
    pub sheet_name: String,
    pub row_count: usize,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FileMetadataInfo {
    pub file_name: String,
    pub sheet_count: usize,
    pub sheets: Vec<SheetMetadataInfo>,
}

#[derive(Debug, Clone)]
pub struct SheetDataResult {
    pub file_name: String,
    pub sheet_name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub total_rows: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone)]
pub struct TableAliasInfo {
    pub table_name: String,
    pub alias: String,
    pub file_name: String,
    pub sheet_name: String,
    pub row_count: usize,
    pub columns: Vec<String>,
}
