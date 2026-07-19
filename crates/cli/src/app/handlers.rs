use super::{App, AppMode};
use crate::engine::{SearchEngine, SearchMode, SearchResult};
use crate::event::AppEvent;
use crossterm::event::KeyModifiers;
use ratatui::widgets::ScrollbarState;
use std::sync::Arc;
use tui_input::backend::crossterm::EventHandler;

impl App {
    pub(super) fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
        match self.mode {
            AppMode::Normal => self.handle_normal_mode(key),
            AppMode::EditingSearch => self.handle_search_edit_mode(key),
            AppMode::EditingColumn => self.handle_column_edit_mode(key),
            AppMode::EditingAggregate => self.handle_aggregate_edit_mode(key),
            AppMode::EditingSql => self.handle_sql_edit_mode(key),
            AppMode::SqlTableInfo => self.handle_sql_table_info_mode(key),
            AppMode::SelectFile => self.handle_select_file_mode(key),
            AppMode::Help => self.handle_help_mode(key),
            AppMode::DetailPanel => self.handle_detail_panel_mode(key),
        }
    }

    pub(super) fn is_browse_mode(&self) -> bool {
        self.results.is_empty()
            && self.search_input.value().is_empty()
            && self.sql_result.is_none()
            && self.browse_data.is_some()
    }

    pub(super) fn handle_normal_mode(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        let browsing = self.is_browse_mode();

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
            KeyCode::Char('a') => {
                self.mode = AppMode::EditingAggregate;
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
            KeyCode::Char(']') => {
                if browsing {
                    self.browse_next_sheet();
                }
            }
            KeyCode::Char('[') => {
                if browsing {
                    self.browse_prev_sheet();
                }
            }
            KeyCode::Enter => {
                if browsing {
                    self.browse_show_detail();
                } else if self.results.is_empty() || self.loading {
                    self.execute_search();
                } else if self.is_flat_view_active() {
                    let results = self.get_flat_current_results();
                    if !results.is_empty() {
                        self.mode = AppMode::DetailPanel;
                        self.detail_scroll = 0;
                    }
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
                if browsing {
                    let index = (c as usize) - ('1' as usize);
                    let mut total_sheets = 0usize;
                    for f in &self.file_list {
                        total_sheets += f.sheets.len();
                    }
                    if index < total_sheets {
                        let mut remaining = index;
                        for (fi, f) in self.file_list.iter().enumerate() {
                            if remaining < f.sheets.len() {
                                self.browse_file_index = fi;
                                self.browse_sheet_index = remaining;
                                self.load_browse_data();
                                break;
                            }
                            remaining -= f.sheets.len();
                        }
                    }
                } else {
                    let index = (c as usize) - ('1' as usize);
                    self.select_tab(index);
                }
            }
            KeyCode::Left => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                if ctrl {
                    if browsing {
                        self.browse_prev_sheet_in_file();
                    } else if self.is_flat_view_active() {
                        self.navigate_flat_prev_sheet_in_file();
                    } else if !self.results_by_sheet.is_empty() {
                        self.tab_state = 0;
                        self.flat_view = true;
                        self.navigate_flat_prev_sheet_in_file();
                    }
                } else if browsing {
                    if self.browse_col_offset > 0 {
                        self.browse_col_offset -= 1;
                    }
                } else if self.is_flat_view_active() {
                    if let Some(sheet_key) = self.get_flat_current_sheet_name() {
                        let offset = self.get_flat_col_offset(&sheet_key);
                        if offset > 0 {
                            self.set_flat_col_offset(&sheet_key, offset - 1);
                        }
                    }
                } else if self.col_offset > 0 {
                    self.col_offset -= 1;
                }
            }
            KeyCode::Right => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                if ctrl {
                    if browsing {
                        self.browse_next_sheet_in_file();
                    } else if self.is_flat_view_active() {
                        self.navigate_flat_next_sheet_in_file();
                    } else if !self.results_by_sheet.is_empty() {
                        self.tab_state = 0;
                        self.flat_view = true;
                        self.navigate_flat_next_sheet_in_file();
                    }
                } else if browsing {
                    let total_cols = self.browse_data.as_ref()
                        .map(|d| d.columns.len()).unwrap_or(0);
                    if self.browse_col_offset < total_cols.saturating_sub(1) {
                        self.browse_col_offset += 1;
                    }
                } else if self.is_flat_view_active() {
                    if let Some(sheet_key) = self.get_flat_current_sheet_name() {
                        let col_count = self.get_flat_current_col_count();
                        let offset = self.get_flat_col_offset(&sheet_key);
                        if offset < col_count.saturating_sub(1) {
                            self.set_flat_col_offset(&sheet_key, offset + 1);
                        }
                    }
                } else {
                    let col_count = self.get_current_col_count();
                    if self.col_offset < col_count.saturating_sub(1) {
                        self.col_offset += 1;
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                if ctrl {
                    if browsing {
                        self.browse_prev_file();
                    } else if self.is_flat_view_active() {
                        self.navigate_flat_prev_file();
                    } else if !self.results_by_sheet.is_empty() {
                        self.tab_state = 0;
                        self.flat_view = true;
                        self.navigate_flat_prev_file();
                    }
                } else if browsing {
                    self.browse_scroll_up(1);
                } else if self.is_flat_view_active() {
                    self.navigate_flat_view(-1);
                } else {
                    self.navigate_table(-1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                if ctrl {
                    if browsing {
                        self.browse_next_file();
                    } else if self.is_flat_view_active() {
                        self.navigate_flat_next_file();
                    } else if !self.results_by_sheet.is_empty() {
                        self.tab_state = 0;
                        self.flat_view = true;
                        self.navigate_flat_next_file();
                    }
                } else if browsing {
                    self.browse_scroll_down(1);
                } else if self.is_flat_view_active() {
                    self.navigate_flat_view(1);
                } else {
                    self.navigate_table(1);
                }
            }
            KeyCode::Char('g') => {
                if browsing {
                    self.table_state.select(Some(0));
                    self.browse_scroll_offset = 0;
                } else if self.is_flat_view_active() {
                    self.flat_selected_sheet = 0;
                    self.flat_row_index = 0;
                    self.flat_scroll_offset = 0;
                } else {
                    self.table_state.select(Some(0));
                    self.scroll_state = self.scroll_state.position(0);
                }
            }
            KeyCode::Char('G') => {
                if browsing {
                    let total_rows = self.browse_data.as_ref()
                        .map(|d| d.rows.len()).unwrap_or(0);
                    let last = total_rows.saturating_sub(1);
                    self.table_state.select(Some(last));
                    self.browse_scroll_offset = last;
                } else if self.is_flat_view_active() {
                    let sheet_keys = self.get_ordered_sheet_list();
                    if let Some(last_key) = sheet_keys.last() {
                        self.flat_selected_sheet = sheet_keys.len().saturating_sub(1);
                        if let Some(results) = self.results_by_sheet.get(last_key) {
                            self.flat_row_index = results.len().saturating_sub(1);
                            self.flat_scroll_offset = self.flat_row_index;
                        }
                    }
                } else {
                    let results = self.get_current_results();
                    let last = results.len().saturating_sub(1);
                    self.table_state.select(Some(last));
                    self.scroll_state = self.scroll_state.position(last);
                }
            }
            KeyCode::Char('H') => {
                if browsing {
                    if self.browse_col_offset > 0 {
                        self.browse_col_offset -= 1;
                    }
                } else if self.col_offset > 0 {
                    self.col_offset -= 1;
                }
            }
            KeyCode::Char('L') => {
                if browsing {
                    let total_cols = self.browse_data.as_ref()
                        .map(|d| d.columns.len()).unwrap_or(0);
                    if self.browse_col_offset < total_cols.saturating_sub(1) {
                        self.browse_col_offset += 1;
                    }
                } else {
                    let col_count = self.get_current_col_count();
                    if self.col_offset < col_count.saturating_sub(1) {
                        self.col_offset += 1;
                    }
                }
            }
            KeyCode::Char('d') => {
                self.browse_data = None;
                self.browse_scroll_offset = 0;
                self.browse_col_offset = 0;
                self.browse_file_index = 0;
                self.browse_sheet_index = 0;

                let mut db = self.database.lock();
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
                    self.sql_result = None;
                    self.sql_input = tui_input::Input::default();
                    self.flat_view = false;
                    self.flat_selected_sheet = 0;
                    self.flat_row_index = 0;
                    self.flat_scroll_offset = 0;
                    self.flat_col_offsets.clear();
                    self.aggregate_input = tui_input::Input::default();
                    self.aggregate_stats = None;
                    self.status_message = crate::i18n::status_cleared().to_string();
                }
            }
            KeyCode::Char('n') => {
                if browsing {
                    self.load_more_browse_data();
                } else if self.stats.as_ref().is_some_and(|s| s.truncated) {
                    self.result_limit = self.result_limit.saturating_add(5000);
                    self.execute_search();
                }
            }
            KeyCode::Char('s') => {
                self.export_results();
            }
            KeyCode::Char('v') => {
                if self.tab_state == 0 && self.results_by_sheet.len() > 1 {
                    self.flat_view = !self.flat_view;
                    self.flat_selected_sheet = 0;
                    self.flat_row_index = 0;
                    self.flat_scroll_offset = 0;
                    let view_name = if self.flat_view {
                        crate::i18n::status_view_flat()
                    } else {
                        crate::i18n::status_view_table()
                    };
                    self.status_message = crate::i18n::status_mode_changed(view_name);
                }
            }
            KeyCode::Char('S') => {
                if self.table_aliases.is_empty() {
                    let db_guard = self.database.lock();
                    self.table_aliases = db_guard.0.list_table_aliases();
                }
                if self.table_aliases.is_empty() {
                    self.status_message = crate::i18n::status_no_tables().to_string();
                } else {
                    self.table_info_scroll = 0;
                    self.mode = AppMode::SqlTableInfo;
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
                self.column_input = tui_input::Input::default();
                self.mode = AppMode::Normal;
            }
            _ => {
                self.column_input
                    .handle_event(&crossterm::event::Event::Key(key));
            }
        }
    }

    fn handle_aggregate_edit_mode(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Enter => {
                self.refresh_aggregate_stats();
                self.mode = AppMode::Normal;
            }
            KeyCode::Esc => {
                self.aggregate_input = tui_input::Input::default();
                self.refresh_aggregate_stats();
                self.mode = AppMode::Normal;
            }
            _ => {
                self.aggregate_input
                    .handle_event(&crossterm::event::Event::Key(key));
            }
        }
    }

    fn handle_sql_edit_mode(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Enter => {
                self.execute_sql_query();
                self.mode = AppMode::Normal;
            }
            KeyCode::Esc => {
                self.mode = AppMode::Normal;
            }
            _ => {
                self.sql_input
                    .handle_event(&crossterm::event::Event::Key(key));
            }
        }
    }

    fn handle_sql_table_info_mode(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Enter => {
                self.mode = AppMode::EditingSql;
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = AppMode::Normal;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.table_info_scroll = self.table_info_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.table_info_scroll = self.table_info_scroll.saturating_add(1);
            }
            _ => {}
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

        let came_from_browse = self.results.len() == 1 && self.browse_data.is_some();
        let browse_restore_row = if came_from_browse {
            self.results.first().map(|r| r.row_index)
        } else {
            None
        };

        match key.code {
            KeyCode::Enter | KeyCode::Esc => {
                if came_from_browse {
                    self.results.clear();
                    self.results_by_sheet.clear();
                    self.tab_state = 0;
                    if let Some(row_idx) = browse_restore_row {
                        self.table_state.select(Some(row_idx));
                        let visible = self.browse_visible_rows.max(5);
                        if row_idx < self.browse_scroll_offset
                            || row_idx >= self.browse_scroll_offset + visible
                        {
                            self.browse_scroll_offset = row_idx.saturating_sub(visible / 2);
                        }
                    }
                }
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
            if index == 0 {
                self.flat_selected_sheet = 0;
                self.flat_row_index = 0;
                self.flat_scroll_offset = 0;
            }
        }
    }

    pub(super) fn get_tab_count(&self) -> usize {
        self.results_by_sheet.len() + 1
    }

    pub(super) fn get_current_results(&self) -> Vec<&SearchResult> {
        if self.tab_state == 0 {
            self.results.iter().collect()
        } else {
            let ordered = self.get_ordered_sheet_list();
            if let Some(sheet_key) = ordered.get(self.tab_state - 1) {
                self.results_by_sheet
                    .get(sheet_key)
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

    pub(super) fn navigate_flat_view(&mut self, direction: i32) {
        let sheet_keys = self.get_ordered_sheet_list();
        if sheet_keys.is_empty() {
            return;
        }

        let current_key = &sheet_keys[self.flat_selected_sheet];
        let results = self.results_by_sheet.get(current_key).unwrap();
        let new_row = self.flat_row_index as i32 + direction;

        if new_row < 0 {
            if self.flat_selected_sheet > 0 {
                self.flat_selected_sheet -= 1;
                let prev_key = &sheet_keys[self.flat_selected_sheet];
                let prev_results = self.results_by_sheet.get(prev_key).unwrap();
                self.flat_row_index = prev_results.len().saturating_sub(1);
                self.flat_scroll_offset = self.flat_row_index;
            }
        } else if new_row >= results.len() as i32 {
            if self.flat_selected_sheet < sheet_keys.len() - 1 {
                self.flat_selected_sheet += 1;
                self.flat_row_index = 0;
                self.flat_scroll_offset = 0;
            }
        } else {
            self.flat_row_index = new_row as usize;
            let visible_rows = 10;
            if self.flat_row_index < self.flat_scroll_offset {
                self.flat_scroll_offset = self.flat_row_index;
            } else if self.flat_row_index >= self.flat_scroll_offset + visible_rows {
                self.flat_scroll_offset = self.flat_row_index.saturating_sub(visible_rows - 1);
            }
        }
    }

    /// Ctrl+←: previous sheet within the same file
    pub(super) fn navigate_flat_prev_sheet_in_file(&mut self) {
        let sheet_keys = self.get_ordered_sheet_list();
        if sheet_keys.is_empty() {
            return;
        }
        let (file_start, _) = self.get_file_sheet_range(self.flat_selected_sheet);
        if self.flat_selected_sheet > file_start {
            self.flat_selected_sheet -= 1;
            self.flat_row_index = 0;
            self.flat_scroll_offset = 0;
            self.status_message = self.format_flat_sheet_status();
        }
    }

    /// Ctrl+→: next sheet within the same file
    pub(super) fn navigate_flat_next_sheet_in_file(&mut self) {
        let sheet_keys = self.get_ordered_sheet_list();
        if sheet_keys.is_empty() {
            return;
        }
        let (_, file_end) = self.get_file_sheet_range(self.flat_selected_sheet);
        if self.flat_selected_sheet < file_end {
            self.flat_selected_sheet += 1;
            self.flat_row_index = 0;
            self.flat_scroll_offset = 0;
            self.status_message = self.format_flat_sheet_status();
        }
    }

    /// Ctrl+↑: switch to previous file's first sheet
    pub(super) fn navigate_flat_prev_file(&mut self) {
        let sheet_keys = self.get_ordered_sheet_list();
        if sheet_keys.is_empty() {
            return;
        }
        let current_file_idx = self.find_file_for_sheet_key(&sheet_keys[self.flat_selected_sheet]);
        if let Some(fi) = current_file_idx {
            if fi > 0 {
                // Find the first sheet of the previous file
                let prev_file_name = &self.file_list[fi - 1].name;
                for (i, key) in sheet_keys.iter().enumerate() {
                    let (f, _) = App::parse_sheet_key(key);
                    if f == prev_file_name {
                        self.flat_selected_sheet = i;
                        self.flat_row_index = 0;
                        self.flat_scroll_offset = 0;
                        self.status_message = self.format_flat_sheet_status();
                        return;
                    }
                }
            }
        }
    }

    /// Ctrl+↓: switch to next file's first sheet
    pub(super) fn navigate_flat_next_file(&mut self) {
        let sheet_keys = self.get_ordered_sheet_list();
        if sheet_keys.is_empty() {
            return;
        }
        let current_file_idx = self.find_file_for_sheet_key(&sheet_keys[self.flat_selected_sheet]);
        if let Some(fi) = current_file_idx {
            if fi + 1 < self.file_list.len() {
                let next_file_name = &self.file_list[fi + 1].name;
                for (i, key) in sheet_keys.iter().enumerate() {
                    let (f, _) = App::parse_sheet_key(key);
                    if f == next_file_name {
                        self.flat_selected_sheet = i;
                        self.flat_row_index = 0;
                        self.flat_scroll_offset = 0;
                        self.status_message = self.format_flat_sheet_status();
                        return;
                    }
                }
            }
        }
    }

    fn format_flat_sheet_status(&self) -> String {
        let sheet_keys = self.get_ordered_sheet_list();
        if self.flat_selected_sheet < sheet_keys.len() {
            let (file_name, sheet_name) = App::parse_sheet_key(&sheet_keys[self.flat_selected_sheet]);
            let total = sheet_keys.len();
            crate::i18n::status_flat_sheet(file_name, sheet_name, self.flat_selected_sheet + 1, total)
        } else {
            String::new()
        }
    }

    pub(super) fn get_flat_current_col_count(&self) -> usize {
        let results = self.get_flat_current_results();
        results
            .iter()
            .find(|r| !r.col_names.is_empty())
            .map(|r| r.col_names.len())
            .unwrap_or(0)
    }

    pub(super) fn browse_show_detail(&mut self) {
        let data = match &self.browse_data {
            Some(d) => d,
            None => return,
        };
        let row_idx = self.table_state.selected().unwrap_or(0);
        if row_idx >= data.rows.len() {
            return;
        }
        let row = data.rows[row_idx].clone();
        let fake_result = SearchResult {
            sheet_name: data.sheet_name.clone(),
            file_name: data.file_name.clone(),
            row,
            col_names: data.columns.clone(),
            matched_columns: vec![],
            col_widths: vec![],
            row_index: row_idx,
            context: crate::types::ContextRows::default(),
        };
        self.results = vec![fake_result];
        self.results_by_sheet.clear();
        let key = format!("{}::{}", data.file_name, data.sheet_name);
        self.results_by_sheet
            .insert(key, self.results.clone());
        self.tab_state = 1;
        self.detail_scroll = 0;
        self.table_state.select(Some(0));
        self.mode = AppMode::DetailPanel;
    }

    pub(super) fn browse_scroll_up(&mut self, amount: usize) {
        let total_rows = self.browse_data.as_ref().map(|d| d.rows.len()).unwrap_or(0);
        if total_rows == 0 {
            return;
        }
        let current = self.table_state.selected().unwrap_or(0);
        let new_selection = current.saturating_sub(amount);
        self.table_state.select(Some(new_selection));
        if new_selection < self.browse_scroll_offset {
            self.browse_scroll_offset = new_selection;
        }
    }

    pub(super) fn browse_scroll_down(&mut self, amount: usize) {
        let total_rows = self.browse_data.as_ref().map(|d| d.rows.len()).unwrap_or(0);
        if total_rows == 0 {
            return;
        }
        let current = self.table_state.selected().unwrap_or(0);
        let new_selection = (current + amount).min(total_rows.saturating_sub(1));
        self.table_state.select(Some(new_selection));

        let visible_rows = self.browse_visible_rows.max(5);
        if new_selection >= self.browse_scroll_offset + visible_rows {
            self.browse_scroll_offset = new_selection.saturating_sub(visible_rows - 1);
        }
    }

    pub(super) fn browse_next_sheet(&mut self) {
        if self.file_list.is_empty() {
            return;
        }
        if self.browse_file_index >= self.file_list.len() {
            self.browse_file_index = 0;
            self.browse_sheet_index = 0;
        } else {
            let sheets_in_file = self.file_list[self.browse_file_index].sheets.len();
            if self.browse_sheet_index + 1 < sheets_in_file {
                self.browse_sheet_index += 1;
            } else if self.browse_file_index + 1 < self.file_list.len() {
                self.browse_file_index += 1;
                self.browse_sheet_index = 0;
            } else {
                self.browse_file_index = 0;
                self.browse_sheet_index = 0;
            }
        }
        self.load_browse_data();
    }

    pub(super) fn browse_prev_sheet(&mut self) {
        if self.file_list.is_empty() {
            return;
        }
        if self.browse_sheet_index > 0 {
            self.browse_sheet_index -= 1;
        } else if self.browse_file_index > 0 {
            self.browse_file_index -= 1;
            self.browse_sheet_index = self.file_list[self.browse_file_index]
                .sheets
                .len()
                .saturating_sub(1);
        } else {
            self.browse_file_index = self.file_list.len().saturating_sub(1);
            self.browse_sheet_index = self.file_list[self.browse_file_index]
                .sheets
                .len()
                .saturating_sub(1);
        }
        self.load_browse_data();
    }

    pub(super) fn load_more_browse_data(&mut self) {
        let data = match &self.browse_data {
            Some(d) => d,
            None => return,
        };
        if data.truncated {
            let file_name = data.file_name.clone();
            let sheet_name = data.sheet_name.clone();
            let current_count = data.rows.len();

            self.browse_loading = true;
            self.status_message = crate::i18n::status_browse_loading(&file_name, &sheet_name);

            let db = Arc::clone(&self.database);
            let tx = self.event_tx.clone();

            std::thread::spawn(move || {
                let result = {
                    let db_guard = db.lock();
                    db_guard.0.get_sheet_data(
                        &file_name,
                        &sheet_name,
                        Some(0),
                        Some(current_count + 500),
                        None,
                    )
                };
                let _ = tx.send(AppEvent::BrowseDataLoaded(result));
            });
        }
    }

    /// Ctrl+←: switch to previous sheet within the same file (browse mode)
    pub(super) fn browse_prev_sheet_in_file(&mut self) {
        if self.file_list.is_empty() || self.browse_file_index >= self.file_list.len() {
            return;
        }
        let sheets_in_file = self.file_list[self.browse_file_index].sheets.len();
        if sheets_in_file <= 1 {
            return;
        }
        if self.browse_sheet_index > 0 {
            self.browse_sheet_index -= 1;
        } else {
            self.browse_sheet_index = sheets_in_file - 1;
        }
        self.load_browse_data();
    }

    /// Ctrl+→: switch to next sheet within the same file (browse mode)
    pub(super) fn browse_next_sheet_in_file(&mut self) {
        if self.file_list.is_empty() || self.browse_file_index >= self.file_list.len() {
            return;
        }
        let sheets_in_file = self.file_list[self.browse_file_index].sheets.len();
        if sheets_in_file <= 1 {
            return;
        }
        if self.browse_sheet_index + 1 < sheets_in_file {
            self.browse_sheet_index += 1;
        } else {
            self.browse_sheet_index = 0;
        }
        self.load_browse_data();
    }

    /// Ctrl+↑: switch to previous file's first sheet (browse mode)
    pub(super) fn browse_prev_file(&mut self) {
        if self.file_list.len() <= 1 {
            return;
        }
        if self.browse_file_index > 0 {
            self.browse_file_index -= 1;
        } else {
            self.browse_file_index = self.file_list.len() - 1;
        }
        self.browse_sheet_index = 0;
        self.load_browse_data();
    }

    /// Ctrl+↓: switch to next file's first sheet (browse mode)
    pub(super) fn browse_next_file(&mut self) {
        if self.file_list.len() <= 1 {
            return;
        }
        if self.browse_file_index + 1 < self.file_list.len() {
            self.browse_file_index += 1;
        } else {
            self.browse_file_index = 0;
        }
        self.browse_sheet_index = 0;
        self.load_browse_data();
    }
}
