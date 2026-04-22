use std::sync::Arc;

use parking_lot::RwLock;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars;
use rmcp::tool;
use rmcp::tool_router;
use rmcp::ServiceExt;
use serde::{Deserialize, Serialize};

use crate::engine::{DefaultEngine, SearchEngine, SearchMode, SearchQuery};
use crate::types::{FileInfo, SearchResult, SearchStats};

pub(crate) struct SyncDb(pub(crate) DefaultEngine);
unsafe impl Sync for SyncDb {}
unsafe impl Send for SyncDb {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ImportFileParams {
    #[schemars(description = "Absolute or relative path to the Excel/CSV file. Supports xlsx, xls, xlsm, xlsb, ods, csv.")]
    pub file_path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    #[schemars(description = "Search query string")]
    pub query: String,
    #[schemars(description = "Filter to a specific column name")]
    pub column: Option<String>,
    #[schemars(description = "Search mode: fulltext, exact, wildcard, regex")]
    pub mode: Option<String>,
    #[schemars(description = "Maximum results to return (default: 100)")]
    pub limit: Option<usize>,
}

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
pub struct McpSearchResponse {
    pub results: Vec<McpSearchResult>,
    pub stats: McpSearchStats,
}

#[derive(Debug, Serialize)]
pub struct McpFileListResponse {
    pub files: Vec<McpFileInfo>,
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
        tokio::task::spawn_blocking(move || {
            let mut guard = db.write();
            guard
                .0
                .import_excel(&path, &|_, _| {})
                .map(|info| {
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
        };
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let guard = db.read();
            guard
                .0
                .search(&query)
                .map(|(results, stats)| {
                    let response = McpSearchResponse {
                        results: results.into_iter().map(Into::into).collect(),
                        stats: stats.into(),
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
}

pub async fn run_mcp_server() -> anyhow::Result<()> {
    let engine = DefaultEngine::new()?;
    let db = Arc::new(RwLock::new(SyncDb(engine)));
    let server = GrepExcelServer { db };
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
