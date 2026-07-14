use std::sync::atomic::{AtomicU8, Ordering};

static LANG_ATOMIC: AtomicU8 = AtomicU8::new(0); // 0=uninit, 1=zh, 2=en

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Zh,
    En,
}

pub fn init() -> Lang {
    let lang = detect();
    LANG_ATOMIC.store(
        if lang == Lang::Zh { 1 } else { 2 },
        Ordering::Relaxed,
    );
    lang
}

pub fn current() -> Lang {
    match LANG_ATOMIC.load(Ordering::Relaxed) {
        1 => Lang::Zh,
        _ => Lang::En,
    }
}

fn detect() -> Lang {
    let locale = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .unwrap_or_default()
        .to_lowercase();
    if locale.starts_with("zh") {
        Lang::Zh
    } else {
        Lang::En
    }
}

// ─────────────────────────────────────────────────────────────
// Search modes
// ─────────────────────────────────────────────────────────────

pub fn mode_fulltext() -> &'static str {
    match current() {
        Lang::Zh => "全文",
        Lang::En => "FullText",
    }
}

pub fn mode_exact() -> &'static str {
    match current() {
        Lang::Zh => "精确",
        Lang::En => "Exact",
    }
}

pub fn mode_wildcard() -> &'static str {
    match current() {
        Lang::Zh => "通配符",
        Lang::En => "Wildcard",
    }
}

pub fn mode_regex() -> &'static str {
    match current() {
        Lang::Zh => "正则",
        Lang::En => "Regex",
    }
}

pub fn mode_name(mode: crate::types::SearchMode) -> &'static str {
    match mode {
        crate::types::SearchMode::FullText => mode_fulltext(),
        crate::types::SearchMode::ExactMatch => mode_exact(),
        crate::types::SearchMode::Wildcard => mode_wildcard(),
        crate::types::SearchMode::Regex => mode_regex(),
    }
}

// ─────────────────────────────────────────────────────────────
// App modes (title bar)
// ─────────────────────────────────────────────────────────────

pub fn appmode_normal() -> &'static str {
    match current() {
        Lang::Zh => "普通",
        Lang::En => "Normal",
    }
}

pub fn appmode_search() -> &'static str {
    match current() {
        Lang::Zh => "搜索",
        Lang::En => "Search",
    }
}

pub fn appmode_column() -> &'static str {
    match current() {
        Lang::Zh => "列",
        Lang::En => "Column",
    }
}

pub fn appmode_aggregate() -> &'static str {
    match current() {
        Lang::Zh => "聚合",
        Lang::En => "Aggregate",
    }
}

pub fn appmode_help() -> &'static str {
    match current() {
        Lang::Zh => "帮助",
        Lang::En => "Help",
    }
}

pub fn appmode_file() -> &'static str {
    match current() {
        Lang::Zh => "文件",
        Lang::En => "File",
    }
}

pub fn appmode_sql() -> &'static str {
    match current() {
        Lang::Zh => "SQL",
        Lang::En => "SQL",
    }
}

pub fn appmode_detail() -> &'static str {
    match current() {
        Lang::Zh => "详情",
        Lang::En => "Detail",
    }
}

// ─────────────────────────────────────────────────────────────
// Labels (search bar)
// ─────────────────────────────────────────────────────────────

pub fn label_search() -> &'static str {
    match current() { Lang::Zh => "搜索", Lang::En => "Search" }
}

pub fn label_column() -> &'static str {
    match current() { Lang::Zh => "列", Lang::En => "Col" }
}

pub fn label_sql() -> &'static str {
    match current() { Lang::Zh => "SQL", Lang::En => "SQL" }
}

pub fn label_aggregate() -> &'static str {
    match current() { Lang::Zh => "聚合", Lang::En => "Agg" }
}

// ─────────────────────────────────────────────────────────────
// Tabs
// ─────────────────────────────────────────────────────────────

pub fn tab_all(count: usize) -> String {
    match current() {
        Lang::Zh => format!("全部({})", count),
        Lang::En => format!("All({})", count),
    }
}

// ─────────────────────────────────────────────────────────────
// Columns
// ─────────────────────────────────────────────────────────────

pub fn col_source() -> &'static str {
    match current() { Lang::Zh => "来源", Lang::En => "Source" }
}

pub fn col_auto_name(idx: usize) -> String {
    match current() {
        Lang::Zh => format!("列{}", idx),
        Lang::En => format!("Col {}", idx),
    }
}

// ─────────────────────────────────────────────────────────────
// Empty states
// ─────────────────────────────────────────────────────────────

pub fn empty_no_files() -> &'static str {
    match current() { Lang::Zh => "未加载任何文件", Lang::En => "No files loaded" }
}

pub fn empty_open_hint() -> &'static str {
    match current() { Lang::Zh => "按 o 打开文件选择器", Lang::En => "Press 'o' to open file picker" }
}

pub fn empty_help_hint() -> &'static str {
    match current() { Lang::Zh => "按 ? 查看帮助", Lang::En => "Press '?' for help" }
}

pub fn empty_files_title() -> &'static str {
    match current() { Lang::Zh => "已加载的文件", Lang::En => "Loaded Files" }
}

pub fn empty_sheets(count: usize) -> String {
    match current() {
        Lang::Zh => format!("{} 个 sheet", count),
        Lang::En => if count == 1 { "1 sheet".to_string() } else { format!("{} sheets", count) },
    }
}

pub fn file_rows(count: usize) -> String {
    match current() {
        Lang::Zh => format!("{} 行", count),
        Lang::En => if count == 1 { "1 row".to_string() } else { format!("{} rows", count) },
    }
}

pub fn empty_sheet_rows(count: usize) -> String {
    match current() {
        Lang::Zh => format!("{} 行", count),
        Lang::En => if count == 1 { "1 row".to_string() } else { format!("{} rows", count) },
    }
}

pub fn empty_no_results() -> &'static str {
    match current() { Lang::Zh => "无结果", Lang::En => "No results" }
}

pub fn empty_no_matches(query: &str) -> String {
    match current() {
        Lang::Zh => format!("未找到与 '{}' 匹配的结果", query),
        Lang::En => format!("No matches for '{}'", query),
    }
}

pub fn empty_query_label(query: &str) -> String {
    match current() {
        Lang::Zh => format!("查询: {}", query),
        Lang::En => format!("Query: {}", query),
    }
}

pub fn empty_enter_to_search() -> &'static str {
    match current() { Lang::Zh => "按 Enter 执行搜索", Lang::En => "Press Enter to search" }
}

pub fn empty_edit_search_hint() -> &'static str {
    match current() { Lang::Zh => "按 / 编辑搜索", Lang::En => "Press / to edit search" }
}

pub fn empty_search_hint() -> &'static str {
    match current() { Lang::Zh => "搜索  ", Lang::En => "search  " }
}

pub fn empty_help_word() -> &'static str {
    match current() { Lang::Zh => "帮助", Lang::En => "help" }
}

pub fn press_label() -> &'static str {
    match current() { Lang::Zh => "按 ", Lang::En => "Press " }
}

// ─────────────────────────────────────────────────────────────
// Detail panel
// ─────────────────────────────────────────────────────────────

pub fn detail_title() -> &'static str {
    match current() { Lang::Zh => "详情", Lang::En => "Detail" }
}

pub fn detail_file_label() -> &'static str {
    match current() { Lang::Zh => "文件: ", Lang::En => "File: " }
}

pub fn detail_sheet_label() -> &'static str {
    match current() { Lang::Zh => "Sheet: ", Lang::En => "Sheet: " }
}

// ─────────────────────────────────────────────────────────────
// Preview
// ─────────────────────────────────────────────────────────────

pub fn preview(sheet: &str) -> String {
    match current() {
        Lang::Zh => format!("  预览: {}", sheet),
        Lang::En => format!("  Preview: {}", sheet),
    }
}

// ─────────────────────────────────────────────────────────────
// Status bar
// ─────────────────────────────────────────────────────────────

pub fn files_loaded(count: usize) -> String {
    match current() {
        Lang::Zh => format!("{} 个文件", count),
        Lang::En => if count == 1 { "1 file".to_string() } else { format!("{} files", count) },
    }
}

pub fn status_importing(path: &std::path::Path) -> String {
    match current() {
        Lang::Zh => format!("正在导入: {}", path.display()),
        Lang::En => format!("Importing: {}", path.display()),
    }
}

pub fn status_searching() -> &'static str {
    match current() { Lang::Zh => "正在搜索...", Lang::En => "Searching..." }
}

pub fn status_imported(name: &str) -> String {
    match current() {
        Lang::Zh => format!("已导入: {}", name),
        Lang::En => format!("Imported: {}", name),
    }
}

pub fn status_import_error(e: &str) -> String {
    match current() {
        Lang::Zh => format!("导入错误: {}", e),
        Lang::En => format!("Import error: {}", e),
    }
}

pub fn status_import_failed() -> &'static str {
    match current() { Lang::Zh => "导入失败", Lang::En => "Import failed" }
}

pub fn status_search_error(e: &str) -> String {
    match current() {
        Lang::Zh => format!("搜索错误: {}", e),
        Lang::En => format!("Search error: {}", e),
    }
}

pub fn status_search_failed() -> &'static str {
    match current() { Lang::Zh => "搜索失败", Lang::En => "Search failed" }
}

pub fn status_progress(cur: usize, tot: usize) -> String {
    match current() {
        Lang::Zh => format!("正在导入... {}/{}", cur, tot),
        Lang::En => format!("Importing... {}/{}", cur, tot),
    }
}

pub fn status_loading() -> &'static str {
    match current() { Lang::Zh => "加载中...", Lang::En => "Loading..." }
}

pub fn status_browse_loading(file: &str, sheet: &str) -> String {
    match current() {
        Lang::Zh => format!("正在加载: {}/{}...", file, sheet),
        Lang::En => format!("Loading: {}/{}...", file, sheet),
    }
}

pub fn status_browse_loaded(count: usize) -> String {
    match current() {
        Lang::Zh => format!("已加载 {} 行数据", count),
        Lang::En => format!("Browsing: {} rows loaded", count),
    }
}

pub fn status_browse_load_failed() -> &'static str {
    match current() {
        Lang::Zh => "浏览数据加载失败",
        Lang::En => "Browse load failed",
    }
}

pub fn status_browse_load_error(e: &str) -> String {
    match current() {
        Lang::Zh => format!("浏览加载错误: {}", e),
        Lang::En => format!("Browse load error: {}", e),
    }
}

pub fn status_mode_changed(mode: &str) -> String {
    match current() {
        Lang::Zh => format!("模式切换为: {}", mode),
        Lang::En => format!("Mode changed to: {}", mode),
    }
}

pub fn status_cleared() -> &'static str {
    match current() { Lang::Zh => "已清除所有数据", Lang::En => "All data cleared" }
}

pub fn status_matches(total: usize, duration: f64) -> String {
    match current() {
        Lang::Zh => format!("找到 {} 个匹配, 用时 {:.2}s", total, duration),
        Lang::En => format!("Found {} matches in {:.2}s", total, duration),
    }
}

pub fn status_matches_truncated(total: usize, shown: usize, duration: f64) -> String {
    match current() {
        Lang::Zh => format!("找到 {}+ 个匹配 (显示前 {}), 用时 {:.2}s — [n] 加载更多", total, shown, duration),
        Lang::En => format!("Found {}+ matches (showing first {}), took {:.2}s — [n] load more", total, shown, duration),
    }
}

pub fn welcome_loaded(count: usize) -> String {
    match current() {
        Lang::Zh => format!("已加载 {} 个文件。按 'o' 查看, '/' 搜索, '?' 帮助", count),
        Lang::En => format!("Loaded {} files. Press 'o' to view, '/' to search, '?' for help", count),
    }
}

pub fn welcome_empty() -> &'static str {
    match current() { Lang::Zh => "未加载文件。按 'o' 导入, '?' 帮助", Lang::En => "No files loaded. Press 'o' to import, '?' for help" }
}

// ─────────────────────────────────────────────────────────────
// Row / col indicators
// ─────────────────────────────────────────────────────────────

pub fn status_row_indicator(cur: usize, tot: usize) -> String {
    match current() {
        Lang::Zh => format!("行 {}/{}", cur, tot),
        Lang::En => format!("Row {}/{}", cur, tot),
    }
}

pub fn status_row_empty() -> &'static str {
    match current() { Lang::Zh => "无行", Lang::En => "No rows" }
}

pub fn status_col_range(_start: usize, end: usize, total: usize) -> String {
    match current() {
        Lang::Zh => format!(" | 列 {}/{}", end, total),
        Lang::En => format!(" | Col {}/{}", end, total),
    }
}

pub fn status_matches_label(matches: usize, rows: usize) -> String {
    match current() {
        Lang::Zh => format!("匹配 {}/{}", matches, rows),
        Lang::En => format!("Matches {}/{}", matches, rows),
    }
}

// ─────────────────────────────────────────────────────────────
// Error messages
// ─────────────────────────────────────────────────────────────

pub fn err_no_files() -> &'static str {
    match current() { Lang::Zh => "没有已加载的文件", Lang::En => "No files loaded" }
}

pub fn export_no_results() -> &'static str {
    match current() { Lang::Zh => "没有可导出的结果", Lang::En => "No results to export" }
}

pub fn export_done(path: &str) -> String {
    match current() {
        Lang::Zh => format!("已导出到 {}", path),
        Lang::En => format!("Exported to {}", path),
    }
}

pub fn export_failed() -> &'static str {
    match current() { Lang::Zh => "导出失败", Lang::En => "Export failed" }
}

pub fn export_no_dialog() -> &'static str {
    match current() { Lang::Zh => "未启用文件对话框功能", Lang::En => "File dialog feature not enabled" }
}

pub fn export_error(e: &str) -> String {
    match current() {
        Lang::Zh => format!("导出错误: {}", e),
        Lang::En => format!("Export error: {}", e),
    }
}

// ─────────────────────────────────────────────────────────────
// Hints (bottom bar)
// ─────────────────────────────────────────────────────────────

pub fn hint_search() -> &'static str {
    match current() { Lang::Zh => "搜索  ", Lang::En => "search " }
}

pub fn hint_col() -> &'static str {
    match current() { Lang::Zh => "列  ", Lang::En => "col  " }
}

pub fn hint_mode() -> &'static str {
    match current() { Lang::Zh => "模式  ", Lang::En => "mode  " }
}

pub fn hint_open() -> &'static str {
    match current() { Lang::Zh => "打开  ", Lang::En => "open  " }
}

pub fn hint_export() -> &'static str {
    match current() { Lang::Zh => "导出  ", Lang::En => "export " }
}

pub fn hint_clear() -> &'static str {
    match current() { Lang::Zh => "清除  ", Lang::En => "clear " }
}

pub fn hint_help() -> &'static str {
    match current() { Lang::Zh => "帮助  ", Lang::En => "help  " }
}

pub fn hint_quit() -> &'static str {
    match current() { Lang::Zh => "退出", Lang::En => "quit" }
}

pub fn hint_view() -> &'static str {
    match current() { Lang::Zh => "视图  ", Lang::En => "view  " }
}

pub fn hint_aggregate() -> &'static str {
    match current() { Lang::Zh => "聚合  ", Lang::En => "agg  " }
}

pub fn hint_execute() -> &'static str {
    match current() { Lang::Zh => "执行  ", Lang::En => "exec  " }
}

pub fn hint_cancel() -> &'static str {
    match current() { Lang::Zh => "取消  ", Lang::En => "cancel " }
}

pub fn hint_toggle_mode() -> &'static str {
    match current() { Lang::Zh => "切换模式", Lang::En => "toggle mode" }
}

pub fn hint_confirm() -> &'static str {
    match current() { Lang::Zh => "确认  ", Lang::En => "ok  " }
}

pub fn hint_cancel_short() -> &'static str {
    match current() { Lang::Zh => "取消", Lang::En => "cancel" }
}

pub fn hint_close_help() -> &'static str {
    match current() { Lang::Zh => "关闭帮助", Lang::En => "close help" }
}

pub fn hint_up() -> &'static str {
    match current() { Lang::Zh => "上", Lang::En => "up" }
}

pub fn hint_down() -> &'static str {
    match current() { Lang::Zh => "下", Lang::En => "down" }
}

pub fn hint_select() -> &'static str {
    match current() { Lang::Zh => "选择", Lang::En => "select" }
}

pub fn hint_close() -> &'static str {
    match current() { Lang::Zh => "关闭", Lang::En => "close" }
}

pub fn hint_scroll_up() -> &'static str {
    match current() { Lang::Zh => "上滚", Lang::En => "scroll up" }
}

pub fn hint_scroll_down() -> &'static str {
    match current() { Lang::Zh => "下滚", Lang::En => "scroll down" }
}

pub fn status_view_flat() -> &'static str {
    match current() { Lang::Zh => "平铺", Lang::En => "Flat" }
}

pub fn status_view_table() -> &'static str {
    match current() { Lang::Zh => "表格", Lang::En => "Table" }
}

pub fn status_flat_sheet(file: &str, sheet: &str, index: usize, total: usize) -> String {
    match current() {
        Lang::Zh => format!("{} / {} ({}/{})", file, sheet, index, total),
        Lang::En => format!("{} / {} ({}/{})", file, sheet, index, total),
    }
}

pub fn help_nav_sheet_in_file() -> &'static str {
    match current() {
        Lang::Zh => "切换文件内 Sheet",
        Lang::En => "Switch sheet within file",
    }
}

pub fn help_nav_file() -> &'static str {
    match current() {
        Lang::Zh => "切换文件",
        Lang::En => "Switch file",
    }
}

// ─────────────────────────────────────────────────────────────
// Help popup
// ─────────────────────────────────────────────────────────────

pub fn help_title() -> &'static str {
    match current() { Lang::Zh => "帮助", Lang::En => "Help" }
}

pub fn help_close_hint() -> &'static str {
    match current() { Lang::Zh => "按 Esc, q, ? 或 h 关闭帮助", Lang::En => "Press Esc, q, ? or h to close help" }
}

pub fn help_group_nav() -> &'static str {
    match current() { Lang::Zh => "导航", Lang::En => "Navigation" }
}

pub fn help_nav_up() -> &'static str {
    match current() { Lang::Zh => "上移", Lang::En => "Move up" }
}

pub fn help_nav_down() -> &'static str {
    match current() { Lang::Zh => "下移", Lang::En => "Move down" }
}

pub fn help_nav_top() -> &'static str {
    match current() { Lang::Zh => "跳到顶部", Lang::En => "Jump to top" }
}

pub fn help_nav_bottom() -> &'static str {
    match current() { Lang::Zh => "跳到底部", Lang::En => "Jump to bottom" }
}

pub fn help_nav_scroll_cols() -> &'static str {
    match current() { Lang::Zh => "左右滚动列", Lang::En => "Scroll columns left/right" }
}

pub fn help_nav_tab() -> &'static str {
    match current() { Lang::Zh => "切换标签页", Lang::En => "Switch tabs" }
}

pub fn help_group_search() -> &'static str {
    match current() { Lang::Zh => "搜索", Lang::En => "Search" }
}

pub fn help_search_input() -> &'static str {
    match current() { Lang::Zh => "输入搜索查询", Lang::En => "Enter search query" }
}

pub fn help_search_col() -> &'static str {
    match current() { Lang::Zh => "设置列过滤器", Lang::En => "Set column filter" }
}

pub fn help_search_sql() -> &'static str {
    match current() { Lang::Zh => "进入 SQL 查询模式", Lang::En => "Enter SQL query mode" }
}

pub fn help_search_toggle() -> &'static str {
    match current() { Lang::Zh => "切换搜索模式", Lang::En => "Toggle search mode" }
}

pub fn help_search_exec() -> &'static str {
    match current() { Lang::Zh => "执行搜索", Lang::En => "Execute search" }
}

pub fn help_group_general() -> &'static str {
    match current() { Lang::Zh => "通用", Lang::En => "General" }
}

pub fn help_gen_open() -> &'static str {
    match current() { Lang::Zh => "打开文件", Lang::En => "Open file" }
}

pub fn help_gen_clear() -> &'static str {
    match current() { Lang::Zh => "清除所有数据", Lang::En => "Clear all data" }
}

pub fn help_gen_export() -> &'static str {
    match current() { Lang::Zh => "导出结果为 CSV", Lang::En => "Export results to CSV" }
}

pub fn help_gen_more() -> &'static str {
    match current() { Lang::Zh => "加载更多结果", Lang::En => "Load more results" }
}

pub fn help_gen_toggle_help() -> &'static str {
    match current() { Lang::Zh => "切换帮助", Lang::En => "Toggle help" }
}

pub fn help_gen_quit() -> &'static str {
    match current() { Lang::Zh => "退出", Lang::En => "Quit" }
}

// ─────────────────────────────────────────────────────────────
// CLI output
// ─────────────────────────────────────────────────────────────

pub fn cli_file_not_found(path: &str) -> String {
    match current() {
        Lang::Zh => format!("文件未找到: {}", path),
        Lang::En => format!("File not found: {}", path),
    }
}

pub fn cli_imported(name: &str, sheets: usize, rows: usize) -> String {
    match current() {
        Lang::Zh => format!("已导入: {} ({} 个 sheet, {} 行)", name, sheets, rows),
        Lang::En => format!("Imported: {} ({} sheets, {} rows)", name, sheets, rows),
    }
}

pub fn cli_import_failed(path: &str, e: &str) -> String {
    match current() {
        Lang::Zh => format!("导入失败 {}: {}", path, e),
        Lang::En => format!("Failed to import {}: {}", path, e),
    }
}

pub fn cli_search_failed(e: &str) -> String {
    match current() {
        Lang::Zh => format!("搜索失败: {}", e),
        Lang::En => format!("Search failed: {}", e),
    }
}

pub fn cli_no_matches(query: &str) -> String {
    match current() {
        Lang::Zh => format!("未找到与 '{}' 匹配的结果", query),
        Lang::En => format!("No matches for '{}'", query),
    }
}

pub fn cli_match_summary(matches: usize, rows: usize, duration_ms: u128) -> String {
    match current() {
        Lang::Zh => format!("找到 {} 个匹配 (搜索了 {} 行, 用时 {}ms)", matches, rows, duration_ms),
        Lang::En => format!("Found {} matches across {} rows in {}ms", matches, rows, duration_ms),
    }
}

pub fn cli_export_done(path: &str) -> String {
    match current() {
        Lang::Zh => format!("已导出到 {}", path),
        Lang::En => format!("Exported to {}", path),
    }
}

pub fn cli_export_failed() -> &'static str {
    match current() { Lang::Zh => "导出失败", Lang::En => "Export failed" }
}

// ─────────────────────────────────────────────────────────────
// Full help text (--help)
// ─────────────────────────────────────────────────────────────

#[cfg(feature = "share-url")]
fn kdocs_help_line_zh() -> String {
    "    --kdocs-cookie <COOKIE>  金山文档 (kdocs.cn) 分享链接下载专用 Cookie\n\
     --share-hosts <HOSTS>   额外域名 (逗号分隔)，用于企业版云文档链接\n".to_string()
}

#[cfg(not(feature = "share-url"))]
fn kdocs_help_line_zh() -> String {
    String::new()
}

#[cfg(feature = "share-url")]
fn kdocs_help_line_en() -> String {
    "    --kdocs-cookie <COOKIE>  Cookie for Kingsoft Docs (kdocs.cn) share URL downloads\n\
         --share-hosts <HOSTS>   Additional comma-separated hosts for enterprise share URLs\n".to_string()
}

#[cfg(not(feature = "share-url"))]
fn kdocs_help_line_en() -> String {
    String::new()
}

pub fn help_full_text() -> String {
    let kdocs_line_zh = kdocs_help_line_zh();
    let kdocs_line_en = kdocs_help_line_en();
    match current() {
        Lang::Zh => {
            let version = env!("CARGO_PKG_VERSION");
            format!(
                 "grep_excel {version}\n\n\
                 基于 DuckDB 的多格式表格文件搜索 TUI 工具\n\n\
                   用法: grep_excel [FILES...] [OPTIONS]\n\n\
                    选项:\n\
                                      -i, --interactive        进入交互式 SQL REPL ($ 提示符, 历史浏览, 多行)\n\
                                          --no-history         禁用跨会话 SQL 历史持久化 (默认保存)\n\
                                     -q, --query <QUERY>      搜索查询字符串\n\
                                     -c, --column <COLUMN>    筛选指定列名\n\
                                     -s, --sheet <SHEET>      筛选指定 Sheet 名称\n\
                                     -m, --mode <MODE>        搜索模式 [默认: fulltext]\n\
                                              可选: fulltext, exact, wildcard, regex\n\
                                     -v, --invert             反向匹配: 返回未命中的行\n\
                                     -e, --export <PATH>      将搜索结果导出为 CSV 文件\n\
                                      -x, --sql <QUERY>        执行 SQL SELECT 查询\n\
                                      -t, --list-tables        列出已导入表及其别名和列名\n\
                                       -f, --format <FORMAT>    输出格式: markdown (默认), pretty, json, simple (TSV)\n\
                                       -g, --aggregate <COL>    聚合列: 统计匹配行在该列的值分布\n\
                                           -E, --exec <JSON>        执行 MCP 工具命令 (JSON 格式)\n\
                                       -X, --run <CMD>          对每个匹配行执行 Shell 命令 (${{列名}} 占位符)\n\
                                             --run-output-column <COL>    --run 模式: 命令 stdout 写入该列\n\
                                             --mcp              启动 MCP Server 模式 (stdio)\n\
                                        -r, --repair             导入前尝试修复损坏的 xlsx 文件\n\
{kdocs_line_zh}\
                                        -h, --help               显示帮助信息\n\
                                       -V, --version            显示版本号\n\n\
                  支持的文件格式:\n\
                    .xlsx  .xls  .xlsm  .xlsb  .ods  (Excel/电子表格)\n\
                    .csv                                (逗号分隔)\n\
                    .html  .htm                         (HTML 表格, 自动检测编码)\n\
                    .txt   .md   .markdown              (文本/Markdown 表格)\n\n\
                 搜索模式:\n\n\
                 \x1b[1mfulltext\x1b[0m (默认)\n\
                 \x1b[3m不区分大小写的子串匹配。\x1b[0m 匹配所有包含查询文本的单元格，\n\
                 忽略大小写。适用于一般性搜索。\n\
                 \x1b[2m示例: --query \"john\" 匹配 \"John Smith\"、\"Johnson\"、\"JOHN\"\x1b[0m\n\n\
                 \x1b[1mexact\x1b[0m\n\
                 \x1b[3m区分大小写的精确匹配。\x1b[0m 整个单元格内容必须完全等于查询文本。\n\
                 适用于精确查找 ID 或确切名称。\n\
                 \x1b[2m示例: --query \"Engineering\" 仅匹配内容恰好为 \"Engineering\" 的单元格\x1b[0m\n\n\
                 \x1b[1mwildcard\x1b[0m\n\
                 \x1b[3mSQL LIKE 风格的模式匹配。\x1b[0m 不区分大小写。两种通配符:\n\
                 \x1b[1m%\x1b[0m = 任意字符序列（包括空）\n\
                 \x1b[1m_\x1b[0m = 恰好一个字符\n\
                 \x1b[2m示例: --query \"San%\" --mode wildcard  → \"San Francisco\"、\"San Jose\"\x1b[0m\n\
                 \x1b[2m示例: --query \"A__\" --mode wildcard  → \"ABC\"、\"Amy\"\x1b[0m\n\n\
                 \x1b[1mregex\x1b[0m\n\
                 \x1b[3m正则表达式匹配。\x1b[0m 不区分大小写。使用 | 进行多关键词 OR 搜索。\n\
                 支持完整的 Rust 正则语法。\n\
                 \x1b[2m示例: --query \"张三|李四\" --mode regex  → 匹配包含任一关键词的单元格\x1b[0m\n\
                 \x1b[2m示例: --query \"\\d{{4}}-\\d{{2}}-\\d{{2}}\" --mode regex  → 匹配日期格式\x1b[0m\n\n\
                    \x1b[1m提示:\x1b[0m\n\
                    • 使用 --column 限定搜索范围到指定列名\n\
                    • 组合 --query 和 --mode 进行 CLI 一次性搜索\n\
                    • 使用 --aggregate <列名> 对匹配结果按指定列做值分布统计\n\
                    • 使用 --list-tables 查看文件到表名的映射关系\n\
                    • SQL 查询支持友好表名: 文件名.工作表名 (如 employees.Sheet1)\n\
                    • HTML/文本文件自动检测编码 (UTF-8 / meta charset / CJK 回退)\n\
                    • 不带 --query 运行将启动交互式 TUI；导入后自动浏览首个 sheet\n\
                    • TUI: Ctrl+←/→ 切换同文件 sheet，Ctrl+↑/↓ 切换文件\n\
                    • REPL (-i): .output / .save 可导出 SQL 结果；.help 查看全部点命令\n\
                    • 使用 --run <命令> 对每个匹配行执行 Shell 命令 (用 ${{列名}} 引用单元格)\n\
                    • 使用 --exec 执行 MCP 工具命令: 单条 '{{\"tool\":\"search\",\"params\":{{\"query\":\"关键词\"}}}}' 或数组 '[{{...}},{{...}}]'\n\
                    • 使用 --run --help 或 --exec --help 查看详细用法\n"
            )
        }
        Lang::En => {
            let version = env!("CARGO_PKG_VERSION");
            format!(
                 "grep_excel {version}\n\n\
                 TUI tool for searching tabular data files with DuckDB-powered performance.\n\n\
                   Usage: grep_excel [FILES...] [OPTIONS]\n\n\
                    Options:\n\
                                      -i, --interactive        Enter interactive SQL REPL ($ prompt, history, multi-line)\n\
                                          --no-history         Disable persistent SQL history across sessions (on by default)\n\
                                     -q, --query <QUERY>      Search query string\n\
                                     -c, --column <COLUMN>    Filter to a specific column name\n\
                                     -s, --sheet <SHEET>      Filter to a specific sheet name\n\
                                     -m, --mode <MODE>        Search mode [default: fulltext]\n\
                                               Choices: fulltext, exact, wildcard, regex\n\
                                     -v, --invert             Invert match: show non-matching rows\n\
                                     -e, --export <PATH>      Export search results to a CSV file\n\
                                      -x, --sql <QUERY>        Execute a SQL SELECT query\n\
                                      -t, --list-tables        List imported tables with aliases and columns\n\
                                       -f, --format <FORMAT>    Output format: markdown (default), pretty, json, simple (TSV)\n\
                                       -g, --aggregate <COL>    Aggregate column: count distinct values in matched rows\n\
                                        -E, --exec <JSON>        Execute MCP tool command(s) as JSON\n\
                                       -X, --run <CMD>          Run a shell command for each matching row (${{col}} placeholders)\n\
                                             --run-output-column <COL>   --run mode: write command stdout to this column\n\
                                             --mcp              Start MCP Server mode (stdio)\n\
                                        -r, --repair             Try to repair damaged xlsx files before importing\n\
{kdocs_line_en}\
                                        -h, --help               Show help information\n\
                                       -V, --version            Show version\n\n\
                  Supported Formats:\n\
                    .xlsx  .xls  .xlsm  .xlsb  .ods  (Excel / Spreadsheets)\n\
                    .csv                               (Comma-separated)\n\
                    .html  .htm                        (HTML tables, auto-detect encoding)\n\
                    .txt   .md   .markdown             (Text / Markdown tables)\n\n\
                 Search Modes:\n\n\
                 \x1b[1mfulltext\x1b[0m (default)\n\
                 \x1b[3mCase-insensitive substring match.\x1b[0m Matches any cell containing the query\n\
                 text, regardless of case. Best for general-purpose searching.\n\
                 \x1b[2mExample: --query \"john\" matches \"John Smith\", \"Johnson\", \"JOHN\"\x1b[0m\n\n\
                 \x1b[1mexact\x1b[0m\n\
                 \x1b[3mCase-sensitive exact match.\x1b[0m The entire cell content must exactly equal\n\
                 the query text. Use for precise lookups like IDs or exact names.\n\
                 \x1b[2mExample: --query \"Engineering\" matches only cells that are exactly \"Engineering\"\x1b[0m\n\n\
                 \x1b[1mwildcard\x1b[0m\n\
                 \x1b[3mSQL LIKE-style pattern matching.\x1b[0m Case-insensitive. Two wildcards:\n\
                 \x1b[1m%\x1b[0m = any sequence of characters (including empty)\n\
                 \x1b[1m_\x1b[0m = exactly one character\n\
                 \x1b[2mExample: --query \"San%\" --mode wildcard  → \"San Francisco\", \"San Jose\"\x1b[0m\n\
                 \x1b[2mExample: --query \"A__\" --mode wildcard  → \"ABC\", \"Amy\"\x1b[0m\n\n\
                 \x1b[1mregex\x1b[0m\n\
                 \x1b[3mRegular expression match.\x1b[0m Case-insensitive. Use | for OR across multiple\n\
                 keywords. Supports full Rust regex syntax.\n\
                 \x1b[2mExample: --query \"foo|bar\" --mode regex  → matches cells containing either keyword\x1b[0m\n\
                 \x1b[2mExample: --query \"\\d{{4}}-\\d{{2}}-\\d{{2}}\" --mode regex  → matches date patterns\x1b[0m\n\n\
                    \x1b[1mTips:\x1b[0m\n\
                    • Use --column to restrict search to a specific column name\n\
                    • Combine --query and --mode for CLI one-shot search\n\
                    • Use --aggregate <column> to count distinct values in matched rows\n\
                    • Use --list-tables to see file-to-table name mapping\n\
                    • SQL queries support friendly names: filename.sheetname (e.g. employees.Sheet1)\n\
                    • HTML/text files auto-detect encoding (UTF-8 / meta charset / CJK fallback)\n\
                    • Run without --query to launch TUI; import auto-browses the first sheet\n\
                    • TUI: Ctrl+←/→ switch sheet within file, Ctrl+↑/↓ switch file\n\
                    • REPL (-i): .output / .save export SQL results; .help lists all dot-commands\n\
                    • Use --run <cmd> to run a shell command per matching row (use ${{col}} for cell values)\n\
                    • Use --exec to run MCP tools: single '{{\"tool\":\"search\",\"params\":{{\"query\":\"term\"}}}}' or array '[{{...}},{{...}}]'\n\
                    • Use --run --help or --exec --help for detailed usage\n"
            )
        }
    }
}

pub fn status_executing_sql() -> &'static str {
    match current() { Lang::Zh => "执行 SQL...", Lang::En => "Executing SQL..." }
}

pub fn status_sql_done(count: usize, duration: f64) -> String {
    match current() {
        Lang::Zh => format!("SQL 查询完成: {} 行, 用时 {:.2}s", count, duration),
        Lang::En => format!("SQL complete: {} rows, took {:.2}s", count, duration),
    }
}

pub fn status_sql_truncated(count: usize, limit: usize, duration: f64) -> String {
    match current() {
        Lang::Zh => format!(
            "SQL 查询完成: {}+ 行 (显示前 {}), 用时 {:.2}s — [n] 加载更多",
            count, limit, duration
        ),
        Lang::En => format!(
            "SQL complete: {}+ rows (showing first {}), took {:.2}s — [n] load more",
            count, limit, duration
        ),
    }
}

pub fn status_sql_error(e: &str) -> String {
    match current() {
        Lang::Zh => format!("SQL 错误: {}", e),
        Lang::En => format!("SQL error: {}", e),
    }
}

pub fn status_sql_failed() -> &'static str {
    match current() { Lang::Zh => "SQL 执行失败", Lang::En => "SQL execution failed" }
}

pub fn sql_no_results() -> &'static str {
    match current() { Lang::Zh => "SQL 查询无结果", Lang::En => "SQL query returned no results" }
}

pub fn sql_results_title(count: usize) -> String {
    match current() {
        Lang::Zh => format!(" SQL 结果 ({} 行) ", count),
        Lang::En => format!(" SQL Results ({} rows) ", count),
    }
}

pub fn hint_sql() -> &'static str {
    match current() { Lang::Zh => "SQL  ", Lang::En => "SQL  " }
}

pub fn cli_sql_failed(e: &str) -> String {
    match current() {
        Lang::Zh => format!("SQL 执行失败: {}", e),
        Lang::En => format!("SQL execution failed: {}", e),
    }
}

pub fn cli_sql_no_results() -> &'static str {
    match current() { Lang::Zh => "SQL 查询无结果", Lang::En => "SQL query returned no results" }
}

pub fn cli_aggregate_label(col: &str) -> String {
    match current() {
        Lang::Zh => format!("聚合列 '{}':", col),
        Lang::En => format!("Aggregate column '{}':", col),
    }
}

pub fn cli_aggregate_no_data(col: &str) -> String {
    match current() {
        Lang::Zh => format!("聚合列 '{}': 无数据", col),
        Lang::En => format!("Aggregate column '{}': no data", col),
    }
}

pub fn filelist_meta(sheets: usize, rows: usize) -> String {
    match current() {
        Lang::Zh => format!("{} 个 sheet, {} 行", sheets, rows),
        Lang::En => format!("{} sheets, {} rows", sheets, rows),
    }
}

pub fn filelist_title() -> &'static str {
    match current() { Lang::Zh => " 文件列表 ", Lang::En => " File List " }
}

pub fn export_dialog_title() -> &'static str {
    match current() { Lang::Zh => "导出结果", Lang::En => "Export Results" }
}

pub fn cli_list_tables_empty() -> &'static str {
    match current() {
        Lang::Zh => "未导入任何文件。请指定文件路径。",
        Lang::En => "No files imported. Please specify file paths.",
    }
}

pub fn cli_list_tables_header() -> &'static str {
    match current() {
        Lang::Zh => "可用表:",
        Lang::En => "Available tables:",
    }
}

pub fn cli_list_tables_entry(alias: &str, table_name: &str, rows: usize, columns: &str) -> String {
    match current() {
        Lang::Zh => format!("{} → {} ({} 行) [{}]", alias, table_name, rows, columns),
        Lang::En => format!("{} → {} ({} rows) [{}]", alias, table_name, rows, columns),
    }
}

pub fn cli_list_tables_footer(count: usize) -> String {
    match current() {
        Lang::Zh => format!("共 {} 张表。在 SQL 中使用别名或内部名称进行查询。", count),
        Lang::En => format!("{} table(s) total. Use alias or internal name in SQL queries.", count),
    }
}

pub fn sql_info_title() -> &'static str {
    match current() {
        Lang::Zh => " 可用表 (SQL) ",
        Lang::En => " Available Tables (SQL) ",
    }
}

pub fn sql_info_col_alias() -> &'static str {
    match current() { Lang::Zh => "别名", Lang::En => "Alias" }
}

pub fn sql_info_col_table() -> &'static str {
    match current() { Lang::Zh => "内部名", Lang::En => "Table" }
}

pub fn sql_info_col_columns() -> &'static str {
    match current() { Lang::Zh => "列", Lang::En => "Columns" }
}

pub fn sql_info_footer() -> &'static str {
    match current() {
        Lang::Zh => " Enter: 输入SQL │ Esc: 取消 │ ↑↓: 滚动 ",
        Lang::En => " Enter: Type SQL │ Esc: Cancel │ ↑↓: Scroll ",
    }
}

pub fn status_no_tables() -> &'static str {
    match current() {
        Lang::Zh => "未导入任何文件，请先按 o 导入文件",
        Lang::En => "No files imported. Press o to import files first",
    }
}

// ─────────────────────────────────────────────────────────────
// Interactive REPL (-i / --interactive)
// ─────────────────────────────────────────────────────────────

pub fn repl_welcome(version: &str) -> String {
    match current() {
        Lang::Zh => format!("grep_excel {} — 交互式 SQL REPL", version),
        Lang::En => format!("grep_excel {} — Interactive SQL REPL", version),
    }
}

pub fn repl_hint() -> &'static str {
    match current() {
        Lang::Zh => "输入 SQL (以 ; 结束) 执行，或输入 .help 查看命令列表，.exit 退出。历史会跨会话保存（用 --no-history 关闭）",
        Lang::En => "Type SQL ending with ';' to execute. Type .help for commands, .exit to quit. History persists across sessions (use --no-history to disable)",
    }
}

pub fn repl_goodbye() -> &'static str {
    match current() { Lang::Zh => "再见", Lang::En => "Goodbye" }
}

pub fn repl_no_files() -> &'static str {
    match current() {
        Lang::Zh => "未导入任何文件。",
        Lang::En => "No files imported.",
    }
}

pub fn repl_history_empty() -> &'static str {
    match current() {
        Lang::Zh => "(历史为空)",
        Lang::En => "(history empty)",
    }
}

pub fn repl_unknown_dot(cmd: &str) -> String {
    match current() {
        Lang::Zh => format!(
            "未知命令: {}。输入 .help 查看可用命令。",
            cmd
        ),
        Lang::En => format!(
            "Unknown command: {}. Type .help for available commands.",
            cmd
        ),
    }
}

pub fn repl_sql_error(msg: &str) -> String {
    match current() {
        Lang::Zh => format!("错误: {}", msg),
        Lang::En => format!("Error: {}", msg),
    }
}

pub fn repl_sql_summary(shown: usize, total: usize, truncated: bool, duration_ms: u128) -> String {
    match current() {
        Lang::Zh => {
            if truncated {
                format!(
                    "显示 {} / {}+ 行 (已截断)，用时 {} ms",
                    shown, total, duration_ms
                )
            } else {
                format!("{} 行，用时 {} ms", shown, duration_ms)
            }
        }
        Lang::En => {
            if truncated {
                format!(
                    "Showing {} of {}+ rows (truncated), took {} ms",
                    shown, total, duration_ms
                )
            } else {
                format!("{} row(s), took {} ms", shown, duration_ms)
            }
        }
    }
}

pub fn repl_output_on(path: &str) -> String {
    match current() {
        Lang::Zh => format!("已切换输出到 {}", path),
        Lang::En => format!("Output redirected to {}", path),
    }
}

pub fn repl_output_off() -> String {
    match current() {
        Lang::Zh => "已恢复输出到终端".to_string(),
        Lang::En => "Output restored to terminal".to_string(),
    }
}

pub fn repl_output_error(msg: &str) -> String {
    match current() {
        Lang::Zh => format!("输出错误: {}", msg),
        Lang::En => format!("Output error: {}", msg),
    }
}

pub fn repl_output_open_error(path: &str, err: &str) -> String {
    match current() {
        Lang::Zh => format!("无法打开输出文件 '{}': {}", path, err),
        Lang::En => format!("Cannot open output file '{}': {}", path, err),
    }
}

pub fn repl_output_write_error(err: &str) -> String {
    match current() {
        Lang::Zh => format!("写入输出文件失败: {}", err),
        Lang::En => format!("Error writing to output file: {}", err),
    }
}

pub fn repl_save_done(path: &str, rows: usize) -> String {
    match current() {
        Lang::Zh => format!("已保存 {} 行到 {}", rows, path),
        Lang::En => format!("Saved {} row(s) to {}", rows, path),
    }
}

pub fn repl_save_no_result() -> String {
    match current() {
        Lang::Zh => "没有可保存的上次 SQL 结果".to_string(),
        Lang::En => "No previous SQL result to save".to_string(),
    }
}

pub fn repl_save_error(path: &str, err: &str) -> String {
    match current() {
        Lang::Zh => format!("保存到 '{}' 失败: {}", path, err),
        Lang::En => format!("Failed to save to '{}': {}", path, err),
    }
}

pub fn repl_save_truncated() -> String {
    match current() {
        Lang::Zh => "注意: 结果被截断 (终端显示限制). 使用 .output <文件> 可导出完整数据。".to_string(),
        Lang::En => "Note: result was truncated (terminal display limit). Use .output <file> for full export.".to_string(),
    }
}

pub fn repl_help() -> String {
    match current() {
        Lang::Zh => {
            "REPL 命令:\n\
             \x1b[1m.help\x1b[0m              显示此帮助\n\
             \x1b[1m.exit\x1b[0m / \x1b[1m.quit\x1b[0m      退出 REPL (也可 Ctrl-D)\n\
             \x1b[1m.tables\x1b[0m / \x1b[1m.schema\x1b[0m  列出可用表别名和列名\n\
             \x1b[1m.files\x1b[0m             列出已导入文件\n\
             \x1b[1m.history\x1b[0m           显示 SQL 历史\n\
             \x1b[1m.clear\x1b[0m / \x1b[1m.cls\x1b[0m      清屏\n\
             \x1b[1m.output <文件>\x1b[0m     持续重定向 SQL 结果到文件 (CSV 格式)\n\
             \x1b[1m.output\x1b[0m            恢复终端输出\n\
             \x1b[1m.save <文件> [fmt]\x1b[0m  保存上次 SQL 结果到文件 (fmt: csv|json|tsv|table)\n\n\
             SQL 执行:\n\
             • 输入以 \x1b[1m;\x1b[0m 结尾即执行；未结束时显示 \x1b[1m> \x1b[0m 续行提示\n\
             • \x1b[2mSELECT 1;\x1b[0m  →  立即执行\n\
             • \x1b[2mSELECT\\n  *\\nFROM t;  →  跨多行直到 ;\x1b[0m\n\n\
             行编辑 (readline 风格):\n\
             • \x1b[1m↑ / ↓\x1b[0m            浏览历史 (可编辑后重新执行)\n\
             • \x1b[1m← / →\x1b[0m            移动光标\n\
             • \x1b[1mHome / End\x1b[0m       跳到行首/行尾\n\
             • \x1b[1mCtrl-A / Ctrl-E\x1b[0m   同 Home / End\n\
             • \x1b[1mCtrl-U\x1b[0m            删除至行首\n\
             • \x1b[1mCtrl-K\x1b[0m            删除至行尾\n\
             • \x1b[1mCtrl-L\x1b[0m            清屏\n\
             • \x1b[1mCtrl-C\x1b[0m            取消当前输入 (清空多行 SQL 缓冲)\n\
             • \x1b[1mCtrl-D\x1b[0m            空行时退出；非空时删除光标处字符"
                .to_string()
        }
        Lang::En => {
            "REPL commands:\n\
             \x1b[1m.help\x1b[0m              Show this help\n\
             \x1b[1m.exit\x1b[0m / \x1b[1m.quit\x1b[0m      Exit the REPL (also Ctrl-D)\n\
             \x1b[1m.tables\x1b[0m / \x1b[1m.schema\x1b[0m  List available table aliases and columns\n\
             \x1b[1m.files\x1b[0m             List imported files\n\
             \x1b[1m.history\x1b[0m           Show SQL history\n\
             \x1b[1m.clear\x1b[0m / \x1b[1m.cls\x1b[0m      Clear screen\n\
             \x1b[1m.output <file>\x1b[0m     Continuously redirect SQL results to file (CSV)\n\
             \x1b[1m.output\x1b[0m            Restore terminal output\n\
             \x1b[1m.save <file> [fmt]\x1b[0m Save last SQL result to file (fmt: csv|json|tsv|table)\n\n\
             SQL execution:\n\
             • Input executes when it ends with \x1b[1m;\x1b[0m; mid-statement shows \x1b[1m> \x1b[0m continuation prompt\n\
             • \x1b[2mSELECT 1;\x1b[0m  →  executes immediately\n\
             • \x1b[2mSELECT\\n  *\\nFROM t;  →  spans lines until ;\x1b[0m\n\n\
             Line editing (readline-style):\n\
             • \x1b[1mUp / Down\x1b[0m        Browse history (edit then re-run)\n\
             • \x1b[1mLeft / Right\x1b[0m     Move cursor\n\
             • \x1b[1mHome / End\x1b[0m       Jump to start/end of line\n\
             • \x1b[1mCtrl-A / Ctrl-E\x1b[0m   Same as Home / End\n\
             • \x1b[1mCtrl-U\x1b[0m            Delete to start of line\n\
             • \x1b[1mCtrl-K\x1b[0m            Delete to end of line\n\
             • \x1b[1mCtrl-L\x1b[0m            Clear screen\n\
             • \x1b[1mCtrl-C\x1b[0m            Abort current input (clears multi-line SQL buffer)\n\
              • \x1b[1mCtrl-D\x1b[0m            Exit on empty line; delete char at cursor otherwise"
                .to_string()
        }
    }
}

pub fn share_needs_auth(url: &str) -> String {
    match current() {
        Lang::Zh => format!(
            "云文档链接需要登录凭证: {}\n请设置 KDOCS_COOKIE 环境变量，或使用 --kdocs-cookie 参数。\n获取方式：浏览器登录 kdocs.cn → F12 → Network → 复制 Cookie",
            url
        ),
        Lang::En => format!(
            "Cloud share URL requires authentication: {}\nSet KDOCS_COOKIE env var or use --kdocs-cookie flag.\nTo get cookie: login to kdocs.cn → F12 → Network → copy Cookie header",
            url
        ),
    }
}

pub fn share_unsupported_url(url: &str) -> String {
    match current() {
        Lang::Zh => format!(
            "不支持的远程链接: {}\n支持: kdocs.cn / *.kdocs.cn 分享链接 (/l/...)。\n如使用企业版域名，请设置 SHARE_HOSTS 环境变量 (逗号分隔)。",
            url
        ),
        Lang::En => format!(
            "Unsupported remote URL: {}\nSupported: kdocs.cn / *.kdocs.cn share links (/l/...).\nFor enterprise domains, set SHARE_HOSTS env var (comma-separated).",
            url
        ),
    }
}

pub fn share_auth_failed() -> String {
    match current() {
        Lang::Zh => "认证失败: 会话已过期或权限不足，请重新登录 kdocs.cn 并更新 Cookie。".to_string(),
        Lang::En => "Authentication failed: session expired or insufficient permissions. Re-login and update cookie.".to_string(),
    }
}
