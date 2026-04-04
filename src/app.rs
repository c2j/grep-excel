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
        Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState, Tabs,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppMode {
    Normal,
    EditingSearch,
    EditingColumn,
    SelectFile,
    Help,
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
}

impl App {
    pub fn new(database: Database, event_tx: EventSender, event_rx: EventReceiver) -> Self {
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
            scroll_state: ScrollbarState::default(),
            status_message: "Press 'o' to open a file, '/' to search, '?' for help".to_string(),
            file_list: Vec::new(),
            file_list_state: ratatui::widgets::ListState::default(),
            loading: false,
            error_message: None,
            database: Arc::new(Mutex::new(database)),
            event_tx,
            event_rx,
            tick_count: 0,
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
        self.status_message = format!("Importing {:?}...", path);
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

    pub fn execute_search(&mut self) {
        if self.search_input.value().is_empty() {
            return;
        }

        self.loading = true;
        self.status_message = "Searching...".to_string();
        self.error_message = None;

        let query = SearchQuery {
            text: self.search_input.value().to_string(),
            column: if self.column_input.value().is_empty() {
                None
            } else {
                Some(self.column_input.value().to_string())
            },
            mode: self.search_mode,
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
                        self.status_message = format!("Imported: {}", file_info.name);
                        self.error_message = None;
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Import error: {}", e));
                        self.status_message = "Import failed".to_string();
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
                        self.table_state.select(Some(0));
                        self.update_scroll_state();

                        self.status_message = format!(
                            "Found {} matches in {:.2}s",
                            stats.total_matches,
                            stats.search_duration.as_secs_f64()
                        );
                        self.error_message = None;
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Search error: {}", e));
                        self.status_message = "Search failed".to_string();
                    }
                }
            }
            AppEvent::Progress(current, total) => {
                self.status_message = format!("Progress: {}/{}", current, total);
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
                    SearchMode::Wildcard => SearchMode::FullText,
                };
                let mode_str = match self.search_mode {
                    SearchMode::FullText => "FullText",
                    SearchMode::ExactMatch => "ExactMatch",
                    SearchMode::Wildcard => "Wildcard",
                };
                self.status_message = format!("Search mode: {}", mode_str);
            }
            KeyCode::Enter => {
                self.execute_search();
            }
            KeyCode::Char('o') => {
                if !self.file_list.is_empty() {
                    self.mode = AppMode::SelectFile;
                    self.file_list_state.select(Some(0));
                } else {
                    self.status_message =
                        "No files imported. Use command line to import files.".to_string();
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
                if self.tab_state > 0 {
                    self.tab_state -= 1;
                    self.table_state.select(Some(0));
                    self.update_scroll_state();
                }
            }
            KeyCode::Right => {
                let max_tabs = self.get_tab_count();
                if self.tab_state < max_tabs - 1 {
                    self.tab_state += 1;
                    self.table_state.select(Some(0));
                    self.update_scroll_state();
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
            KeyCode::Char('d') => {
                if let Ok(mut db) = self.database.lock() {
                    if db.clear().is_ok() {
                        self.file_list.clear();
                        self.results.clear();
                        self.results_by_sheet.clear();
                        self.stats = None;
                        self.tab_state = 0;
                        self.table_state.select(Some(0));
                        self.status_message = "Cleared all data".to_string();
                    }
                }
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

    fn select_tab(&mut self, index: usize) {
        let max_tabs = self.get_tab_count();
        if index < max_tabs {
            self.tab_state = index;
            self.table_state.select(Some(0));
            self.update_scroll_state();
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

    pub fn draw(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(2),
            ])
            .split(frame.area());

        self.draw_tabs(frame, chunks[0]);
        self.draw_search_bar(frame, chunks[1]);
        self.draw_results_table(frame, chunks[2]);
        self.draw_status_bar(frame, chunks[3]);

        if self.mode == AppMode::Help {
            self.draw_help_popup(frame);
        }

        if self.mode == AppMode::SelectFile {
            self.draw_file_list_popup(frame);
        }
    }

    fn draw_tabs(&mut self, frame: &mut Frame, area: Rect) {
        let mut tab_titles = vec!["All Results".to_string()];

        let mut sheet_names: Vec<_> = self.results_by_sheet.keys().cloned().collect();
        sheet_names.sort();

        for sheet_name in &sheet_names {
            let count = self
                .results_by_sheet
                .get(sheet_name)
                .map(|v| v.len())
                .unwrap_or(0);
            tab_titles.push(format!("{} ({})", sheet_name, count));
        }

        let tabs = Tabs::new(
            tab_titles
                .iter()
                .map(|t| Span::raw(t.clone()))
                .collect::<Vec<_>>(),
        )
        .select(self.tab_state)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::raw("|"));

        frame.render_widget(tabs, area);
    }

    fn draw_search_bar(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(8),
                Constraint::Min(20),
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Length(8),
                Constraint::Length(10),
            ])
            .split(area);

        let search_label = Paragraph::new("Search:")
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Right);
        frame.render_widget(search_label, chunks[0]);

        let search_style = if self.mode == AppMode::EditingSearch {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        let search_text = self.search_input.value();
        let cursor_pos = self.search_input.visual_cursor();
        let scroll = self.search_input.visual_scroll(chunks[1].width as usize);

        let search_paragraph = Paragraph::new(search_text)
            .style(search_style)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(search_paragraph, chunks[1]);

        if self.mode == AppMode::EditingSearch {
            let cursor_x = chunks[1].x + (cursor_pos.saturating_sub(scroll)) as u16 + 1;
            let cursor_y = chunks[1].y + 1;
            frame.set_cursor_position((cursor_x, cursor_y));
        }

        let column_label = Paragraph::new("Column:")
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Right);
        frame.render_widget(column_label, chunks[2]);

        let column_style = if self.mode == AppMode::EditingColumn {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        let column_paragraph = Paragraph::new(self.column_input.value())
            .style(column_style)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(column_paragraph, chunks[3]);

        if self.mode == AppMode::EditingColumn {
            let cursor_pos = self.column_input.visual_cursor();
            let scroll = self.column_input.visual_scroll(chunks[3].width as usize);
            let cursor_x = chunks[3].x + (cursor_pos.saturating_sub(scroll)) as u16 + 1;
            let cursor_y = chunks[3].y + 1;
            frame.set_cursor_position((cursor_x, cursor_y));
        }

        let mode_label = Paragraph::new("Mode:")
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Right);
        frame.render_widget(mode_label, chunks[4]);

        let mode_str = match self.search_mode {
            SearchMode::FullText => "FT",
            SearchMode::ExactMatch => "EX",
            SearchMode::Wildcard => "WC",
        };
        let mode_paragraph = Paragraph::new(mode_str)
            .style(Style::default().fg(Color::Green))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(mode_paragraph, chunks[5]);
    }

    fn draw_results_table(&mut self, frame: &mut Frame, area: Rect) {
        let results: Vec<SearchResult> = self.get_current_results().into_iter().cloned().collect();

        if results.is_empty() {
            let empty_msg = if self.loading {
                let spinner_chars = ['|', '/', '-', '\\'];
                let char_idx = self.tick_count % spinner_chars.len();
                format!("Loading... {}", spinner_chars[char_idx])
            } else if self.results.is_empty() && self.search_input.value().is_empty() {
                "Press '/' to search".to_string()
            } else {
                "No results found".to_string()
            };

            let paragraph = Paragraph::new(empty_msg)
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            frame.render_widget(paragraph, area);
            return;
        }

        let first_result = &results[0];
        let headers: Vec<String> = first_result
            .row
            .iter()
            .enumerate()
            .map(|(i, _)| format!("Col{}", i + 1))
            .collect();

        let header_cells: Vec<_> = headers
            .iter()
            .map(|h| {
                ratatui::widgets::Cell::from(h.as_str()).style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            })
            .collect();
        let header_row = ratatui::widgets::Row::new(header_cells)
            .height(1)
            .bottom_margin(1);

        let rows: Vec<_> = results
            .iter()
            .enumerate()
            .map(|(_idx, result)| {
                let cells: Vec<_> = result
                    .row
                    .iter()
                    .map(|cell| {
                        ratatui::widgets::Cell::from(cell.as_str())
                            .style(Style::default().fg(Color::White))
                    })
                    .collect();
                ratatui::widgets::Row::new(cells).height(1)
            })
            .collect();

        let num_columns = headers.len();
        let mut constraints = Vec::new();
        for _ in 0..num_columns {
            constraints.push(Constraint::Min(10));
        }
        if constraints.is_empty() {
            constraints.push(Constraint::Min(10));
        }

        let table = Table::new(rows, constraints)
            .header(header_row)
            .block(Block::default().borders(Borders::ALL))
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

    fn draw_status_bar(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);

        let stats_text = if let Some(stats) = &self.stats {
            let mut sheet_stats: Vec<_> = stats.matches_per_sheet.iter().collect();
            sheet_stats.sort_by_key(|(k, _)| *k);
            let sheet_info: Vec<_> = sheet_stats
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect();
            format!(
                "Rows: {} | Matched: {} | Time: {:.2}s | {}",
                stats.total_rows_searched,
                stats.total_matches,
                stats.search_duration.as_secs_f64(),
                sheet_info.join(" | ")
            )
        } else {
            self.status_message.clone()
        };

        let stats_line = if self.loading {
            let spinner_chars = ['|', '/', '-', '\\'];
            let char_idx = self.tick_count % spinner_chars.len();
            format!("{} {}", stats_text, spinner_chars[char_idx])
        } else {
            stats_text
        };

        let stats_paragraph = Paragraph::new(stats_line).style(Style::default().fg(Color::White));
        frame.render_widget(stats_paragraph, chunks[0]);

        let error_style = if self.error_message.is_some() {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Gray)
        };

        let hints = vec![
            Span::styled("/", Style::default().fg(Color::Cyan)),
            Span::raw(":Search "),
            Span::styled("c", Style::default().fg(Color::Cyan)),
            Span::raw(":Column "),
            Span::styled("Tab", Style::default().fg(Color::Cyan)),
            Span::raw(":Mode "),
            Span::styled("?", Style::default().fg(Color::Cyan)),
            Span::raw(":Help "),
            Span::styled("q", Style::default().fg(Color::Cyan)),
            Span::raw(":Quit"),
        ];

        let hints_line = Line::from(hints);
        let hints_paragraph = Paragraph::new(hints_line).style(error_style);
        frame.render_widget(hints_paragraph, chunks[1]);
    }

    fn draw_help_popup(&mut self, frame: &mut Frame) {
        let area = centered_rect(60, 70, frame.area());

        let help_text = vec![
            Line::from(Span::styled(
                "Keybindings",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("q / Ctrl+C", Style::default().fg(Color::Yellow)),
                Span::raw(" - Quit"),
            ]),
            Line::from(vec![
                Span::styled("/ or e", Style::default().fg(Color::Yellow)),
                Span::raw(" - Enter search mode"),
            ]),
            Line::from(vec![
                Span::styled("c", Style::default().fg(Color::Yellow)),
                Span::raw(" - Enter column filter mode"),
            ]),
            Line::from(vec![
                Span::styled("Tab", Style::default().fg(Color::Yellow)),
                Span::raw(" - Cycle search mode"),
            ]),
            Line::from(vec![
                Span::styled("Enter", Style::default().fg(Color::Yellow)),
                Span::raw(" - Execute search"),
            ]),
            Line::from(vec![
                Span::styled("o", Style::default().fg(Color::Yellow)),
                Span::raw(" - Open file dialog"),
            ]),
            Line::from(vec![
                Span::styled("? or h", Style::default().fg(Color::Yellow)),
                Span::raw(" - Toggle this help"),
            ]),
            Line::from(vec![
                Span::styled("1-9", Style::default().fg(Color::Yellow)),
                Span::raw(" - Select sheet tab"),
            ]),
            Line::from(vec![
                Span::styled("Left/Right", Style::default().fg(Color::Yellow)),
                Span::raw(" - Previous/next tab"),
            ]),
            Line::from(vec![
                Span::styled("Up/Down or j/k", Style::default().fg(Color::Yellow)),
                Span::raw(" - Navigate rows"),
            ]),
            Line::from(vec![
                Span::styled("g", Style::default().fg(Color::Yellow)),
                Span::raw(" - Jump to first row"),
            ]),
            Line::from(vec![
                Span::styled("G", Style::default().fg(Color::Yellow)),
                Span::raw(" - Jump to last row"),
            ]),
            Line::from(vec![
                Span::styled("d", Style::default().fg(Color::Yellow)),
                Span::raw(" - Clear all data"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::raw(" - Cancel current mode"),
            ]),
        ];

        let help_block = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .alignment(Alignment::Left);

        frame.render_widget(Clear, area);
        frame.render_widget(help_block, area);
    }

    fn draw_file_list_popup(&mut self, frame: &mut Frame) {
        let area = centered_rect(50, 50, frame.area());

        let items: Vec<_> = self
            .file_list
            .iter()
            .map(|file| {
                ListItem::new(format!(
                    "{} ({} sheets, {} rows)",
                    file.name,
                    file.sheets.len(),
                    file.total_rows
                ))
                .style(Style::default().fg(Color::White))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Files"))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

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
