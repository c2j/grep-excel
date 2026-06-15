mod handlers;
pub mod render;
mod theme;
mod ui;

use crate::engine::{
    DefaultEngine, FileInfo, SearchEngine, SearchMode, SearchQuery, SearchResult, SearchStats,
};
use crate::event::{AppEvent, EventReceiver, EventSender};
use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use parking_lot::RwLock;
use ratatui::widgets::{ScrollbarState, TableState};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tui_input::Input;

pub(crate) struct SyncDb(pub(crate) DefaultEngine);
unsafe impl Sync for SyncDb {}
unsafe impl Send for SyncDb {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppMode {
    Normal,
    EditingSearch,
    EditingColumn,
    EditingAggregate,
    EditingSql,
    SqlTableInfo,
    SelectFile,
    Help,
    DetailPanel,
}

pub struct App {
    pub(crate) running: bool,
    pub(crate) mode: AppMode,
    pub(crate) search_input: Input,
    pub(crate) column_input: Input,
    pub(crate) search_mode: SearchMode,
    pub(crate) results: Vec<SearchResult>,
    pub(crate) results_by_sheet: HashMap<String, Vec<SearchResult>>,
    pub(crate) stats: Option<SearchStats>,
    pub(crate) table_state: TableState,
    pub(crate) tab_state: usize,
    pub(crate) col_offset: usize,
    pub(crate) scroll_state: ScrollbarState,
    pub(crate) status_message: String,
    pub(crate) file_list: Vec<FileInfo>,
    pub(crate) file_list_state: ratatui::widgets::ListState,
    pub(crate) loading: bool,
    pub(crate) error_message: Option<String>,
    pub(crate) database: Arc<RwLock<SyncDb>>,
    pub(crate) event_tx: EventSender,
    pub(crate) event_rx: EventReceiver,
    pub(crate) tick_count: usize,
    pub(crate) detail_scroll: usize,
    pub(crate) visible_col_count: usize,
    pub(crate) result_limit: usize,
    pub(crate) sql_input: Input,
    pub(crate) sql_result: Option<crate::types::SqlResult>,
    pub(crate) flat_view: bool,
    pub(crate) flat_selected_sheet: usize,
    pub(crate) flat_row_index: usize,
    pub(crate) flat_scroll_offset: usize,
    pub(crate) flat_col_offsets: HashMap<String, usize>,
    pub(crate) aggregate_input: Input,
    pub(crate) aggregate_stats: Option<crate::types::AggregateStats>,
    pub(crate) table_aliases: Vec<crate::types::TableAliasInfo>,
    pub(crate) table_info_scroll: usize,
}

impl App {
    pub fn new(database: DefaultEngine, event_tx: EventSender, event_rx: EventReceiver) -> Self {
        let database = Arc::new(RwLock::new(SyncDb(database)));
        let initial_files = database.read().0.list_files();
        let file_count = initial_files.len();
        let status = if file_count > 0 {
            crate::i18n::welcome_loaded(file_count)
        } else {
            crate::i18n::welcome_empty().to_string()
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
            sql_input: Input::default(),
            sql_result: None,
            flat_view: false,
            flat_selected_sheet: 0,
            flat_row_index: 0,
            flat_scroll_offset: 0,
            flat_col_offsets: HashMap::new(),
            aggregate_input: Input::default(),
            aggregate_stats: None,
            table_aliases: Vec::new(),
            table_info_scroll: 0,
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
        self.status_message = crate::i18n::status_importing(&path);
        let db = Arc::clone(&self.database);
        let tx = self.event_tx.clone();
        let path_clone = path.clone();

        std::thread::spawn(move || {
            let result = {
                let mut db_guard = db.write();
                let progress_cb = |current, total| {
                    let _ = tx.send(AppEvent::Progress(current, total));
                };
                db_guard.0.import_excel(&path_clone, &progress_cb)
            };
            let _ = tx.send(AppEvent::FileImported(result));
        });
    }

    pub fn execute_sql_query(&mut self) {
        if self.sql_input.value().is_empty() {
            return;
        }

        self.loading = true;
        self.status_message = crate::i18n::status_executing_sql().to_string();
        self.error_message = None;

        let sql = self.sql_input.value().to_string();
        let limit = self.result_limit;
        let db = Arc::clone(&self.database);
        let tx = self.event_tx.clone();

        std::thread::spawn(move || {
            let result = {
                let db_guard = db.read();
                db_guard.0.execute_sql(&sql, limit)
            };
            let _ = tx.send(AppEvent::SqlCompleted(result));
        });
    }

    pub fn execute_search(&mut self) {
        if self.search_input.value().is_empty() {
            return;
        }

        self.loading = true;
        self.status_message = crate::i18n::status_searching().to_string();
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
            sheet: None,
            invert: false,
        };

        let db = Arc::clone(&self.database);
        let tx = self.event_tx.clone();

        std::thread::spawn(move || {
            let result = {
                let db_guard = db.read();
                db_guard.0.search(&query)
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
                        {
                            let db_guard = self.database.read();
                            self.table_aliases = db_guard.0.list_table_aliases();
                        }
                        self.status_message = crate::i18n::status_imported(&file_info.name);
                        self.error_message = None;
                    }
                    Err(e) => {
                        self.error_message = Some(crate::i18n::status_import_error(&e.to_string()));
                        self.status_message = crate::i18n::status_import_failed().to_string();
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
                        self.flat_view = self.results_by_sheet.len() > 1;
                        self.flat_selected_sheet = 0;
                        self.flat_row_index = 0;
                        self.flat_scroll_offset = 0;
                        self.flat_col_offsets.clear();
                        self.refresh_aggregate_stats();

                        self.status_message = if stats.truncated {
                            crate::i18n::status_matches_truncated(
                                stats.total_matches,
                                self.result_limit,
                                stats.search_duration.as_secs_f64(),
                            )
                        } else {
                            crate::i18n::status_matches(
                                stats.total_matches,
                                stats.search_duration.as_secs_f64(),
                            )
                        };
                        self.error_message = None;
                    }
                    Err(e) => {
                        self.error_message = Some(crate::i18n::status_search_error(&e.to_string()));
                        self.status_message = crate::i18n::status_search_failed().to_string();
                    }
                }
            }
            AppEvent::Progress(current, total) => {
                self.status_message = crate::i18n::status_progress(current, total);
            }
            AppEvent::SqlCompleted(result) => {
                self.loading = false;
                match result {
                    Ok(sql_result) => {
                        let count = sql_result.row_count;
                        let duration = sql_result.duration.as_secs_f64();
                        let truncated = sql_result.truncated;
                        self.sql_result = Some(sql_result);
                        self.error_message = None;
                        self.results.clear();
                        self.results_by_sheet.clear();
                        self.stats = None;
                        self.tab_state = 0;
                        self.col_offset = 0;
                        self.table_state.select(Some(0));
                        self.update_scroll_state();

                        self.status_message = if truncated {
                            crate::i18n::status_sql_truncated(count, self.result_limit, duration)
                        } else {
                            crate::i18n::status_sql_done(count, duration)
                        };
                    }
                    Err(e) => {
                        self.error_message = Some(crate::i18n::status_sql_error(&e.to_string()));
                        self.status_message = crate::i18n::status_sql_failed().to_string();
                    }
                }
            }
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut terminal = render::init_terminal()?;

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

        render::restore_terminal()?;
        Ok(())
    }

    pub(crate) fn get_sorted_sheet_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.results_by_sheet.keys().cloned().collect();
        names.sort();
        names
    }

    pub(crate) fn get_flat_current_sheet_name(&self) -> Option<String> {
        let names = self.get_sorted_sheet_names();
        names.get(self.flat_selected_sheet).cloned()
    }

    pub(crate) fn get_flat_current_results(&self) -> Vec<&SearchResult> {
        if let Some(sheet_name) = self.get_flat_current_sheet_name() {
            self.results_by_sheet
                .get(&sheet_name)
                .map(|v| v.iter().collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    pub(crate) fn is_flat_view_active(&self) -> bool {
        self.tab_state == 0 && self.flat_view && self.results_by_sheet.len() > 1
    }

    pub(crate) fn get_flat_col_offset(&self, sheet_name: &str) -> usize {
        self.flat_col_offsets.get(sheet_name).copied().unwrap_or(0)
    }

    pub(crate) fn set_flat_col_offset(&mut self, sheet_name: &str, offset: usize) {
        self.flat_col_offsets.insert(sheet_name.to_string(), offset);
    }

    pub(crate) fn compute_aggregate_stats(&self) -> Option<crate::types::AggregateStats> {
        let col_name = self.aggregate_input.value();
        if col_name.is_empty() {
            return None;
        }

        let mut counts: HashMap<String, usize> = HashMap::new();

        for result in &self.results {
            if let Some(col_idx) = result.col_names.iter().position(|c| c == col_name) {
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
            Some(crate::types::AggregateStats {
                column: col_name.to_string(),
                counts,
            })
        }
    }

    pub(crate) fn refresh_aggregate_stats(&mut self) {
        self.aggregate_stats = self.compute_aggregate_stats();
    }
}
