use std::collections::HashMap;
use std::time::Duration;
use serde::Deserialize;

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

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct ImportFileParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Absolute or relative path to the Excel/CSV file"))]
    pub file_path: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct SearchParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Search query string"))]
    pub query: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Filter to a specific column name"))]
    pub column: Option<String>,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Filter to a specific sheet name"))]
    pub sheet: Option<String>,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Search mode: fulltext, exact, wildcard, regex"))]
    pub mode: Option<String>,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Maximum results to return (default: 100)"))]
    pub limit: Option<usize>,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Aggregate column"))]
    pub aggregate: Option<String>,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Invert match"))]
    pub invert: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct SqlQueryParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "SQL SELECT query"))]
    pub sql: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Max results"))]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct GetMetadataParams {
    pub file_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct GetSheetSampleParams {
    pub file_name: String,
    pub sheet_name: String,
    pub sample_size: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct GetSheetDataParams {
    pub file_name: String,
    pub sheet_name: String,
    pub start_row: Option<usize>,
    pub end_row: Option<usize>,
    pub columns: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct SaveAsParams {
    pub file_name: String,
    pub output_path: String,
    pub sheet_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct SaveParams {
    pub file_name: String,
    pub sheet_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct UpdateCellParams {
    pub file_name: String,
    pub sheet_name: String,
    pub row: usize,
    pub column: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct UpdateCellsParams {
    pub file_name: String,
    pub sheet_name: String,
    pub updates: Vec<CellUpdate>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct CellUpdate {
    pub row: usize,
    pub column: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct InsertRowsParams {
    pub file_name: String,
    pub sheet_name: String,
    pub start_row: usize,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct DeleteRowsParams {
    pub file_name: String,
    pub sheet_name: String,
    pub start_row: usize,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct AddColumnParams {
    pub file_name: String,
    pub sheet_name: String,
    pub column_name: String,
    pub default_value: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct RenameColumnParams {
    pub file_name: String,
    pub sheet_name: String,
    pub old_name: String,
    pub new_name: String,
}
