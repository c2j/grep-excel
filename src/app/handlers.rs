use super::{App, AppMode};
use crate::engine::{SearchEngine, SearchMode, SearchResult};
use ratatui::widgets::ScrollbarState;
use tui_input::backend::crossterm::EventHandler;

impl App {
    pub(super) fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
        match self.mode {
            AppMode::Normal => self.handle_normal_mode(key),
            AppMode::EditingSearch => self.handle_search_edit_mode(key),
            AppMode::EditingColumn => self.handle_column_edit_mode(key),
            AppMode::SelectFile => self.handle_select_file_mode(key),
            AppMode::Help => self.handle_help_mode(key),
            AppMode::DetailPanel => self.handle_detail_panel_mode(key),
        }
    }

    pub(super) fn handle_normal_mode(&mut self, key: crossterm::event::KeyEvent) {
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
                let mode_str = crate::i18n::mode_name(self.search_mode);
                self.status_message = crate::i18n::status_mode_changed(mode_str);
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
                        self.status_message = crate::i18n::err_no_files().to_string();
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
                let mut db = self.database.write();
                if db.0.clear().is_ok() {
                    self.file_list.clear();
                    self.results.clear();
                    self.results_by_sheet.clear();
                    self.stats = None;
                    self.tab_state = 0;
                    self.col_offset = 0;
                    self.table_state.select(Some(0));
                    self.detail_scroll = 0;
                    self.result_limit = 5000;
                    self.status_message = crate::i18n::status_cleared().to_string();
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
                self.column_input = tui_input::Input::default();
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

    pub(super) fn select_tab(&mut self, index: usize) {
        let max_tabs = self.get_tab_count();
        if index < max_tabs {
            self.tab_state = index;
            self.col_offset = 0;
            self.table_state.select(Some(0));
            self.update_scroll_state();
            self.detail_scroll = 0;
        }
    }

    pub(super) fn get_tab_count(&self) -> usize {
        self.results_by_sheet.len() + 1
    }

    pub(super) fn get_current_results(&self) -> Vec<&SearchResult> {
        if self.tab_state == 0 {
            self.results.iter().collect()
        } else {
            let sheet_names: Vec<_> = self.results_by_sheet.keys().cloned().collect();
            let mut sorted_sheets = sheet_names;
            sorted_sheets.sort();

            if let Some(sheet_name) = sorted_sheets.get(self.tab_state - 1) {
                self.results_by_sheet
                    .get(sheet_name)
                    .map(|v: &Vec<SearchResult>| v.iter().collect())
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        }
    }

    pub(super) fn navigate_table(&mut self, direction: i32) {
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

    pub(super) fn update_scroll_state(&mut self) {
        let results = self.get_current_results();
        let total_rows = results.len();
        self.scroll_state = ScrollbarState::new(total_rows);
    }

    pub(super) fn get_current_col_count(&self) -> usize {
        let results = self.get_current_results();
        results
            .iter()
            .find(|r| !r.col_names.is_empty())
            .map(|r| r.col_names.len())
            .unwrap_or(0)
    }
}
