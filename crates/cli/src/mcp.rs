use std::sync::Arc;

use parking_lot::RwLock;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::tool;
use rmcp::tool_router;
use rmcp::ServiceExt;
use serde::Serialize;

use crate::engine::{DefaultEngine, SearchEngine, SearchMode, SearchQuery};
use crate::types::*;

pub(crate) struct SyncDb(pub(crate) DefaultEngine);
unsafe impl Sync for SyncDb {}
unsafe impl Send for SyncDb {}

#[derive(Debug, Serialize)]
pub struct McpSheetInfo {
    pub name: String,
    pub row_count: usize,
}

#[derive(Debug, Serialize)]
pub struct McpFileInfo {
    pub name: String,
    pub sheets: Vec<McpSheetInfo>,
    pub total_rows: usize,
}

#[derive(Debug, Serialize)]
pub struct McpSearchResult {
    pub file_name: String,
    pub sheet_name: String,
    pub row: Vec<String>,
    pub col_names: Vec<String>,
    pub matched_column_names: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct McpSearchStats {
    pub total_rows_searched: usize,
    pub total_matches: usize,
    pub search_duration_ms: u64,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct McpAggregateStats {
    pub column: String,
    pub counts: Vec<McpAggregateCount>,
}

#[derive(Debug, Serialize)]
pub struct McpAggregateCount {
    pub value: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct McpSearchResponse {
    pub results: Vec<McpSearchResult>,
    pub stats: McpSearchStats,
    pub aggregate: Option<McpAggregateStats>,
}

#[derive(Debug, Serialize)]
pub struct McpFileListResponse {
    pub files: Vec<McpFileInfo>,
}

#[derive(Debug, Serialize)]
pub struct McpSqlResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub truncated: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct McpSheetMetadata {
    pub sheet_name: String,
    pub row_count: usize,
    pub columns: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct McpFileMetadata {
    pub file_name: String,
    pub sheet_count: usize,
    pub sheets: Vec<McpSheetMetadata>,
}

#[derive(Debug, Serialize)]
pub struct McpMetadataResponse {
    pub files: Vec<McpFileMetadata>,
}

#[derive(Debug, Serialize)]
pub struct McpSheetData {
    pub file_name: String,
    pub sheet_name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub total_rows: usize,
    pub truncated: bool,
}

impl From<crate::types::SqlResult> for McpSqlResult {
    fn from(r: crate::types::SqlResult) -> Self {
        McpSqlResult {
            columns: r.columns,
            rows: r.rows,
            row_count: r.row_count,
            truncated: r.truncated,
            duration_ms: r.duration.as_millis() as u64,
        }
    }
}

impl From<FileInfo> for McpFileInfo {
    fn from(info: FileInfo) -> Self {
        McpFileInfo {
            name: info.name,
            sheets: info
                .sheets
                .into_iter()
                .map(|(name, row_count)| McpSheetInfo { name, row_count })
                .collect(),
            total_rows: info.total_rows,
        }
    }
}

impl From<SearchResult> for McpSearchResult {
    fn from(r: SearchResult) -> Self {
        let matched_column_names: Vec<String> = r
            .matched_columns
            .iter()
            .filter_map(|&idx| r.col_names.get(idx).cloned())
            .collect();
        McpSearchResult {
            file_name: r.file_name,
            sheet_name: r.sheet_name,
            row: r.row,
            col_names: r.col_names,
            matched_column_names,
        }
    }
}

impl From<SearchStats> for McpSearchStats {
    fn from(s: SearchStats) -> Self {
        McpSearchStats {
            total_rows_searched: s.total_rows_searched,
            total_matches: s.total_matches,
            search_duration_ms: s.search_duration.as_millis() as u64,
            truncated: s.truncated,
        }
    }
}

impl From<FileMetadataInfo> for McpFileMetadata {
    fn from(m: FileMetadataInfo) -> Self {
        McpFileMetadata {
            file_name: m.file_name,
            sheet_count: m.sheet_count,
            sheets: m.sheets.into_iter().map(|s| McpSheetMetadata {
                sheet_name: s.sheet_name,
                row_count: s.row_count,
                columns: s.columns,
            }).collect(),
        }
    }
}

impl From<SheetDataResult> for McpSheetData {
    fn from(r: SheetDataResult) -> Self {
        McpSheetData {
            file_name: r.file_name,
            sheet_name: r.sheet_name,
            columns: r.columns,
            rows: r.rows,
            row_count: r.row_count,
            total_rows: r.total_rows,
            truncated: r.truncated,
        }
    }
}

fn parse_search_mode(mode: &str) -> SearchMode {
    match mode {
        "exact" => SearchMode::ExactMatch,
        "wildcard" => SearchMode::Wildcard,
        "regex" => SearchMode::Regex,
        _ => SearchMode::FullText,
    }
}

#[derive(Clone)]
pub struct GrepExcelServer {
    db: Arc<RwLock<SyncDb>>,
    import_paths: Arc<RwLock<std::collections::HashMap<String, String>>>,
}

#[tool_router(server_handler)]
impl GrepExcelServer {
    #[tool(description = "Import an Excel or CSV file for searching. Supports xlsx, xls, xlsm, xlsb, ods, and csv formats.")]
    pub async fn import_file(
        &self,
        Parameters(params): Parameters<ImportFileParams>,
    ) -> Result<String, String> {
        let path = std::path::PathBuf::from(&params.file_path);
        let file_path = params.file_path.clone();
        let db = Arc::clone(&self.db);
        let import_paths = Arc::clone(&self.import_paths);
        let canonical = std::fs::canonicalize(&path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(file_path.clone());
        tokio::task::spawn_blocking(move || {
            let mut guard = db.write();
            guard
                .0
                .import_excel(&path, &|_, _| {})
                .map(|info| {
                    import_paths.write().insert(info.name.clone(), canonical);
                    let mcp_info: McpFileInfo = info.into();
                    serde_json::to_string_pretty(&mcp_info)
                        .unwrap_or_else(|_| "Import successful".to_string())
                })
                .map_err(|e| format!("Failed to import '{}': {}", file_path, e))
        })
        .await
        .map_err(|e| format!("Task error: {}", e))?
    }

    #[tool(description = "Search across all imported Excel/CSV files. Supports fulltext, exact, wildcard, and regex modes.")]
    pub async fn search(
        &self,
        Parameters(params): Parameters<SearchParams>,
    ) -> Result<String, String> {
        let mode = params
            .mode
            .as_deref()
            .map(parse_search_mode)
            .unwrap_or(SearchMode::FullText);
        let query = SearchQuery {
            text: params.query,
            column: params.column,
            mode,
            limit: params.limit.unwrap_or(100),
            sheet: params.sheet,
            invert: params.invert.unwrap_or(false),
        };
        let aggregate_col = params.aggregate;
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let guard = db.read();
            guard
                .0
                .search(&query)
                .map(|(results, stats)| {
                    let aggregate = aggregate_col.and_then(|col| {
                        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
                        for result in &results {
                            if let Some(col_idx) = result.col_names.iter().position(|c| c == &col) {
                                if let Some(value) = result.row.get(col_idx) {
                                    if !value.is_empty() {
                                        *counts.entry(value.clone()).or_insert(0) += 1;
                                    }
                                }
                            }
                        }
                        if counts.is_empty() {
                            None
                        } else {
                            let mut sorted: Vec<_> = counts.into_iter().collect();
                            sorted.sort_by(|a, b| b.1.cmp(&a.1));
                            Some(McpAggregateStats {
                                column: col,
                                counts: sorted.into_iter().map(|(value, count)| McpAggregateCount { value, count }).collect(),
                            })
                        }
                    });
                    let response = McpSearchResponse {
                        results: results.into_iter().map(Into::into).collect(),
                        stats: stats.into(),
                        aggregate,
                    };
                    serde_json::to_string_pretty(&response)
                        .unwrap_or_else(|_| "Search complete".to_string())
                })
                .map_err(|e| format!("Search failed: {}", e))
        })
        .await
        .map_err(|e| format!("Task error: {}", e))?
    }

    #[tool(description = "List all imported files and their sheet information.")]
    pub async fn list_files(&self) -> String {
        let db = Arc::clone(&self.db);
        let result = tokio::task::spawn_blocking(move || {
            let guard = db.read();
            let files: Vec<McpFileInfo> = guard
                .0
                .list_files()
                .into_iter()
                .map(Into::into)
                .collect();
            McpFileListResponse { files }
        })
        .await;
        match result {
            Ok(resp) => serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "{}".to_string()),
            Err(e) => format!("{{\"error\": \"{}\"}}", e),
        }
    }

    #[tool(description = "Execute a SQL SELECT query against imported Excel/CSV data. Only SELECT statements are allowed. Table names follow pattern: sheet_{file_id}_{sheet_idx}. Use list_files to discover tables and their schemas. Supports standard SQL plus engine-specific functions (DuckDB: ILIKE, :: casts, window functions; SQLite: LIKE, regexp()).")]
    pub async fn execute_sql(
        &self,
        Parameters(params): Parameters<SqlQueryParams>,
    ) -> Result<String, String> {
        let sql = params.sql;
        let limit = params.limit.unwrap_or(1000);
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let guard = db.read();
            guard
                .0
                .execute_sql(&sql, limit)
                .map(|result| {
                    let output = McpSqlResult::from(result);
                    serde_json::to_string_pretty(&output)
                        .unwrap_or_else(|_| "SQL query complete".to_string())
                })
                .map_err(|e| format!("SQL error: {}", e))
        })
        .await
        .map_err(|e| format!("Task error: {}", e))?
    }

    #[tool(description = "Get detailed metadata for imported files, including sheet names and column names. If file_name is omitted, returns metadata for all imported files.")]
    pub async fn get_metadata(
        &self,
        Parameters(params): Parameters<GetMetadataParams>,
    ) -> Result<String, String> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let guard = db.read();
            if let Some(file_name) = params.file_name {
                guard.0.get_metadata(&file_name)
                    .map(|m| {
                        let mcp: McpFileMetadata = m.into();
                        serde_json::to_string_pretty(&McpMetadataResponse {
                            files: vec![mcp],
                        }).unwrap_or_else(|_| "Metadata retrieved".to_string())
                    })
                    .map_err(|e| format!("Failed to get metadata: {}", e))
            } else {
                let files = guard.0.list_files();
                let mut all_metadata = Vec::new();
                for file in files {
                    match guard.0.get_metadata(&file.name) {
                        Ok(m) => all_metadata.push(m.into()),
                        Err(_) => continue,
                    }
                }
                Ok(serde_json::to_string_pretty(&McpMetadataResponse {
                    files: all_metadata,
                }).unwrap_or_else(|_| "Metadata retrieved".to_string()))
            }
        })
        .await
        .map_err(|e| format!("Task error: {}", e))?
    }

    #[tool(description = "Get a sample of rows from a specific sheet. Uses deterministic evenly-spaced sampling.")]
    pub async fn get_sheet_sample(
        &self,
        Parameters(params): Parameters<GetSheetSampleParams>,
    ) -> Result<String, String> {
        let sample_size = params.sample_size.unwrap_or(10);
        let file_name = params.file_name;
        let sheet_name = params.sheet_name;
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let guard = db.read();
            guard.0.get_sheet_sample(&file_name, &sheet_name, sample_size)
                .map(|r| {
                    let mcp: McpSheetData = r.into();
                    serde_json::to_string_pretty(&mcp)
                        .unwrap_or_else(|_| "Sample retrieved".to_string())
                })
                .map_err(|e| format!("Failed to get sample: {}", e))
        })
        .await
        .map_err(|e| format!("Task error: {}", e))?
    }

    #[tool(description = "Get rows from a specific sheet with pagination and column filtering. Supports start_row/end_row for pagination and optional column selection.")]
    pub async fn get_sheet_data(
        &self,
        Parameters(params): Parameters<GetSheetDataParams>,
    ) -> Result<String, String> {
        let db = Arc::clone(&self.db);
        let file_name = params.file_name;
        let sheet_name = params.sheet_name;
        let start_row = params.start_row;
        let end_row = params.end_row;
        let columns = params.columns;
        tokio::task::spawn_blocking(move || {
            let guard = db.read();
            guard.0.get_sheet_data(&file_name, &sheet_name, start_row, end_row, columns.as_deref())
                .map(|r| {
                    let mcp: McpSheetData = r.into();
                    serde_json::to_string_pretty(&mcp)
                        .unwrap_or_else(|_| "Data retrieved".to_string())
                })
                .map_err(|e| format!("Failed to get sheet data: {}", e))
        })
        .await
        .map_err(|e| format!("Task error: {}", e))?
    }

    #[tool(description = "Save imported data to a new Excel file (Save As). Does not modify the original file.")]
    pub async fn save_as(
        &self,
        Parameters(params): Parameters<SaveAsParams>,
    ) -> Result<String, String> {
        let db = Arc::clone(&self.db);
        let file_name = params.file_name;
        let output_path = params.output_path;
        let sheet_name = params.sheet_name;
        let result: Result<String, String> = tokio::task::spawn_blocking(move || {
            let guard = db.read();
            if let Some(ref sheet_name) = sheet_name {
                let data = guard.0.get_sheet_data(&file_name, sheet_name, None, None, None)
                    .map_err(|e| format!("Failed to read sheet data: {}", e))?;
                use crate::engine::write_xlsx;
                let headers = &data.columns;
                let rows = &data.rows;
                write_xlsx(&[(sheet_name.as_str(), headers.as_slice(), rows.as_slice())], std::path::Path::new(&output_path))
                    .map(|_| format!("Successfully saved sheet '{}' to '{}'", sheet_name, output_path))
                    .map_err(|e| format!("Failed to save: {}", e))
            } else {
                guard.0.save_as(&file_name, std::path::Path::new(&output_path))
                    .map(|_| format!("Successfully saved '{}' to '{}'", file_name, output_path))
                    .map_err(|e| format!("Failed to save: {}", e))
            }
        })
        .await
        .map_err(|e: tokio::task::JoinError| format!("Task error: {}", e))?;
        result
    }

    #[tool(description = "Update a single cell value. Row index is 0-based. Column is identified by name.")]
    pub async fn update_cell(
        &self,
        Parameters(params): Parameters<UpdateCellParams>,
    ) -> Result<String, String> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let mut guard = db.write();
            guard.0.update_cell(&params.file_name, &params.sheet_name, params.row, &params.column, &params.value)
                .map(|_| format!("Updated cell at row {}, column '{}' to '{}'", params.row, params.column, params.value))
                .map_err(|e| format!("Failed to update cell: {}", e))
        })
        .await
        .map_err(|e| format!("Task error: {}", e))?
    }

    #[tool(description = "Batch update multiple cells. Each update specifies row (0-based index), column (name), and value.")]
    pub async fn update_cells(
        &self,
        Parameters(params): Parameters<UpdateCellsParams>,
    ) -> Result<String, String> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let updates: Vec<(usize, String, String)> = params.updates.into_iter()
                .map(|u| (u.row, u.column, u.value))
                .collect();
            let total = updates.len();
            let mut guard = db.write();
            guard.0.update_cells(&params.file_name, &params.sheet_name, &updates)
                .map(|count| format!("Updated {}/{} cells", count, total))
                .map_err(|e| format!("Failed to update cells: {}", e))
        })
        .await
        .map_err(|e| format!("Task error: {}", e))?
    }

    #[tool(description = "Insert rows at a specified position. Existing rows at and after start_row are shifted down. Each row is an array of string values.")]
    pub async fn insert_rows(
        &self,
        Parameters(params): Parameters<InsertRowsParams>,
    ) -> Result<String, String> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let count = params.rows.len();
            let mut guard = db.write();
            guard.0.insert_rows(&params.file_name, &params.sheet_name, params.start_row, params.rows)
                .map(|_| format!("Inserted {} rows at position {}", count, params.start_row))
                .map_err(|e| format!("Failed to insert rows: {}", e))
        })
        .await
        .map_err(|e| format!("Task error: {}", e))?
    }

    #[tool(description = "Delete rows starting at start_row (0-based). Returns the actual number of rows deleted.")]
    pub async fn delete_rows(
        &self,
        Parameters(params): Parameters<DeleteRowsParams>,
    ) -> Result<String, String> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let mut guard = db.write();
            guard.0.delete_rows(&params.file_name, &params.sheet_name, params.start_row, params.count)
                .map(|actual| format!("Deleted {} rows starting at row {}", actual, params.start_row))
                .map_err(|e| format!("Failed to delete rows: {}", e))
        })
        .await
        .map_err(|e| format!("Task error: {}", e))?
    }

    #[tool(description = "Add a new column to a sheet. All existing rows are filled with the default value.")]
    pub async fn add_column(
        &self,
        Parameters(params): Parameters<AddColumnParams>,
    ) -> Result<String, String> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let default = params.default_value.unwrap_or_default();
            let mut guard = db.write();
            guard.0.add_column(&params.file_name, &params.sheet_name, &params.column_name, &default)
                .map(|_| format!("Added column '{}' with default value '{}'", params.column_name, default))
                .map_err(|e| format!("Failed to add column: {}", e))
        })
        .await
        .map_err(|e| format!("Task error: {}", e))?
    }

    #[tool(description = "Rename a column in a sheet.")]
    pub async fn rename_column(
        &self,
        Parameters(params): Parameters<RenameColumnParams>,
    ) -> Result<String, String> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let mut guard = db.write();
            guard.0.rename_column(&params.file_name, &params.sheet_name, &params.old_name, &params.new_name)
                .map(|_| format!("Renamed column '{}' to '{}'", params.old_name, params.new_name))
                .map_err(|e| format!("Failed to rename column: {}", e))
        })
        .await
        .map_err(|e| format!("Task error: {}", e))?
    }

    #[tool(description = "Save changes back to the original imported file (overwrite). Use save_as to save to a different file.")]
    pub async fn save(
        &self,
        Parameters(params): Parameters<SaveParams>,
    ) -> Result<String, String> {
        let db = Arc::clone(&self.db);
        let import_paths = Arc::clone(&self.import_paths);
        let result: Result<String, String> = tokio::task::spawn_blocking(move || {
            let original_path = import_paths.read().get(&params.file_name).cloned()
                .ok_or_else(|| format!("Original path for '{}' not found. File may not have been imported via import_file, or path tracking lost.", params.file_name))?;

            let guard = db.read();
            if let Some(ref sheet_name) = params.sheet_name {
                let data = guard.0.get_sheet_data(&params.file_name, sheet_name, None, None, None)
                    .map_err(|e| format!("Failed to read sheet data: {}", e))?;
                use crate::engine::write_xlsx;
                let headers = &data.columns;
                let rows = &data.rows;
                write_xlsx(&[(sheet_name.as_str(), headers.as_slice(), rows.as_slice())], std::path::Path::new(&original_path))
                    .map(|_| format!("Overwrote '{}' (sheet '{}')", original_path, sheet_name))
                    .map_err(|e| format!("Failed to save: {}", e))
            } else {
                guard.0.save_as(&params.file_name, std::path::Path::new(&original_path))
                    .map(|_| format!("Overwrote '{}'", original_path))
                    .map_err(|e| format!("Failed to save: {}", e))
            }
        })
        .await
        .map_err(|e: tokio::task::JoinError| format!("Task error: {}", e))?;
        result
    }
}

pub async fn run_mcp_server() -> anyhow::Result<()> {
    let engine = DefaultEngine::new()?;
    let db = Arc::new(RwLock::new(SyncDb(engine)));
    let import_paths = Arc::new(RwLock::new(std::collections::HashMap::new()));
    let server = GrepExcelServer { db, import_paths };
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
