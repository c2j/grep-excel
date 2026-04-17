use crate::database::{Database, FileInfo, SearchMode, SearchQuery, SearchResult, SearchStats};
use crate::event::{AppEvent, EventReceiver, EventSender};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, TableState,
    },
    Frame, Terminal,
};
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppMode {
    Normal,
    EditingSearch,
    EditingColumn,
    SelectFile,
    Help,
    DetailPanel,
}

pub struct App {
    running: bool,
    mode: AppMode,
    search_input: Input,
    column_input: Input,
    search_mode: SearchMode,
    results: Vec<SearchResult>,
    results_by_sheet: HashMap<String, Vec<SearchResult>>,
    stats: Option<SearchStats>,
    table_state: TableState,
    tab_state: usize,
    col_offset: usize,
    scroll_state: ScrollbarState,
    status_message: String,
    file_list: Vec<FileInfo>,
    file_list_state: ratatui::widgets::ListState,
    loading: bool,
    error_message: Option<String>,
    database: Arc<Mutex<Database>>,
    event_tx: EventSender,
    event_rx: EventReceiver,
    tick_count: usize,
    detail_scroll: usize,
    visible_col_count: usize,
    result_limit: usize,
}

impl App {
    pub fn new(database: Database, event_tx: EventSender, event_rx: EventReceiver) -> Self {
        let database = Arc::new(Mutex::new(database));
        let initial_files = database.lock().map(|d| d.list_files()).unwrap_or_default();
        let file_count = initial_files.len();
        let status = if file_count > 0 {
            format!(
                "已加载 {} 个文件。按 'o' 查看，'/' 搜索，'?' 帮助",
                file_count
            )
        } else {
            "按 'o' 打开文件，'/' 搜索，'?' 帮助".to_string()
        };

        App {
            running: true,
            mode: AppMode::Normal,
            search_input: Input::default(),
            column_input: Input::default(),
            search_mode: SearchMode::FullText,
            results: Vec::new(),
            results_by_sheet: HashMap::new(),
            stats: None,
            table_state: TableState::default(),
            tab_state: 0,
            col_offset: 0,
            scroll_state: ScrollbarState::default(),
            status_message: status,
            file_list: initial_files,
            file_list_state: ratatui::widgets::ListState::default(),
            loading: false,
            error_message: None,
            database,
            event_tx,
            event_rx,
            tick_count: 0,
            detail_scroll: 0,
            visible_col_count: 0,
            result_limit: 5000,
        }
    }

    pub fn set_search_query(&mut self, query: String) {
        self.search_input = Input::new(query);
    }

    pub fn set_column_filter(&mut self, column: String) {
        self.column_input = Input::new(column);
    }

    pub fn set_search_mode(&mut self, mode: SearchMode) {
        self.search_mode = mode;
    }

    pub fn import_file(&mut self, path: PathBuf) {
        self.loading = true;
        self.status_message = format!("导入中: {:?}...", path);
        let db = Arc::clone(&self.database);
        let tx = self.event_tx.clone();
        let path_clone = path.clone();

        std::thread::spawn(move || {
            let result = match db.lock() {
                Ok(mut db_guard) => db_guard.import_excel(&path_clone, |current, total| {
                    let _ = tx.send(AppEvent::Progress(current, total));
                }),
                Err(e) => Err(anyhow::anyhow!("Lock error: {}", e)),
            };
            let _ = tx.send(AppEvent::FileImported(result));
        });
    }

    #[cfg(feature = "file-dialog")]
    fn open_file_dialog(&mut self) {
        use crate::app::restore_terminal;

        let _ = restore_terminal();

        let files = rfd::FileDialog::new()
            .add_filter(
                "Spreadsheet Files",
                &["xlsx", "xls", "xlsm", "xlsb", "ods", "csv"],
            )
            .set_title("Open Excel Files")
            .pick_files();

        let _ = init_terminal();

        match files {
            Some(paths) if !paths.is_empty() => {
                for path in paths {
                    self.import_file(path);
                }
            }
            _ => {}
        }
    }

    pub fn execute_search(&mut self) {
        if self.search_input.value().is_empty() {
            return;
        }

        self.loading = true;
        self.status_message = "搜索中...".to_string();
        self.error_message = None;
        self.detail_scroll = 0;

        let query = SearchQuery {
            text: self.search_input.value().to_string(),
            column: if self.column_input.value().is_empty() {
                None
            } else {
                Some(self.column_input.value().to_string())
            },
            mode: self.search_mode,
            limit: self.result_limit,
        };

        let db = Arc::clone(&self.database);
        let tx = self.event_tx.clone();

        std::thread::spawn(move || {
            let result = match db.lock() {
                Ok(db_guard) => db_guard.search(&query),
                Err(e) => Err(anyhow::anyhow!("Lock error: {}", e)),
            };
            let _ = tx.send(AppEvent::SearchCompleted(result));
        });
    }

    fn export_results(&mut self) {
        if self.results.is_empty() {
            self.status_message = "无搜索结果可导出".to_string();
            return;
        }

        #[cfg(feature = "file-dialog")]
        {
            use crate::app::restore_terminal;

            let _ = restore_terminal();
            let file = rfd::FileDialog::new()
                .add_filter("CSV Files", &["csv"])
                .set_title("导出搜索结果")
                .save_file();
            let _ = init_terminal();

            if let Some(path) = file {
                match crate::database::export_results_csv(&self.results, &path) {
                    Ok(()) => {
                        self.status_message = format!("已导出: {}", path.display());
                    }
                    Err(e) => {
                        self.error_message = Some(format!("导出失败: {}", e));
                        self.status_message = "导出失败".to_string();
                    }
                }
            }
        }

        #[cfg(not(feature = "file-dialog"))]
        {
            self.status_message = "需要 file-dialog 功能才能导出".to_string();
        }
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Tick => {
                self.tick_count += 1;
            }
            AppEvent::Key(key) => {
                if key.kind == KeyEventKind::Press {
                    self.handle_key_event(key);
                }
            }
            AppEvent::FileImported(result) => {
                self.loading = false;
                match result {
                    Ok(file_info) => {
                        self.file_list.push(file_info.clone());
                        self.status_message = format!("已导入: {}", file_info.name);
                        self.error_message = None;
                    }
                    Err(e) => {
                        self.error_message = Some(format!("导入错误: {}", e));
                        self.status_message = "导入失败".to_string();
                    }
                }
            }
            AppEvent::SearchCompleted(result) => {
                self.loading = false;
                match result {
                    Ok((results, stats)) => {
                        self.results = results.clone();
                        self.stats = Some(stats.clone());

                        self.results_by_sheet.clear();
                        for result in results {
                            self.results_by_sheet
                                .entry(result.sheet_name.clone())
                                .or_insert_with(Vec::new)
                                .push(result);
                        }

                        self.tab_state = 0;
                        self.col_offset = 0;
                        self.table_state.select(Some(0));
                        self.update_scroll_state();

                        self.status_message = if stats.truncated {
                            format!(
                                "找到 {}+ 个匹配 (显示前 {})，用时 {:.2}s — [n] 加载更多",
                                stats.total_matches,
                                self.result_limit,
                                stats.search_duration.as_secs_f64()
                            )
                        } else {
                            format!(
                                "找到 {} 个匹配，用时 {:.2}s",
                                stats.total_matches,
                                stats.search_duration.as_secs_f64()
                            )
                        };
                        self.error_message = None;
                    }
                    Err(e) => {
                        self.error_message = Some(format!("搜索错误: {}", e));
                        self.status_message = "搜索失败".to_string();
                    }
                }
            }
            AppEvent::Progress(current, total) => {
                self.status_message = format!("进度: {}/{}", current, total);
            }
        }
    }

    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
        match self.mode {
            AppMode::Normal => self.handle_normal_mode(key),
            AppMode::EditingSearch => self.handle_search_edit_mode(key),
            AppMode::EditingColumn => self.handle_column_edit_mode(key),
            AppMode::SelectFile => self.handle_select_file_mode(key),
            AppMode::Help => self.handle_help_mode(key),
            AppMode::DetailPanel => self.handle_detail_panel_mode(key),
        }
    }

    fn handle_normal_mode(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('c')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.running = false;
            }
            KeyCode::Char('q') => {
                self.running = false;
            }
            KeyCode::Char('/') | KeyCode::Char('e') => {
                self.mode = AppMode::EditingSearch;
            }
            KeyCode::Char('c') => {
                self.mode = AppMode::EditingColumn;
            }
            KeyCode::Tab => {
                self.search_mode = match self.search_mode {
                    SearchMode::FullText => SearchMode::ExactMatch,
                    SearchMode::ExactMatch => SearchMode::Wildcard,
                    SearchMode::Wildcard => SearchMode::Regex,
                    SearchMode::Regex => SearchMode::FullText,
                };
                let mode_str = match self.search_mode {
                    SearchMode::FullText => "全文",
                    SearchMode::ExactMatch => "精确",
                    SearchMode::Wildcard => "通配符",
                    SearchMode::Regex => "正则",
                };
                self.status_message = format!("搜索模式: {}", mode_str);
            }
            KeyCode::Enter => {
                if self.results.is_empty() || self.loading {
                    self.execute_search();
                } else {
                    let current_results: Vec<&SearchResult> = self.get_current_results();
                    if !current_results.is_empty() {
                        self.mode = AppMode::DetailPanel;
                        self.detail_scroll = 0;
                    } else {
                        self.execute_search();
                    }
                }
            }
            KeyCode::Char('o') => {
                #[cfg(feature = "file-dialog")]
                {
                    self.open_file_dialog();
                }
                #[cfg(not(feature = "file-dialog"))]
                {
                    if !self.file_list.is_empty() {
                        self.mode = AppMode::SelectFile;
                        self.file_list_state.select(Some(0));
                    } else {
                        self.status_message = "未导入文件。请使用命令行导入文件。".to_string();
                    }
                }
            }
            KeyCode::Char('?') | KeyCode::Char('h') => {
                self.mode = AppMode::Help;
            }
            KeyCode::Char(c @ '1'..='9') => {
                let index = (c as usize) - ('1' as usize);
                self.select_tab(index);
            }
            KeyCode::Left => {
                if self.col_offset > 0 {
                    self.col_offset -= 1;
                }
            }
            KeyCode::Right => {
                let col_count = self.get_current_col_count();
                if self.col_offset < col_count.saturating_sub(1) {
                    self.col_offset += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.navigate_table(-1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.navigate_table(1);
            }
            KeyCode::Char('g') => {
                self.table_state.select(Some(0));
                self.scroll_state = self.scroll_state.position(0);
            }
            KeyCode::Char('G') => {
                let results = self.get_current_results();
                let last = results.len().saturating_sub(1);
                self.table_state.select(Some(last));
                self.scroll_state = self.scroll_state.position(last);
            }
            KeyCode::Char('H') => {
                if self.col_offset > 0 {
                    self.col_offset -= 1;
                }
            }
            KeyCode::Char('L') => {
                let col_count = self.get_current_col_count();
                if self.col_offset < col_count.saturating_sub(1) {
                    self.col_offset += 1;
                }
            }
            KeyCode::Char('d') => {
                if let Ok(mut db) = self.database.lock() {
                    if db.clear().is_ok() {
                        self.file_list.clear();
                        self.results.clear();
                        self.results_by_sheet.clear();
                        self.stats = None;
                        self.tab_state = 0;
                        self.col_offset = 0;
                        self.table_state.select(Some(0));
                        self.detail_scroll = 0;
                        self.result_limit = 5000;
                        self.status_message = "已清除所有数据".to_string();
                    }
                }
            }
            KeyCode::Char('n') => {
                if self.stats.as_ref().map_or(false, |s| s.truncated) {
                    self.result_limit = self.result_limit.saturating_add(5000);
                    self.execute_search();
                }
            }
            KeyCode::Char('s') => {
                self.export_results();
            }
            _ => {}
        }
    }

    fn handle_search_edit_mode(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Enter => {
                self.execute_search();
                self.mode = AppMode::Normal;
            }
            KeyCode::Esc => {
                self.mode = AppMode::Normal;
            }
            _ => {
                self.search_input
                    .handle_event(&crossterm::event::Event::Key(key));
            }
        }
    }

    fn handle_column_edit_mode(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Enter => {
                self.mode = AppMode::Normal;
            }
            KeyCode::Esc => {
                self.column_input = Input::default();
                self.mode = AppMode::Normal;
            }
            _ => {
                self.column_input
                    .handle_event(&crossterm::event::Event::Key(key));
            }
        }
    }

    fn handle_select_file_mode(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let current = self.file_list_state.selected().unwrap_or(0);
                if current > 0 {
                    self.file_list_state.select(Some(current - 1));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let current = self.file_list_state.selected().unwrap_or(0);
                if current < self.file_list.len().saturating_sub(1) {
                    self.file_list_state.select(Some(current + 1));
                }
            }
            KeyCode::Enter => {
                self.mode = AppMode::Normal;
            }
            KeyCode::Esc => {
                self.mode = AppMode::Normal;
            }
            _ => {}
        }
    }

    fn handle_help_mode(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        if let KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Char('h') =
            key.code
        {
            self.mode = AppMode::Normal;
        }
    }

    fn handle_detail_panel_mode(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Enter | KeyCode::Esc => {
                self.mode = AppMode::Normal;
                self.detail_scroll = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.detail_scroll = self.detail_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.detail_scroll += 1;
            }
            _ => {}
        }
    }

    fn select_tab(&mut self, index: usize) {
        let max_tabs = self.get_tab_count();
        if index < max_tabs {
            self.tab_state = index;
            self.col_offset = 0;
            self.table_state.select(Some(0));
            self.update_scroll_state();
            self.detail_scroll = 0;
        }
    }

    fn get_tab_count(&self) -> usize {
        self.results_by_sheet.len() + 1
    }

    fn get_current_results(&self) -> Vec<&SearchResult> {
        if self.tab_state == 0 {
            self.results.iter().collect()
        } else {
            let sheet_names: Vec<_> = self.results_by_sheet.keys().cloned().collect();
            let mut sorted_sheets = sheet_names;
            sorted_sheets.sort();

            if let Some(sheet_name) = sorted_sheets.get(self.tab_state - 1) {
                self.results_by_sheet
                    .get(sheet_name)
                    .map(|v| v.iter().collect())
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        }
    }

    fn navigate_table(&mut self, direction: i32) {
        let results = self.get_current_results();
        let current = self.table_state.selected().unwrap_or(0);
        let max = results.len().saturating_sub(1);

        let new_index = if direction > 0 {
            (current + 1).min(max)
        } else {
            current.saturating_sub(1)
        };

        self.table_state.select(Some(new_index));
        self.scroll_state = self.scroll_state.position(new_index);
    }

    fn update_scroll_state(&mut self) {
        let results = self.get_current_results();
        let total_rows = results.len();
        self.scroll_state = ScrollbarState::new(total_rows);
    }

    fn get_current_col_count(&self) -> usize {
        let results = self.get_current_results();
        results
            .iter()
            .find(|r| !r.col_names.is_empty())
            .map(|r| r.col_names.len())
            .unwrap_or(0)
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([
                Constraint::Length(1), // Title bar
                Constraint::Length(1), // Tabs
                Constraint::Length(3), // Search bar
                Constraint::Min(5),    // Results
                Constraint::Length(2), // Status bar
            ])
            .split(frame.area());

        self.draw_title_bar(frame, chunks[0]);
        self.draw_tabs(frame, chunks[1]);
        self.draw_search_bar(frame, chunks[2]);
        self.draw_results_table(frame, chunks[3]);
        self.draw_status_bar(frame, chunks[4]);

        if self.mode == AppMode::Help {
            self.draw_help_popup(frame);
        }

        if self.mode == AppMode::SelectFile {
            self.draw_file_list_popup(frame);
        }
    }

    fn draw_title_bar(&self, frame: &mut Frame, area: Rect) {
        let (mode_text, mode_color) = match self.mode {
            AppMode::Normal => ("普通", Color::Green),
            AppMode::EditingSearch => ("搜索", Color::Yellow),
            AppMode::EditingColumn => ("列筛选", Color::Yellow),
            AppMode::Help => ("帮助", Color::Blue),
            AppMode::SelectFile => ("文件", Color::Magenta),
            AppMode::DetailPanel => ("详情", Color::Magenta),
        };

        let file_count = self.file_list.len();
        let file_text = if file_count == 1 {
            "1 个文件已加载".to_string()
        } else {
            format!("{} 个文件已加载", file_count)
        };

        let spans = vec![
            Span::styled(
                " grep-excel",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("[{}]", mode_text), Style::default().fg(mode_color)),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(file_text, Style::default().fg(Color::DarkGray)),
        ];

        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);
    }

    fn draw_tabs(&mut self, frame: &mut Frame, area: Rect) {
        let all_count = self.results.len();
        let mut tab_titles = vec![format!("全部({})", all_count)];

        let mut sheet_names: Vec<_> = self.results_by_sheet.keys().cloned().collect();
        sheet_names.sort();

        for sheet_name in &sheet_names {
            let count = self
                .results_by_sheet
                .get(sheet_name)
                .map(|v| v.len())
                .unwrap_or(0);
            tab_titles.push(format!("{}({})", sheet_name, count));
        }

        let mut spans: Vec<Span> = Vec::new();
        for (i, title) in tab_titles.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            }
            if i == self.tab_state {
                spans.push(Span::styled(
                    format!(" ▶{} ", title),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    format!(" {} ", title),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }

        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);
    }

    fn draw_search_bar(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(9),
                Constraint::Min(20),
                Constraint::Length(3),
                Constraint::Length(10),
                Constraint::Length(12),
                Constraint::Length(3),
                Constraint::Length(11),
            ])
            .split(area);

        let label_area = Rect {
            x: chunks[0].x,
            y: chunks[0].y + 1,
            width: chunks[0].width,
            height: 1,
        };
        let search_label = Paragraph::new("[搜索]")
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Center);
        frame.render_widget(search_label, label_area);

        let search_border_color = if self.mode == AppMode::EditingSearch {
            Color::Yellow
        } else {
            Color::DarkGray
        };
        let scroll = self.search_input.visual_scroll(chunks[1].width as usize);
        let search_paragraph = Paragraph::new(self.search_input.value())
            .style(Style::default().fg(Color::White))
            .scroll((0, scroll as u16))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(search_border_color)),
            );
        frame.render_widget(search_paragraph, chunks[1]);

        if self.mode == AppMode::EditingSearch {
            let cursor_pos = self.search_input.visual_cursor();
            let cursor_x = chunks[1].x + (cursor_pos.saturating_sub(scroll)) as u16 + 1;
            let cursor_y = chunks[1].y + 1;
            frame.set_cursor_position((cursor_x, cursor_y));
        }

        let sep_area = Rect {
            x: chunks[2].x,
            y: chunks[2].y + 1,
            width: chunks[2].width,
            height: 1,
        };
        let sep = Paragraph::new("│")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(sep, sep_area);

        let col_label_area = Rect {
            x: chunks[3].x,
            y: chunks[3].y + 1,
            width: chunks[3].width,
            height: 1,
        };
        let column_label = Paragraph::new("[列]")
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Center);
        frame.render_widget(column_label, col_label_area);

        let column_border_color = if self.mode == AppMode::EditingColumn {
            Color::Yellow
        } else {
            Color::DarkGray
        };
        let col_scroll = self.column_input.visual_scroll(chunks[4].width as usize);
        let column_paragraph = Paragraph::new(self.column_input.value())
            .style(Style::default().fg(Color::White))
            .scroll((0, col_scroll as u16))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(column_border_color)),
            );
        frame.render_widget(column_paragraph, chunks[4]);

        if self.mode == AppMode::EditingColumn {
            let cursor_pos = self.column_input.visual_cursor();
            let cursor_x = chunks[4].x + (cursor_pos.saturating_sub(col_scroll)) as u16 + 1;
            let cursor_y = chunks[4].y + 1;
            frame.set_cursor_position((cursor_x, cursor_y));
        }

        let sep2_area = Rect {
            x: chunks[5].x,
            y: chunks[5].y + 1,
            width: chunks[5].width,
            height: 1,
        };
        let sep2 = Paragraph::new("│")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(sep2, sep2_area);

        let (mode_str, mode_color) = match self.search_mode {
            SearchMode::FullText => ("全文", Color::Green),
            SearchMode::ExactMatch => ("精确", Color::Yellow),
            SearchMode::Wildcard => ("通配符", Color::Magenta),
            SearchMode::Regex => ("正则", Color::Red),
        };
        let mode_area = Rect {
            x: chunks[6].x,
            y: chunks[6].y + 1,
            width: chunks[6].width,
            height: 1,
        };
        let mode_paragraph = Paragraph::new(mode_str)
            .style(Style::default().fg(mode_color).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
        frame.render_widget(mode_paragraph, mode_area);
    }

    fn draw_results_table(&mut self, frame: &mut Frame, area: Rect) {
        if self.mode == AppMode::DetailPanel {
            let current: Vec<SearchResult> =
                self.get_current_results().into_iter().cloned().collect();
            let selected = self.table_state.selected().unwrap_or(0);
            if let Some(result) = current.get(selected).cloned() {
                frame.render_widget(Clear, area);
                self.draw_detail_panel(frame, area, &result);
            }
            return;
        }

        let results: Vec<SearchResult> = self.get_current_results().into_iter().cloned().collect();

        if results.is_empty() {
            let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

            let content = if self.loading {
                let char_idx = self.tick_count % spinner_chars.len();
                vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("{} 加载中...", spinner_chars[char_idx]),
                        Style::default().fg(Color::Cyan),
                    )),
                ]
            } else if self.results.is_empty() && self.search_input.value().is_empty() {
                if self.file_list.is_empty() {
                    vec![
                        Line::from(""),
                        Line::from(Span::styled("未加载文件", Style::default().fg(Color::Gray))),
                        Line::from(""),
                        Line::from(Span::styled(
                            "按 [o] 打开 Excel 文件",
                            Style::default().fg(Color::DarkGray),
                        )),
                        Line::from(Span::styled(
                            "按 [?] 查看帮助",
                            Style::default().fg(Color::DarkGray),
                        )),
                    ]
                } else {
                    let mut lines = vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            "已加载文件",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        )),
                        Line::from(Span::styled(
                            "─────────────────────────────────────────",
                            Style::default().fg(Color::DarkGray),
                        )),
                    ];

                    for file in &self.file_list {
                        let sheets_word = if file.sheets.len() == 1 {
                            "个工作表"
                        } else {
                            "个工作表"
                        };
                        lines.push(Line::from(vec![
                            Span::styled("  ", Style::default()),
                            Span::styled(
                                file.name.clone(),
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                format!(
                                    "  {}{} · {} 行",
                                    file.sheets.len(),
                                    sheets_word,
                                    file.total_rows
                                ),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));

                        for (i, (sheet_name, row_count)) in file.sheets.iter().enumerate() {
                            let prefix = if i == file.sheets.len() - 1 {
                                "    └── "
                            } else {
                                "    ├── "
                            };
                            lines.push(Line::from(vec![
                                Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                                Span::styled(sheet_name.clone(), Style::default().fg(Color::Gray)),
                                Span::styled(
                                    format!("  · {} 行", row_count),
                                    Style::default().fg(Color::DarkGray),
                                ),
                            ]));
                        }
                    }

                    lines.push(Line::from(""));

                    if let Some(sample) = self.file_list.iter().find_map(|f| f.sample.as_ref()) {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  预览: {}", sample.sheet_name),
                            Style::default().fg(Color::Cyan),
                        )]));
                        lines.extend(format_sample_preview(sample, area.width));
                        lines.push(Line::from(""));
                    }

                    lines.push(Line::from(vec![
                        Span::styled("  按 ", Style::default().fg(Color::DarkGray)),
                        Span::styled("[/]", Style::default().fg(Color::Cyan)),
                        Span::styled(" 搜索  ·  ", Style::default().fg(Color::DarkGray)),
                        Span::styled("[?]", Style::default().fg(Color::Cyan)),
                        Span::styled(" 帮助", Style::default().fg(Color::DarkGray)),
                    ]));

                    lines
                }
            } else {
                let query = self.search_input.value();
                if self.mode == AppMode::EditingSearch {
                    vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            format!("  查询: \"{}\"", query),
                            Style::default().fg(Color::White),
                        )),
                        Line::from(Span::styled(
                            "  按 [Enter] 执行搜索",
                            Style::default().fg(Color::DarkGray),
                        )),
                    ]
                } else if self.stats.is_none() {
                    vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            format!("  查询: \"{}\"", query),
                            Style::default().fg(Color::White),
                        )),
                        Line::from(Span::styled(
                            "  按 [/] 编辑  ·  [Enter] 搜索",
                            Style::default().fg(Color::DarkGray),
                        )),
                    ]
                } else {
                    let msg = if query.is_empty() {
                        "未找到结果".to_string()
                    } else {
                        format!("未找到匹配项: \"{}\"", query)
                    };
                    vec![
                        Line::from(""),
                        Line::from(Span::styled(msg, Style::default().fg(Color::Gray))),
                    ]
                }
            };

            let alignment = if self.file_list.is_empty()
                && self.results.is_empty()
                && self.search_input.value().is_empty()
                && !self.loading
            {
                Alignment::Center
            } else {
                Alignment::Left
            };

            let paragraph = Paragraph::new(content)
                .alignment(alignment)
                .block(Block::default().borders(Borders::ALL));
            frame.render_widget(paragraph, area);
            return;
        }

        let col_names: Vec<String> = results
            .iter()
            .find(|r| !r.col_names.is_empty())
            .map(|r| r.col_names.clone())
            .unwrap_or_else(|| {
                let max_cols = results.iter().map(|r| r.row.len()).max().unwrap_or(0);
                (0..max_cols).map(|i| format!("列{}", i + 1)).collect()
            });

        let total_cols = col_names.len();

        let col_widths: Vec<u16> = if let Some(r) = results.first() {
            if r.col_widths.is_empty() {
                let computed = compute_col_widths(&col_names, &results, 0, total_cols, usize::MAX);
                computed.into_iter().map(|w| w as u16).collect()
            } else {
                r.col_widths
                    .iter()
                    .map(|&w| {
                        let chars = w.round().max(4.0).min(50.0) as u16;
                        chars + 2
                    })
                    .collect()
            }
        } else {
            vec![10; total_cols]
        };

        let fixed_width: u16 = 15 + 12;
        let available_width = area.width.saturating_sub(fixed_width + 4);

        let mut visible_count = 0usize;
        let mut used_width: u16 = 0;
        for &w in col_widths.iter().skip(self.col_offset) {
            if used_width + w > available_width {
                break;
            }
            used_width += w;
            visible_count += 1;
        }
        visible_count = visible_count.max(1);
        self.visible_col_count = visible_count;

        let col_offset = self.col_offset;
        let visible_col_names: Vec<String> = col_names
            .iter()
            .skip(col_offset)
            .take(visible_count)
            .cloned()
            .collect();
        let visible_col_widths: Vec<u16> = col_widths
            .iter()
            .skip(col_offset)
            .take(visible_count)
            .copied()
            .collect();

        let mut header_cells: Vec<ratatui::widgets::Cell<'_>> = vec![
            ratatui::widgets::Cell::from("文件").style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            ratatui::widgets::Cell::from("工作表").style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        for name in &visible_col_names {
            header_cells.push(
                ratatui::widgets::Cell::from(name.as_str()).style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            );
        }
        let header_row = ratatui::widgets::Row::new(header_cells)
            .height(1)
            .bottom_margin(1);

        let rows: Vec<_> = results
            .iter()
            .map(|result| {
                let mut cells: Vec<ratatui::widgets::Cell<'_>> = vec![
                    ratatui::widgets::Cell::from(truncate_str(&result.file_name, 15))
                        .style(Style::default().fg(Color::DarkGray)),
                    ratatui::widgets::Cell::from(truncate_str(&result.sheet_name, 12))
                        .style(Style::default().fg(Color::DarkGray)),
                ];
                for (col_idx, cell_value) in result.row.iter().enumerate() {
                    if col_idx < col_offset {
                        continue;
                    }
                    if col_idx >= col_offset + visible_count {
                        break;
                    }
                    let is_matched = result.matched_columns.contains(&col_idx);
                    let cell_style = if is_matched {
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    cells.push(ratatui::widgets::Cell::from(cell_value.as_str()).style(cell_style));
                }
                ratatui::widgets::Row::new(cells).height(1)
            })
            .collect();

        let mut constraints = vec![Constraint::Length(15), Constraint::Length(12)];
        for &w in &visible_col_widths {
            constraints.push(Constraint::Length(w));
        }
        if constraints.len() == 2 {
            constraints.push(Constraint::Min(10));
        }

        let mut title_spans = vec![];
        if col_offset > 0 {
            title_spans.push(Span::styled(" ◄ ", Style::default().fg(Color::Yellow)));
        }
        if total_cols > 0 && col_offset + visible_count < total_cols {
            title_spans.push(Span::styled(" ► ", Style::default().fg(Color::Yellow)));
        }
        let table_title = if title_spans.is_empty() {
            String::new()
        } else {
            format!(
                " {} ",
                title_spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            )
        };

        let table = Table::new(rows, constraints)
            .header(header_row)
            .block(Block::default().borders(Borders::ALL).title(Span::styled(
                table_title,
                Style::default().fg(Color::Yellow),
            )))
            .row_highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        frame.render_stateful_widget(table, area, &mut self.table_state);

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = self.scroll_state.clone();
        frame.render_stateful_widget(
            scrollbar,
            area.inner(ratatui::layout::Margin::new(0, 1)),
            &mut scrollbar_state,
        );
    }

    fn draw_detail_panel(&self, frame: &mut Frame, area: Rect, result: &SearchResult) {
        let mut lines: Vec<Line<'_>> = Vec::new();

        lines.push(Line::from(vec![
            Span::styled("文件: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &result.file_name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("工作表: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&result.sheet_name, Style::default().fg(Color::Cyan)),
        ]));
        lines.push(Line::from(Span::styled(
            "─".repeat(area.width.saturating_sub(2) as usize),
            Style::default().fg(Color::DarkGray),
        )));

        let max_name_width = result
            .col_names
            .iter()
            .map(|n| UnicodeWidthStr::width(n.as_str()))
            .max()
            .unwrap_or(0)
            .min(20);

        let prefix_width = max_name_width + 3;
        let value_width = area.width.saturating_sub(prefix_width as u16 + 2) as usize;

        for (i, (name, value)) in result.col_names.iter().zip(result.row.iter()).enumerate() {
            let is_matched = result.matched_columns.contains(&i);

            let name_style = if is_matched {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Yellow)
            };

            let value_style = if is_matched {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let name_display = pad_to_width(name, max_name_width);
            let continuation_indent = " ".repeat(prefix_width);

            let segments: Vec<&str> = if value.contains('\n') {
                value.split('\n').collect()
            } else {
                vec![value.as_str()]
            };

            let mut is_first_line = true;
            for segment in segments {
                if value_width == 0 || UnicodeWidthStr::width(segment) <= value_width {
                    if is_first_line {
                        lines.push(Line::from(vec![
                            Span::styled(format!("{} ", name_display), name_style),
                            Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                            Span::styled(segment.to_string(), value_style),
                        ]));
                        is_first_line = false;
                    } else {
                        lines.push(Line::from(vec![
                            Span::styled(format!("{} ", continuation_indent), Style::default()),
                            Span::styled("  ", Style::default()),
                            Span::styled(segment.to_string(), value_style),
                        ]));
                    }
                } else {
                    let wrapped = unicode_wrap(segment, value_width);
                    for (wi, chunk) in wrapped.iter().enumerate() {
                        if wi == 0 && is_first_line {
                            lines.push(Line::from(vec![
                                Span::styled(format!("{} ", name_display), name_style),
                                Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                                Span::styled(chunk.clone(), value_style),
                            ]));
                            is_first_line = false;
                        } else {
                            lines.push(Line::from(vec![
                                Span::styled(format!("{} ", continuation_indent), Style::default()),
                                Span::styled("  ", Style::default()),
                                Span::styled(chunk.clone(), value_style),
                            ]));
                        }
                    }
                }
            }

            if is_first_line {
                lines.push(Line::from(vec![
                    Span::styled(format!("{} ", name_display), name_style),
                    Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

        let visible_height = area.height.saturating_sub(2) as usize;
        let total_lines = lines.len();
        let max_scroll = total_lines.saturating_sub(visible_height);
        let scroll = self.detail_scroll.min(max_scroll);
        let visible_lines: Vec<Line<'_>> = lines
            .into_iter()
            .skip(scroll)
            .take(visible_height)
            .collect();

        let detail_block = Paragraph::new(visible_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(Span::styled(
                    " 行详情 ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .title_alignment(Alignment::Center),
        );

        frame.render_widget(detail_block, area);
    }

    fn draw_status_bar(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);

        let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

        let stats_line = if let Some(err) = &self.error_message {
            Line::from(Span::styled(
                format!(" {}", err),
                Style::default().fg(Color::Red),
            ))
        } else if self.loading {
            let char_idx = self.tick_count % spinner_chars.len();
            Line::from(Span::styled(
                format!(" {} 加载中...", spinner_chars[char_idx]),
                Style::default().fg(Color::Cyan),
            ))
        } else if let Some(stats) = &self.stats {
            let current_results = self.get_current_results();
            let selected = self.table_state.selected().unwrap_or(0);
            let row_indicator = if current_results.is_empty() {
                "行 0/0".to_string()
            } else {
                format!("行 {}/{}", selected + 1, current_results.len())
            };

            let mut sheet_stats: Vec<_> = stats.matches_per_sheet.iter().collect();
            sheet_stats.sort_by_key(|(k, _)| *k);
            let sheet_info: Vec<_> = sheet_stats
                .iter()
                .map(|(k, v)| format!("{}({})", k, v))
                .collect();

            let total_cols = self.get_current_col_count();
            let col_start = self.col_offset + 1;
            let col_end = total_cols.min(self.col_offset + self.visible_col_count.max(1));
            let col_indicator = if total_cols == 0 {
                String::new()
            } else {
                format!(" │ 列 {}-{}/{}", col_start, col_end, total_cols)
            };

            Line::from(vec![
                Span::styled(
                    format!(
                        " 匹配: {}/{}",
                        stats.total_matches, stats.total_rows_searched
                    ),
                    Style::default().fg(Color::White),
                ),
                Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:.2}s", stats.search_duration.as_secs_f64()),
                    Style::default().fg(Color::White),
                ),
                Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
                Span::styled(row_indicator, Style::default().fg(Color::White)),
                Span::styled(col_indicator, Style::default().fg(Color::DarkGray)),
                Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
                Span::styled(sheet_info.join(" "), Style::default().fg(Color::DarkGray)),
            ])
        } else {
            let file_count = self.file_list.len();
            let file_text = if file_count == 1 {
                "1 个文件已加载".to_string()
            } else {
                format!("{} 个文件已加载", file_count)
            };
            Line::from(Span::styled(
                format!(" {}", file_text),
                Style::default().fg(Color::DarkGray),
            ))
        };

        let stats_paragraph = Paragraph::new(stats_line);
        frame.render_widget(stats_paragraph, chunks[0]);

        let hints: Vec<Span> = match self.mode {
            AppMode::Normal => vec![
                Span::styled(" [/]", Style::default().fg(Color::Cyan)),
                Span::raw("搜索  "),
                Span::styled("[c]", Style::default().fg(Color::Cyan)),
                Span::raw("列  "),
                Span::styled("[Tab]", Style::default().fg(Color::Cyan)),
                Span::raw("模式  "),
                Span::styled("[o]", Style::default().fg(Color::Cyan)),
                Span::raw("打开  "),
                Span::styled("[s]", Style::default().fg(Color::Cyan)),
                Span::raw("导出  "),
                Span::styled("[d]", Style::default().fg(Color::Cyan)),
                Span::raw("清除  "),
                Span::styled("[?]", Style::default().fg(Color::Cyan)),
                Span::raw("帮助  "),
                Span::styled("[q]", Style::default().fg(Color::Cyan)),
                Span::raw("退出"),
            ],
            AppMode::EditingSearch => vec![
                Span::styled(" [Enter]", Style::default().fg(Color::Cyan)),
                Span::raw("执行  "),
                Span::styled("[Esc]", Style::default().fg(Color::Cyan)),
                Span::raw("取消  "),
                Span::styled("[Tab]", Style::default().fg(Color::Cyan)),
                Span::raw("切换模式"),
            ],
            AppMode::EditingColumn => vec![
                Span::styled(" [Enter]", Style::default().fg(Color::Cyan)),
                Span::raw("确认  "),
                Span::styled("[Esc]", Style::default().fg(Color::Cyan)),
                Span::raw("取消"),
            ],
            AppMode::Help => vec![
                Span::styled(" [Esc/?/h]", Style::default().fg(Color::Cyan)),
                Span::raw("关闭帮助"),
            ],
            AppMode::SelectFile => vec![
                Span::styled(" [↑/k]", Style::default().fg(Color::Cyan)),
                Span::raw("上  "),
                Span::styled("[↓/j]", Style::default().fg(Color::Cyan)),
                Span::raw("下  "),
                Span::styled("[Enter]", Style::default().fg(Color::Cyan)),
                Span::raw("选择  "),
                Span::styled("[Esc]", Style::default().fg(Color::Cyan)),
                Span::raw("关闭"),
            ],
            AppMode::DetailPanel => vec![
                Span::styled(" [Enter/Esc]", Style::default().fg(Color::Cyan)),
                Span::raw("关闭  "),
                Span::styled("[↑/k]", Style::default().fg(Color::Cyan)),
                Span::raw("上滚  "),
                Span::styled("[↓/j]", Style::default().fg(Color::Cyan)),
                Span::raw("下滚"),
            ],
        };

        let hints_paragraph = Paragraph::new(Line::from(hints));
        frame.render_widget(hints_paragraph, chunks[1]);
    }

    fn draw_help_popup(&mut self, frame: &mut Frame) {
        let area = centered_rect(55, 75, frame.area());

        let group_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        let key_style = Style::default().fg(Color::Yellow);
        let desc_style = Style::default().fg(Color::White);
        let sep_style = Style::default().fg(Color::DarkGray);
        let footer_style = Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC);

        let help_text = vec![
            Line::from(""),
            Line::from(Span::styled("  导航", group_style)),
            Line::from(vec![
                Span::styled("    ↑/k   ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("上移一行", desc_style),
            ]),
            Line::from(vec![
                Span::styled("    ↓/j   ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("下移一行", desc_style),
            ]),
            Line::from(vec![
                Span::styled("    g     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("跳至首行", desc_style),
            ]),
            Line::from(vec![
                Span::styled("    G     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("跳至末行", desc_style),
            ]),
            Line::from(vec![
                Span::styled("    ←/→   ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("左右滚动列", desc_style),
            ]),
            Line::from(vec![
                Span::styled("    1-9   ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("跳转至指定标签", desc_style),
            ]),
            Line::from(""),
            Line::from(Span::styled("  搜索", group_style)),
            Line::from(vec![
                Span::styled("    /     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("输入搜索词", desc_style),
            ]),
            Line::from(vec![
                Span::styled("    c     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("列筛选", desc_style),
            ]),
            Line::from(vec![
                Span::styled("    Tab   ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("切换搜索模式", desc_style),
            ]),
            Line::from(vec![
                Span::styled("    Enter ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("执行搜索", desc_style),
            ]),
            Line::from(""),
            Line::from(Span::styled("  通用", group_style)),
            Line::from(vec![
                Span::styled("    o     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("打开文件", desc_style),
            ]),
            Line::from(vec![
                Span::styled("    d     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("清除所有数据", desc_style),
            ]),
            Line::from(vec![
                Span::styled("    s     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("导出搜索结果为CSV", desc_style),
            ]),
            Line::from(vec![
                Span::styled("    n     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("加载更多结果", desc_style),
            ]),
            Line::from(vec![
                Span::styled("    ?     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("开关帮助", desc_style),
            ]),
            Line::from(vec![
                Span::styled("    q     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled("退出", desc_style),
            ]),
            Line::from(""),
            Line::from(Span::styled("  按 Esc、?、h 或 q 关闭", footer_style)),
        ];

        let help_block = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" 帮助 "),
            )
            .alignment(Alignment::Left);

        frame.render_widget(Clear, area);
        frame.render_widget(help_block, area);
    }

    fn draw_file_list_popup(&mut self, frame: &mut Frame) {
        let area = centered_rect(45, 60, frame.area());

        let selected = self.file_list_state.selected();

        let items: Vec<ListItem> = self
            .file_list
            .iter()
            .enumerate()
            .map(|(i, file)| {
                let is_selected = selected == Some(i);
                let prefix = if is_selected { "  >> " } else { "     " };
                let name_style = if is_selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let meta = format!("     {} 工作表 · {} 行", file.sheets.len(), file.total_rows);

                let lines = vec![
                    Line::from(vec![
                        Span::styled(prefix, name_style),
                        Span::styled(file.name.clone(), name_style),
                    ]),
                    Line::from(Span::styled(meta, Style::default().fg(Color::DarkGray))),
                ];
                ListItem::new(lines)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" 已加载文件 "),
            )
            .highlight_style(Style::default());

        frame.render_widget(Clear, area);
        frame.render_stateful_widget(list, area, &mut self.file_list_state);
    }

    pub fn run(&mut self) -> Result<()> {
        let mut terminal = init_terminal()?;

        let tick_rate = Duration::from_millis(200);
        let tx = self.event_tx.clone();

        std::thread::spawn(move || {
            let mut last_tick = Instant::now();
            loop {
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_else(|| Duration::from_secs(0));

                if event::poll(timeout).unwrap_or(false) {
                    if let Event::Key(key) =
                        event::read().unwrap_or(Event::Key(crossterm::event::KeyEvent::new(
                            crossterm::event::KeyCode::Null,
                            crossterm::event::KeyModifiers::empty(),
                        )))
                    {
                        let _ = tx.send(AppEvent::Key(key));
                    }
                }

                if last_tick.elapsed() >= tick_rate {
                    let _ = tx.send(AppEvent::Tick);
                    last_tick = Instant::now();
                }
            }
        });

        while self.running {
            terminal.draw(|frame| self.draw(frame))?;

            match self.event_rx.recv() {
                Ok(event) => self.handle_event(event),
                Err(_) => break,
            }
        }

        restore_terminal()?;
        Ok(())
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic);
    }));

    Ok(terminal)
}

pub fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len - 1).collect();
        format!("{}\u{2026}", truncated)
    }
}

fn pad_to_width(s: &str, width: usize) -> String {
    let sw = UnicodeWidthStr::width(s);
    if sw >= width {
        truncate_str(s, width)
    } else {
        let mut out = s.to_string();
        for _ in 0..width - sw {
            out.push(' ');
        }
        out
    }
}

fn compute_col_widths(
    col_names: &[String],
    results: &[crate::database::SearchResult],
    col_offset: usize,
    max_cols: usize,
    max_width: usize,
) -> Vec<usize> {
    const MIN_COL_WIDTH: usize = 4;
    const MAX_COL_WIDTH: usize = 50;

    let mut widths = vec![MIN_COL_WIDTH; max_cols];

    for (i, name) in col_names.iter().skip(col_offset).take(max_cols).enumerate() {
        let w = UnicodeWidthStr::width(name.as_str()).clamp(MIN_COL_WIDTH, MAX_COL_WIDTH);
        widths[i] = w;
    }

    for result in results.iter().take(100) {
        for (i, cell_value) in result
            .row
            .iter()
            .enumerate()
            .skip(col_offset)
            .take(max_cols)
        {
            let w = UnicodeWidthStr::width(cell_value.as_str()).clamp(MIN_COL_WIDTH, MAX_COL_WIDTH);
            let local_i = i - col_offset;
            if local_i < widths.len() && w > widths[local_i] {
                widths[local_i] = w;
            }
        }
    }

    for w in &mut widths {
        *w = (*w + 2).min(MAX_COL_WIDTH);
    }

    // Proportionally shrink if total exceeds available width (minus fixed file+sheet columns)
    let fixed_width: usize = 15 + 12;
    let available = max_width.saturating_sub(fixed_width);
    let total: usize = widths.iter().sum();
    if total > available && available > 0 {
        let scale = available as f64 / total as f64;
        for w in &mut widths {
            *w = ((*w as f64 * scale).ceil() as usize).max(MIN_COL_WIDTH);
        }
    }

    widths
}

fn format_sample_preview(
    sample: &crate::database::FileSample,
    max_width: u16,
) -> Vec<Line<'static>> {
    const MAX_ROWS: usize = 3;
    const MAX_COL_WIDTH: usize = 20;
    const INDENT: &str = "  ";
    const SEP: &str = " │ ";

    let usable = (max_width as usize).saturating_sub(6);
    let num_cols = sample.headers.len();
    if num_cols == 0 || usable < 10 {
        return vec![];
    }

    let mut widths = vec![0usize; num_cols];
    for (i, h) in sample.headers.iter().enumerate() {
        widths[i] = UnicodeWidthStr::width(h.as_str()).min(MAX_COL_WIDTH);
    }
    for row in sample.rows.iter().take(MAX_ROWS) {
        for (i, cell) in row.iter().enumerate() {
            if i < num_cols {
                let w = UnicodeWidthStr::width(cell.as_str()).min(MAX_COL_WIDTH);
                widths[i] = widths[i].max(w);
            }
        }
    }

    let indent_w = UnicodeWidthStr::width(INDENT);
    let sep_w = UnicodeWidthStr::width(SEP);
    let mut total = indent_w;
    let mut visible = 0;
    for (i, &w) in widths.iter().enumerate() {
        let added = if i == 0 { w } else { sep_w + w };
        if total + added > usable {
            break;
        }
        total += added;
        visible += 1;
    }
    if visible == 0 {
        return vec![];
    }

    let pad_to = |s: &str, width: usize| -> String {
        let sw = UnicodeWidthStr::width(s);
        if sw >= width {
            truncate_str(s, width)
        } else {
            let mut out = s.to_string();
            for _ in 0..width - sw {
                out.push(' ');
            }
            out
        }
    };

    let mut lines = Vec::new();

    let mut header_spans = vec![Span::styled(INDENT.to_string(), Style::default())];
    for (i, h) in sample.headers.iter().take(visible).enumerate() {
        if i > 0 {
            header_spans.push(Span::styled(SEP, Style::default().fg(Color::DarkGray)));
        }
        header_spans.push(Span::styled(
            pad_to(h, widths[i]),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    }
    lines.push(Line::from(header_spans));

    let sep_len = total - indent_w;
    let sep_str = format!("{}{}", INDENT, "─".repeat(sep_len));
    lines.push(Line::from(Span::styled(
        sep_str,
        Style::default().fg(Color::DarkGray),
    )));

    for row in sample.rows.iter().take(MAX_ROWS) {
        let mut row_spans = vec![Span::styled(INDENT.to_string(), Style::default())];
        for i in 0..visible {
            if i > 0 {
                row_spans.push(Span::styled(SEP, Style::default().fg(Color::DarkGray)));
            }
            let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
            row_spans.push(Span::styled(
                pad_to(cell, widths[i]),
                Style::default().fg(Color::Gray),
            ));
        }
        lines.push(Line::from(row_spans));
    }

    lines
}

fn unicode_wrap(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    let mut result = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;

    for ch in text.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + ch_width > max_width && !current.is_empty() {
            result.push(current.clone());
            current.clear();
            current_width = 0;
        }
        current.push(ch);
        current_width += ch_width;
    }

    if !current.is_empty() {
        result.push(current);
    }

    if result.is_empty() {
        result.push(String::new());
    }

    result
}
