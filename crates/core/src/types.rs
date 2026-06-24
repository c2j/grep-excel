use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub truncated: bool,
    pub duration: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SearchMode {
    FullText,
    ExactMatch,
    Wildcard,
    Regex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    pub column: Option<String>,
    pub mode: SearchMode,
    pub limit: usize,
    pub sheet: Option<String>,
    pub invert: bool,
    /// Number of rows to include before and after each match (grep -C style).
    /// 0 (default when None) means no context rows.
    #[serde(default)]
    pub context_lines: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextRows {
    pub before: Vec<Vec<String>>,
    pub after: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub sheet_name: String,
    pub file_name: String,
    pub row: Vec<String>,
    pub col_names: Vec<String>,
    pub matched_columns: Vec<usize>,
    pub col_widths: Vec<f64>,
    pub row_index: usize,
    #[serde(default)]
    pub context: ContextRows,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchStats {
    pub total_rows_searched: usize,
    pub total_matches: usize,
    pub matches_per_sheet: HashMap<String, usize>,
    pub search_duration: Duration,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateStats {
    pub column: String,
    pub counts: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSample {
    pub sheet_name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub sheets: Vec<(String, usize)>,
    pub total_rows: usize,
    pub sample: Option<FileSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetMetadataInfo {
    pub sheet_name: String,
    pub row_count: usize,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadataInfo {
    pub file_name: String,
    pub sheet_count: usize,
    pub sheets: Vec<SheetMetadataInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetDataResult {
    pub file_name: String,
    pub sheet_name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub total_rows: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[cfg_attr(feature = "mcp-server", schemars(description = "Number of rows to include before and after each match (grep -C style). 0 or omit for no context. MUST be a number, not a string."))]
    pub context_lines: Option<usize>,
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
    #[cfg_attr(feature = "mcp-server", schemars(description = "If provided, returns metadata only for this file; otherwise returns metadata for all imported files"))]
    pub file_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct GetSheetSampleParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the imported file (basename, e.g. \"data.xlsx\")"))]
    pub file_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the sheet within the file"))]
    pub sheet_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Number of evenly-spaced rows to sample (default: 10). Pass as a number, not a string."))]
    pub sample_size: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct GetSheetDataParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the imported file (basename, e.g. \"data.xlsx\")"))]
    pub file_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the sheet within the file"))]
    pub sheet_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "0-based row index to start from (inclusive). Omit or pass 0 to start at the beginning. MUST be a number, not a string."))]
    pub start_row: Option<usize>,
    #[cfg_attr(feature = "mcp-server", schemars(description = "0-based row index to end at (exclusive). Omit to read through the end. MUST be a number, not a string."))]
    pub end_row: Option<usize>,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Optional list of column names to include (other columns are filtered out). Omit to return all columns."))]
    pub columns: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct SaveAsParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the already-imported file whose data should be saved"))]
    pub file_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Absolute or relative path for the output .xlsx file"))]
    pub output_path: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "If provided, saves only this sheet; otherwise saves all sheets"))]
    pub sheet_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct SaveParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the imported file to overwrite (its original on-disk path)"))]
    pub file_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "If provided, saves only this sheet; otherwise saves all sheets"))]
    pub sheet_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct UpdateCellParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the imported file"))]
    pub file_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the sheet within the file"))]
    pub sheet_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "0-based row index (header row is not counted). MUST be a number, not a string."))]
    pub row: usize,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Column name (matches header text)"))]
    pub column: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "New cell value (always passed as a string; numeric formatting is preserved on write)"))]
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct UpdateCellsParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the imported file"))]
    pub file_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the sheet within the file"))]
    pub sheet_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "List of cell updates to apply atomically"))]
    pub updates: Vec<CellUpdate>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct CellUpdate {
    #[cfg_attr(feature = "mcp-server", schemars(description = "0-based row index (header row is not counted). MUST be a number, not a string."))]
    pub row: usize,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Column name (matches header text)"))]
    pub column: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "New cell value (always passed as a string)"))]
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct InsertRowsParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the imported file"))]
    pub file_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the sheet within the file"))]
    pub sheet_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "0-based row index where the first new row should be inserted. Existing rows at and after this index are shifted down. MUST be a number."))]
    pub start_row: usize,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Array of new rows; each row is an array of string cell values in column order"))]
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct DeleteRowsParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the imported file"))]
    pub file_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the sheet within the file"))]
    pub sheet_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "0-based row index of the first row to delete. MUST be a number."))]
    pub start_row: usize,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Number of rows to delete. MUST be a number."))]
    pub count: usize,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct AddColumnParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the imported file"))]
    pub file_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the sheet within the file"))]
    pub sheet_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name for the new column (must not duplicate an existing column name)"))]
    pub column_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Default value written to every existing row in the new column. Omit for empty strings."))]
    pub default_value: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct RenameColumnParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the imported file"))]
    pub file_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the sheet within the file"))]
    pub sheet_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Current column name (must match an existing header)"))]
    pub old_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "New column name (must not duplicate an existing column name)"))]
    pub new_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStatistics {
    pub column_name: String,
    pub total_count: usize,
    pub non_null_count: usize,
    pub null_count: usize,
    pub distinct_count: usize,
    pub top_values: Vec<(String, usize)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetStatistics {
    pub file_name: String,
    pub sheet_name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<ColumnStatistics>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct ExportQueryParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "SQL SELECT query whose result will be exported"))]
    pub sql: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Absolute or relative path for the output .xlsx file"))]
    pub output_path: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Sheet name in the output file (default: \"Sheet1\")"))]
    pub sheet_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct GetSheetStatisticsParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the imported file (basename, e.g. \"data.xlsx\")"))]
    pub file_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Name of the sheet within the file"))]
    pub sheet_name: String,
    #[cfg_attr(feature = "mcp-server", schemars(description = "Max number of top values to return per column (default 5). MUST be a number, not a string."))]
    pub max_top_values: Option<usize>,
}
