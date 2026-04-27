use super::render::*;
use super::theme::theme;
use super::{App, AppMode};
use crate::engine::{SearchMode, SearchResult};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Scrollbar,
        ScrollbarOrientation, Table,
    },
    Frame,
};
use unicode_width::UnicodeWidthStr;

impl App {
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
            AppMode::Normal => (crate::i18n::appmode_normal(), theme().highlight_match),
            AppMode::EditingSearch => (crate::i18n::appmode_search(), theme().highlight),
            AppMode::EditingColumn => (crate::i18n::appmode_column(), theme().highlight),
            AppMode::EditingAggregate => (crate::i18n::appmode_aggregate(), theme().highlight),
            AppMode::EditingSql => (crate::i18n::appmode_sql(), theme().info),
            AppMode::Help => (crate::i18n::appmode_help(), theme().info),
            AppMode::SelectFile => (crate::i18n::appmode_file(), theme().info),
            AppMode::DetailPanel => (crate::i18n::appmode_detail(), theme().info),
        };

        let file_count = self.file_list.len();
        let file_text = crate::i18n::files_loaded(file_count);

        let spans = vec![
            Span::styled(
                " grep-excel",
                Style::default()
                    .fg(theme().label)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" │ ", Style::default().fg(theme().text_dim)),
            Span::styled(format!("[{}]", mode_text), Style::default().fg(mode_color)),
            Span::styled(" │ ", Style::default().fg(theme().text_dim)),
            Span::styled(file_text, Style::default().fg(theme().text_dim)),
        ];

        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);
    }

    fn draw_tabs(&mut self, frame: &mut Frame, area: Rect) {
        let all_count = self.results.len();
        let mut tab_titles = vec![crate::i18n::tab_all(all_count)];

        let mut sheet_names: Vec<_> = self.results_by_sheet.keys().cloned().collect();
        sheet_names.sort();

        for sheet_name in &sheet_names {
            let count = self
                .results_by_sheet
                .get(sheet_name)
                .map(|v: &Vec<SearchResult>| v.len())
                .unwrap_or(0);
            tab_titles.push(format!("{}({})", sheet_name, count));
        }

        let mut spans: Vec<Span> = Vec::new();
        for (i, title) in tab_titles.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" │ ", Style::default().fg(theme().text_dim)));
            }
            if i == self.tab_state {
                spans.push(Span::styled(
                    format!(" ▶{} ", title),
                    Style::default()
                        .fg(theme().highlight)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    format!(" {} ", title),
                    Style::default().fg(theme().text_dim),
                ));
            }
        }

        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);
    }

    fn draw_search_bar(&mut self, frame: &mut Frame, area: Rect) {
        if self.mode == AppMode::EditingSql || !self.sql_input.value().is_empty() {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(7),
                    Constraint::Min(20),
                ])
                .split(area);

            let label_area = Rect {
                x: chunks[0].x,
                y: chunks[0].y + 1,
                width: chunks[0].width,
                height: 1,
            };
            let sql_label = Paragraph::new(crate::i18n::label_sql())
                .style(Style::default().fg(theme().info))
                .alignment(Alignment::Center);
            frame.render_widget(sql_label, label_area);

            let border_color = if self.mode == AppMode::EditingSql {
                theme().highlight
            } else {
                theme().text_dim
            };
            let scroll = self.sql_input.visual_scroll(chunks[1].width as usize);
            let sql_paragraph = Paragraph::new(self.sql_input.value())
                .style(Style::default().fg(theme().text))
                .scroll((0, scroll as u16))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .style(Style::default().fg(border_color)),
                );
            frame.render_widget(sql_paragraph, chunks[1]);

            if self.mode == AppMode::EditingSql {
                let cursor_pos = self.sql_input.visual_cursor();
                let cursor_x = chunks[1].x + (cursor_pos.saturating_sub(scroll)) as u16 + 1;
                let cursor_y = chunks[1].y + 1;
                frame.set_cursor_position((cursor_x, cursor_y));
            }
            return;
        }

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
                Constraint::Length(3),
                Constraint::Length(13),
                Constraint::Length(12),
            ])
            .split(area);

        let label_area = Rect {
            x: chunks[0].x,
            y: chunks[0].y + 1,
            width: chunks[0].width,
            height: 1,
        };
        let search_label = Paragraph::new(crate::i18n::label_search())
            .style(Style::default().fg(theme().label))
            .alignment(Alignment::Center);
        frame.render_widget(search_label, label_area);

        let search_border_color = if self.mode == AppMode::EditingSearch {
            theme().highlight
        } else {
            theme().text_dim
        };
        let scroll = self.search_input.visual_scroll(chunks[1].width as usize);
        let search_paragraph = Paragraph::new(self.search_input.value())
            .style(Style::default().fg(theme().text))
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
            .style(Style::default().fg(theme().text_dim))
            .alignment(Alignment::Center);
        frame.render_widget(sep, sep_area);

        let col_label_area = Rect {
            x: chunks[3].x,
            y: chunks[3].y + 1,
            width: chunks[3].width,
            height: 1,
        };
        let column_label = Paragraph::new(crate::i18n::label_column())
            .style(Style::default().fg(theme().label))
            .alignment(Alignment::Center);
        frame.render_widget(column_label, col_label_area);

        let column_border_color = if self.mode == AppMode::EditingColumn {
            theme().highlight
        } else {
            theme().text_dim
        };
        let col_scroll = self.column_input.visual_scroll(chunks[4].width as usize);
        let column_paragraph = Paragraph::new(self.column_input.value())
            .style(Style::default().fg(theme().text))
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
            .style(Style::default().fg(theme().text_dim))
            .alignment(Alignment::Center);
        frame.render_widget(sep2, sep2_area);

        let agg_label_area = Rect {
            x: chunks[6].x,
            y: chunks[6].y + 1,
            width: chunks[6].width,
            height: 1,
        };
        let aggregate_label = Paragraph::new(crate::i18n::label_aggregate())
            .style(Style::default().fg(theme().label))
            .alignment(Alignment::Center);
        frame.render_widget(aggregate_label, agg_label_area);

        let aggregate_border_color = if self.mode == AppMode::EditingAggregate {
            theme().highlight
        } else {
            theme().text_dim
        };
        let agg_scroll = self.aggregate_input.visual_scroll(chunks[7].width as usize);
        let aggregate_paragraph = Paragraph::new(self.aggregate_input.value())
            .style(Style::default().fg(theme().text))
            .scroll((0, agg_scroll as u16))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(aggregate_border_color)),
            );
        frame.render_widget(aggregate_paragraph, chunks[7]);

        if self.mode == AppMode::EditingAggregate {
            let cursor_pos = self.aggregate_input.visual_cursor();
            let cursor_x = chunks[7].x + (cursor_pos.saturating_sub(agg_scroll)) as u16 + 1;
            let cursor_y = chunks[7].y + 1;
            frame.set_cursor_position((cursor_x, cursor_y));
        }

        let sep3_area = Rect {
            x: chunks[8].x,
            y: chunks[8].y + 1,
            width: chunks[8].width,
            height: 1,
        };
        let sep3 = Paragraph::new("│")
            .style(Style::default().fg(theme().text_dim))
            .alignment(Alignment::Center);
        frame.render_widget(sep3, sep3_area);

        let (mode_str, mode_color) = match self.search_mode {
            SearchMode::FullText => (crate::i18n::mode_fulltext(), theme().highlight_match),
            SearchMode::ExactMatch => (crate::i18n::mode_exact(), theme().highlight),
            SearchMode::Wildcard => (crate::i18n::mode_wildcard(), theme().info),
            SearchMode::Regex => (crate::i18n::mode_regex(), theme().error),
        };
        let mode_area = Rect {
            x: chunks[9].x,
            y: chunks[9].y + 1,
            width: chunks[9].width,
            height: 1,
        };
        let mode_paragraph = Paragraph::new(mode_str)
            .style(Style::default().fg(mode_color).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
        frame.render_widget(mode_paragraph, mode_area);
    }

    fn draw_results_table(&mut self, frame: &mut Frame, area: Rect) {
        let has_sql = self.sql_result.is_some();
        let results_empty = self.results.is_empty();
        let search_empty = self.search_input.value().is_empty();
        if has_sql && results_empty && search_empty {
            let sql_result = self.sql_result.clone().unwrap();
            self.draw_sql_results(frame, area, &sql_result);
            return;
        }

        if self.mode == AppMode::DetailPanel {
            if self.is_flat_view_active() {
                let results = self.get_flat_current_results();
                if let Some(result) = results.get(self.flat_row_index) {
                    frame.render_widget(Clear, area);
                    self.draw_detail_panel(frame, area, result);
                }
            } else {
                let current: Vec<SearchResult> =
                    self.get_current_results().into_iter().cloned().collect();
                let selected = self.table_state.selected().unwrap_or(0);
                if let Some(result) = current.get(selected) {
                    frame.render_widget(Clear, area);
                    self.draw_detail_panel(frame, area, result);
                }
            }
            return;
        }

        let results: Vec<SearchResult> = self.get_current_results().into_iter().cloned().collect();

        if self.is_flat_view_active() {
            self.draw_flat_results(frame, area);
            return;
        }

        if results.is_empty() {
            let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

            let content = if self.loading {
                let char_idx = self.tick_count % spinner_chars.len();
                vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("{} {}", spinner_chars[char_idx], crate::i18n::status_loading()),
                        Style::default().fg(theme().label),
                    )),
                ]
            } else if self.results.is_empty() && self.search_input.value().is_empty() {
                if self.file_list.is_empty() {
                    vec![
                        Line::from(""),
                        Line::from(Span::styled(crate::i18n::empty_no_files(), Style::default().fg(theme().text_dim))),
                        Line::from(""),
                        Line::from(Span::styled(
                            crate::i18n::empty_open_hint(),
                            Style::default().fg(theme().text_dim),
                        )),
                        Line::from(Span::styled(
                            crate::i18n::empty_help_hint(),
                            Style::default().fg(theme().text_dim),
                        )),
                    ]
                } else {
                    let mut lines = vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            crate::i18n::empty_files_title(),
                            Style::default()
                                .fg(theme().label)
                                .add_modifier(Modifier::BOLD),
                        )),
                        Line::from(Span::styled(
                            "─────────────────────────────────────────",
                            Style::default().fg(theme().text_dim),
                        )),
                    ];

                    for file in &self.file_list {
                        let sheets_word = crate::i18n::empty_sheets(file.sheets.len());
                        lines.push(Line::from(vec![
                            Span::styled("  ", Style::default()),
                            Span::styled(
                                file.name.clone(),
                                Style::default()
                                    .fg(theme().text)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                format!(
                                    "  {} · {}",
                                    sheets_word,
                                    crate::i18n::file_rows(file.total_rows)
                                ),
                                Style::default().fg(theme().text_dim),
                            ),
                        ]));

                        for (i, (sheet_name, row_count)) in file.sheets.iter().enumerate() {
                            let prefix = if i == file.sheets.len() - 1 {
                                "    └── "
                            } else {
                                "    ├── "
                            };
                            lines.push(Line::from(vec![
                                Span::styled(prefix, Style::default().fg(theme().text_dim)),
                                Span::styled(sheet_name.clone(), Style::default().fg(theme().text_dim)),
                                Span::styled(
                                    format!("  {}", crate::i18n::empty_sheet_rows(*row_count)),
                                    Style::default().fg(theme().text_dim),
                                ),
                            ]));
                        }
                    }

                    lines.push(Line::from(""));

                    if let Some(sample) = self.file_list.iter().find_map(|f| f.sample.as_ref()) {
                        lines.push(Line::from(vec![Span::styled(
                            crate::i18n::preview(&sample.sheet_name),
                            Style::default().fg(theme().label),
                        )]));
                        lines.extend(format_sample_preview(sample, area.width));
                        lines.push(Line::from(""));
                    }

                    lines.push(Line::from(vec![
                        Span::styled("  ", Style::default().fg(theme().text_dim)),
                        Span::styled(crate::i18n::press_label(), Style::default().fg(theme().text_dim)),
                        Span::styled("[/]", Style::default().fg(theme().label)),
                        Span::styled(crate::i18n::empty_search_hint(), Style::default().fg(theme().text_dim)),
                        Span::styled("·  ", Style::default().fg(theme().text_dim)),
                        Span::styled("[?]", Style::default().fg(theme().label)),
                        Span::styled(crate::i18n::empty_help_word(), Style::default().fg(theme().text_dim)),
                    ]));

                    lines
                }
            } else {
                let query = self.search_input.value();
                if self.mode == AppMode::EditingSearch {
                    vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            crate::i18n::empty_query_label(query),
                            Style::default().fg(theme().text),
                        )),
                        Line::from(Span::styled(
                            crate::i18n::empty_enter_to_search(),
                            Style::default().fg(theme().text_dim),
                        )),
                    ]
                } else if self.stats.is_none() {
                    vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            crate::i18n::empty_query_label(query),
                            Style::default().fg(theme().text),
                        )),
                        Line::from(Span::styled(
                            crate::i18n::empty_edit_search_hint(),
                            Style::default().fg(theme().text_dim),
                        )),
                    ]
                } else {
                    let msg = if query.is_empty() {
                        crate::i18n::empty_no_results().to_string()
                    } else {
                        crate::i18n::empty_no_matches(query)
                    };
                    vec![
                        Line::from(""),
                        Line::from(Span::styled(msg, Style::default().fg(theme().text_dim))),
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
                (0..max_cols).map(|i| crate::i18n::col_auto_name(i + 1)).collect()
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
            ratatui::widgets::Cell::from(crate::i18n::col_file()).style(
                Style::default()
                    .fg(theme().label)
                    .add_modifier(Modifier::BOLD),
            ),
            ratatui::widgets::Cell::from(crate::i18n::col_sheet()).style(
                Style::default()
                    .fg(theme().label)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        for name in &visible_col_names {
            header_cells.push(
                ratatui::widgets::Cell::from(name.as_str()).style(
                    Style::default()
                        .fg(theme().label)
                        .add_modifier(Modifier::BOLD),
                ),
            );
        }
        let header_row = ratatui::widgets::Row::new(header_cells)
            .height(1)
            .bottom_margin(1);

        let compiled_regex = match self.search_mode {
            SearchMode::FullText => regex::Regex::new(&format!(
                "(?i){}",
                regex::escape(self.search_input.value())
            ))
            .ok(),
            SearchMode::Regex => {
                regex::Regex::new(&format!("(?i){}", self.search_input.value())).ok()
            }
            _ => None,
        };

        let rows: Vec<_> = results
            .iter()
            .map(|result| {
                let mut cells: Vec<ratatui::widgets::Cell<'_>> = vec![
                    ratatui::widgets::Cell::from(truncate_str(&result.file_name, 15))
                        .style(Style::default().fg(theme().text_dim)),
                    ratatui::widgets::Cell::from(truncate_str(&result.sheet_name, 12))
                        .style(Style::default().fg(theme().text_dim)),
                ];
                for (col_idx, cell_value) in result.row.iter().enumerate() {
                    if col_idx < col_offset {
                        continue;
                    }
                    if col_idx >= col_offset + visible_count {
                        break;
                    }
                    let is_matched = result.matched_columns.contains(&col_idx);
                    if is_matched {
                        let match_spans = find_match_spans_cached(
                            self.search_mode,
                            cell_value,
                            compiled_regex.as_ref(),
                        );
                        let spans = make_highlighted_spans(
                            cell_value,
                            &match_spans,
                            Style::default()
                                .fg(theme().highlight_match)
                                .add_modifier(Modifier::BOLD),
                            Style::default().fg(theme().text),
                        );
                        cells.push(ratatui::widgets::Cell::from(Line::from(spans)));
                    } else {
                        cells.push(
                            ratatui::widgets::Cell::from(cell_value.as_str())
                                .style(Style::default().fg(theme().text)),
                        );
                    }
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
            title_spans.push(Span::styled(" ◄ ", Style::default().fg(theme().highlight)));
        }
        if total_cols > 0 && col_offset + visible_count < total_cols {
            title_spans.push(Span::styled(" ► ", Style::default().fg(theme().highlight)));
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
                Style::default().fg(theme().highlight),
            )))
            .row_highlight_style(
                Style::default()
                    .fg(theme().highlight)
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

    fn draw_flat_results(&mut self, frame: &mut Frame, area: Rect) {
        let sheet_names = self.get_sorted_sheet_names();
        if sheet_names.is_empty() {
            return;
        }

        let block = Block::default().borders(Borders::ALL);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut current_y = inner.y;
        let max_y = inner.y + inner.height;
        let selected_sheet = self.flat_selected_sheet;

        for (sheet_idx, sheet_name) in sheet_names.iter().enumerate() {
            if current_y >= max_y {
                break;
            }

            let is_selected = sheet_idx == selected_sheet;
            let results: Vec<SearchResult> = self
                .results_by_sheet
                .get(sheet_name)
                .unwrap()
                .clone();

            if is_selected {
                let available_height = max_y.saturating_sub(current_y);
                if available_height < 4 {
                    break;
                }

                let sheet_area = Rect {
                    x: inner.x,
                    y: current_y,
                    width: inner.width,
                    height: available_height.min(inner.height),
                };

                self.draw_sheet_block(frame, sheet_area, sheet_name, &results, true);
                break;
            } else {
                let collapsed_height = 1u16;
                if current_y + collapsed_height > max_y {
                    break;
                }

                let sheet_area = Rect {
                    x: inner.x + 1,
                    y: current_y,
                    width: inner.width.saturating_sub(2),
                    height: collapsed_height,
                };

                let text = format!("▶ {} ({})", sheet_name, results.len());
                let paragraph =
                    Paragraph::new(text).style(Style::default().fg(theme().text_dim));
                frame.render_widget(paragraph, sheet_area);
                current_y += collapsed_height;
            }
        }
    }

    fn draw_sheet_block(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        sheet_name: &str,
        results: &[SearchResult],
        is_selected: bool,
    ) {
        if results.is_empty() {
            return;
        }

        let col_names: Vec<String> = results
            .iter()
            .find(|r| !r.col_names.is_empty())
            .map(|r| r.col_names.clone())
            .unwrap_or_else(|| {
                let max_cols = results.iter().map(|r| r.row.len()).max().unwrap_or(0);
                (0..max_cols).map(|i| crate::i18n::col_auto_name(i + 1)).collect()
            });

        let total_cols = col_names.len();
        let col_offset = self.get_flat_col_offset(sheet_name);

        let col_widths: Vec<u16> = if let Some(r) = results.first() {
            if r.col_widths.is_empty() {
                let computed = compute_col_widths(&col_names, results, 0, total_cols, usize::MAX);
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
        for &w in col_widths.iter().skip(col_offset) {
            if used_width + w > available_width {
                break;
            }
            used_width += w;
            visible_count += 1;
        }
        visible_count = visible_count.max(1);

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
            ratatui::widgets::Cell::from(crate::i18n::col_file()).style(
                Style::default()
                    .fg(theme().label)
                    .add_modifier(Modifier::BOLD),
            ),
            ratatui::widgets::Cell::from(crate::i18n::col_sheet()).style(
                Style::default()
                    .fg(theme().label)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        for name in &visible_col_names {
            header_cells.push(
                ratatui::widgets::Cell::from(name.as_str()).style(
                    Style::default()
                        .fg(theme().label)
                        .add_modifier(Modifier::BOLD),
                ),
            );
        }
        let header_row = ratatui::widgets::Row::new(header_cells)
            .height(1)
            .bottom_margin(1);

        let compiled_regex = match self.search_mode {
            SearchMode::FullText => regex::Regex::new(&format!(
                "(?i){}",
                regex::escape(self.search_input.value())
            ))
            .ok(),
            SearchMode::Regex => {
                regex::Regex::new(&format!("(?i){}", self.search_input.value())).ok()
            }
            _ => None,
        };

        let scroll_offset = if is_selected { self.flat_scroll_offset } else { 0 };
        let visible_row_count = area.height.saturating_sub(3) as usize;

        let rows: Vec<_> = results
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_row_count)
            .map(|(row_idx, result)| {
                let is_highlighted = is_selected && row_idx == self.flat_row_index;
                let mut cells: Vec<ratatui::widgets::Cell<'_>> = vec![
                    ratatui::widgets::Cell::from(truncate_str(&result.file_name, 15))
                        .style(Style::default().fg(theme().text_dim)),
                    ratatui::widgets::Cell::from(truncate_str(&result.sheet_name, 12))
                        .style(Style::default().fg(theme().text_dim)),
                ];
                for (col_idx, cell_value) in result.row.iter().enumerate() {
                    if col_idx < col_offset {
                        continue;
                    }
                    if col_idx >= col_offset + visible_count {
                        break;
                    }
                    let is_matched = result.matched_columns.contains(&col_idx);
                    if is_matched {
                        let match_spans = find_match_spans_cached(
                            self.search_mode,
                            cell_value,
                            compiled_regex.as_ref(),
                        );
                        let spans = make_highlighted_spans(
                            cell_value,
                            &match_spans,
                            Style::default()
                                .fg(theme().highlight_match)
                                .add_modifier(Modifier::BOLD),
                            Style::default().fg(theme().text),
                        );
                        cells.push(ratatui::widgets::Cell::from(Line::from(spans)));
                    } else {
                        cells.push(
                            ratatui::widgets::Cell::from(cell_value.as_str())
                                .style(Style::default().fg(theme().text)),
                        );
                    }
                }
                let mut row = ratatui::widgets::Row::new(cells).height(1);
                if is_highlighted {
                    row = row.style(
                        Style::default()
                            .fg(theme().highlight)
                            .add_modifier(Modifier::BOLD),
                    );
                }
                row
            })
            .collect();

        let mut constraints = vec![Constraint::Length(15), Constraint::Length(12)];
        for &w in &visible_col_widths {
            constraints.push(Constraint::Length(w));
        }
        if constraints.len() == 2 {
            constraints.push(Constraint::Min(10));
        }

        let mut title_spans = vec![Span::styled(
            format!(" {} ", sheet_name),
            Style::default().fg(theme().label).add_modifier(Modifier::BOLD),
        )];
        if col_offset > 0 {
            title_spans.push(Span::styled(" ◄ ", Style::default().fg(theme().highlight)));
        }
        if total_cols > 0 && col_offset + visible_count < total_cols {
            title_spans.push(Span::styled(" ► ", Style::default().fg(theme().highlight)));
        }
        let table_title = title_spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();

        let table = Table::new(rows, constraints)
            .header(header_row)
            .block(Block::default().borders(Borders::ALL).title(Span::styled(
                table_title,
                Style::default().fg(theme().label),
            )))
            .row_highlight_style(
                Style::default()
                    .fg(theme().highlight)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(table, area);
    }

    fn draw_sql_results(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        sql_result: &crate::types::SqlResult,
    ) {
        if sql_result.rows.is_empty() {
            let paragraph = Paragraph::new(Line::from(Span::styled(
                crate::i18n::sql_no_results(),
                Style::default().fg(theme().text_dim),
            )))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
            frame.render_widget(paragraph, area);
            return;
        }

        let col_names = &sql_result.columns;
        let total_cols = col_names.len();

        let col_widths: Vec<u16> = {
            let mut widths = vec![10u16; total_cols];
            for (i, name) in col_names.iter().enumerate() {
                widths[i] = UnicodeWidthStr::width(name.as_str()).clamp(4, 50) as u16;
            }
            for row in sql_result.rows.iter().take(100) {
                for (i, cell) in row.iter().enumerate() {
                    if i < total_cols {
                        let w = UnicodeWidthStr::width(cell.as_str()).clamp(4, 50) as u16;
                        if w > widths[i] {
                            widths[i] = w;
                        }
                    }
                }
            }
            widths
        };

        let available_width = area.width.saturating_sub(4);
        let mut visible_count = 0usize;
        let mut used: u16 = 0;
        for &w in &col_widths {
            if used + w > available_width {
                break;
            }
            used += w;
            visible_count += 1;
        }
        visible_count = visible_count.max(1);

        let mut header_cells: Vec<ratatui::widgets::Cell<'_>> = Vec::new();
        for name in col_names.iter().take(visible_count) {
            header_cells.push(
                ratatui::widgets::Cell::from(name.as_str()).style(
                    Style::default()
                        .fg(theme().label)
                        .add_modifier(Modifier::BOLD),
                ),
            );
        }
        let header_row = ratatui::widgets::Row::new(header_cells)
            .height(1)
            .bottom_margin(1);

        let rows: Vec<_> = sql_result
            .rows
            .iter()
            .map(|row| {
                let cells: Vec<ratatui::widgets::Cell<'_>> = row
                    .iter()
                    .take(visible_count)
                    .map(|cell| {
                        ratatui::widgets::Cell::from(truncate_str(cell, 50))
                            .style(Style::default().fg(theme().text))
                    })
                    .collect();
                ratatui::widgets::Row::new(cells).height(1)
            })
            .collect();

        let constraints: Vec<Constraint> = col_widths
            .iter()
            .take(visible_count)
            .map(|&w| Constraint::Length(w))
            .collect();

        let table = Table::new(rows, constraints)
            .header(header_row)
            .block(Block::default().borders(Borders::ALL).title(Span::styled(
                crate::i18n::sql_results_title(sql_result.row_count),
                Style::default().fg(theme().label),
            )))
            .row_highlight_style(
                Style::default()
                    .fg(theme().highlight)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn draw_detail_panel(&self, frame: &mut Frame, area: Rect, result: &SearchResult) {
        let compiled_regex = match self.search_mode {
            SearchMode::FullText => regex::Regex::new(&format!(
                "(?i){}",
                regex::escape(self.search_input.value())
            ))
            .ok(),
            SearchMode::Regex => {
                regex::Regex::new(&format!("(?i){}", self.search_input.value())).ok()
            }
            _ => None,
        };

        let mut lines: Vec<Line<'_>> = Vec::new();

        lines.push(Line::from(vec![
            Span::styled(crate::i18n::detail_file_label(), Style::default().fg(theme().text_dim)),
            Span::styled(
                &result.file_name,
                Style::default()
                    .fg(theme().label)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled(crate::i18n::detail_sheet_label(), Style::default().fg(theme().text_dim)),
            Span::styled(&result.sheet_name, Style::default().fg(theme().label)),
        ]));
        lines.push(Line::from(Span::styled(
            "─".repeat(area.width.saturating_sub(2) as usize),
            Style::default().fg(theme().text_dim),
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
                    .fg(theme().highlight_match)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme().highlight)
            };

            let normal_style = Style::default().fg(theme().text);
            let match_style = Style::default()
                .fg(theme().highlight_match)
                .add_modifier(Modifier::BOLD);

            let name_display = pad_to_width(name, max_name_width);
            let continuation_indent = " ".repeat(prefix_width);

            let match_spans = if is_matched {
                find_match_spans_cached(self.search_mode, value, compiled_regex.as_ref())
            } else {
                vec![]
            };

            let segments: Vec<&str> = if value.contains('\n') {
                value.split('\n').collect()
            } else {
                vec![value.as_str()]
            };

            let mut is_first_line = true;
            for segment in segments {
                if value_width == 0 || UnicodeWidthStr::width(segment) <= value_width {
                    let value_spans = if is_matched && !match_spans.is_empty() {
                        let seg_start = segment.as_ptr() as usize - value.as_ptr() as usize;
                        let seg_end = seg_start + segment.len();
                        let local_spans: Vec<(usize, usize)> = match_spans
                            .iter()
                            .filter_map(|&(s, e)| {
                                if s >= seg_end || e <= seg_start {
                                    None
                                } else {
                                    Some((s.max(seg_start) - seg_start, e.min(seg_end) - seg_start))
                                }
                            })
                            .collect();
                        make_highlighted_spans(segment, &local_spans, match_style, normal_style)
                    } else if is_matched {
                        vec![Span::styled(segment.to_string(), match_style)]
                    } else {
                        vec![Span::styled(segment.to_string(), normal_style)]
                    };

                    if is_first_line {
                        let mut line_spans = vec![
                            Span::styled(format!("{} ", name_display), name_style),
                            Span::styled("│ ", Style::default().fg(theme().text_dim)),
                        ];
                        line_spans.extend(value_spans);
                        lines.push(Line::from(line_spans));
                        is_first_line = false;
                    } else {
                        let mut line_spans = vec![
                            Span::styled(format!("{} ", continuation_indent), Style::default()),
                            Span::styled("  ", Style::default()),
                        ];
                        line_spans.extend(value_spans);
                        lines.push(Line::from(line_spans));
                    }
                } else {
                    let wrapped = unicode_wrap(segment, value_width);
                    let mut byte_offset = 0;
                    for (wi, chunk) in wrapped.iter().enumerate() {
                        let chunk_byte_len = chunk.len();
                        let chunk_spans = if is_matched && !match_spans.is_empty() {
                            let chunk_start = byte_offset;
                            let chunk_end = byte_offset + chunk_byte_len;
                            let local_spans: Vec<(usize, usize)> = match_spans
                                .iter()
                                .filter_map(|&(s, e)| {
                                    if s >= chunk_end || e <= chunk_start {
                                        None
                                    } else {
                                        Some((s.max(chunk_start) - chunk_start, e.min(chunk_end) - chunk_start))
                                    }
                                })
                                .collect();
                            make_highlighted_spans(chunk, &local_spans, match_style, normal_style)
                        } else if is_matched {
                            vec![Span::styled(chunk.clone(), match_style)]
                        } else {
                            vec![Span::styled(chunk.clone(), normal_style)]
                        };
                        byte_offset += chunk_byte_len;

                        if wi == 0 && is_first_line {
                            let mut line_spans = vec![
                                Span::styled(format!("{} ", name_display), name_style),
                                Span::styled("│ ", Style::default().fg(theme().text_dim)),
                            ];
                            line_spans.extend(chunk_spans);
                            lines.push(Line::from(line_spans));
                            is_first_line = false;
                        } else {
                            let mut line_spans = vec![
                                Span::styled(format!("{} ", continuation_indent), Style::default()),
                                Span::styled("  ", Style::default()),
                            ];
                            line_spans.extend(chunk_spans);
                            lines.push(Line::from(line_spans));
                        }
                    }
                }
            }

            if is_first_line {
                lines.push(Line::from(vec![
                    Span::styled(format!("{} ", name_display), name_style),
                    Span::styled("│ ", Style::default().fg(theme().text_dim)),
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
                    crate::i18n::detail_title(),
                    Style::default()
                        .fg(theme().label)
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
                Style::default().fg(theme().error),
            ))
        } else if self.loading {
            let char_idx = self.tick_count % spinner_chars.len();
            Line::from(Span::styled(
                format!(" {} {}", spinner_chars[char_idx], crate::i18n::status_loading()),
                Style::default().fg(theme().label),
            ))
        } else if let Some(stats) = &self.stats {
            let current_results = self.get_current_results();
            let selected = self.table_state.selected().unwrap_or(0);
            let row_indicator = if current_results.is_empty() {
                crate::i18n::status_row_empty().to_string()
            } else {
                crate::i18n::status_row_indicator(selected + 1, current_results.len())
            };

            let view_indicator = if self.is_flat_view_active() {
                format!(" [{}]", crate::i18n::status_view_flat())
            } else if self.tab_state == 0 && self.results_by_sheet.len() > 1 {
                format!(" [{}]", crate::i18n::status_view_table())
            } else {
                String::new()
            };

            let mut sheet_stats: Vec<_> = stats.matches_per_sheet.iter().collect();
            sheet_stats.sort_by_key(|(k, _)| *k);
            let sheet_info: Vec<_> = sheet_stats
                .iter()
                .map(|(k, v)| format!("{}({})", k, v))
                .collect();

            let aggregate_indicator = if let Some(agg_stats) = &self.aggregate_stats {
                let mut sorted_counts: Vec<_> = agg_stats.counts.iter().collect();
                sorted_counts.sort_by(|a, b| b.1.cmp(a.1));
                let count_parts: Vec<String> = sorted_counts
                    .into_iter()
                    .take(5)
                    .map(|(k, v)| format!("{}({})", k, v))
                    .collect();
                format!(" | {}: {}", agg_stats.column, count_parts.join(" "))
            } else {
                String::new()
            };

            let total_cols = self.get_current_col_count();
            let col_start = self.col_offset + 1;
            let col_end = total_cols.min(self.col_offset + self.visible_col_count.max(1));
            let col_indicator = if total_cols == 0 {
                String::new()
            } else {
                crate::i18n::status_col_range(col_start, col_end, total_cols)
            };

            Line::from(vec![
                Span::styled(
                    crate::i18n::status_matches_label(stats.total_matches, stats.total_rows_searched),
                    Style::default().fg(theme().text),
                ),
                Span::styled(" │ ", Style::default().fg(theme().text_dim)),
                Span::styled(
                    format!("{:.2}s", stats.search_duration.as_secs_f64()),
                    Style::default().fg(theme().text),
                ),
                Span::styled(" │ ", Style::default().fg(theme().text_dim)),
                Span::styled(row_indicator, Style::default().fg(theme().text)),
                Span::styled(col_indicator, Style::default().fg(theme().text_dim)),
                Span::styled(view_indicator, Style::default().fg(theme().info)),
                Span::styled(" │ ", Style::default().fg(theme().text_dim)),
                Span::styled(sheet_info.join(" "), Style::default().fg(theme().text_dim)),
                Span::styled(aggregate_indicator, Style::default().fg(theme().highlight_match)),
            ])
        } else {
            let file_count = self.file_list.len();
            let file_text = crate::i18n::files_loaded(file_count);
            Line::from(Span::styled(
                format!(" {}", file_text),
                Style::default().fg(theme().text_dim),
            ))
        };

        let stats_paragraph = Paragraph::new(stats_line);
        frame.render_widget(stats_paragraph, chunks[0]);

        let hints: Vec<Span> = match self.mode {
            AppMode::Normal => vec![
                Span::styled(" [/]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_search()),
                Span::styled("[c]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_col()),
                Span::styled("[a]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_aggregate()),
                Span::styled("[Tab]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_mode()),
                Span::styled("[o]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_open()),
                Span::styled("[s]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_export()),
                Span::styled("[S]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_sql()),
                Span::styled("[d]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_clear()),
                Span::styled("[v]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_view()),
                Span::styled("[?]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_help()),
                Span::styled("[q]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_quit()),
            ],
            AppMode::EditingSearch => vec![
                Span::styled(" [Enter]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_execute()),
                Span::styled("[Esc]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_cancel()),
                Span::styled("[Tab]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_toggle_mode()),
            ],
            AppMode::EditingColumn => vec![
                Span::styled(" [Enter]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_confirm()),
                Span::styled("[Esc]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_cancel_short()),
            ],
            AppMode::EditingAggregate => vec![
                Span::styled(" [Enter]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_confirm()),
                Span::styled("[Esc]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_cancel_short()),
            ],
            AppMode::EditingSql => vec![
                Span::styled(" [Enter]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_execute()),
                Span::styled("[Esc]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_cancel()),
            ],
            AppMode::Help => vec![
                Span::styled(" [Esc/?/h]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_close_help()),
            ],
            AppMode::SelectFile => vec![
                Span::styled(" [↑/k]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_up()),
                Span::styled("[↓/j]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_down()),
                Span::styled("[Enter]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_select()),
                Span::styled("[Esc]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_close()),
            ],
            AppMode::DetailPanel => vec![
                Span::styled(" [Enter/Esc]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_close()),
                Span::styled("  [↑/k]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_scroll_up()),
                Span::styled("[↓/j]", Style::default().fg(theme().label)),
                Span::raw(crate::i18n::hint_scroll_down()),
            ],
        };

        let hints_paragraph = Paragraph::new(Line::from(hints));
        frame.render_widget(hints_paragraph, chunks[1]);
    }

    fn draw_help_popup(&mut self, frame: &mut Frame) {
        let area = centered_rect(55, 75, frame.area());

        let group_style = Style::default()
            .fg(theme().label)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        let key_style = Style::default().fg(theme().highlight);
        let desc_style = Style::default().fg(theme().text);
        let sep_style = Style::default().fg(theme().text_dim);
        let footer_style = Style::default()
            .fg(theme().text_dim)
            .add_modifier(Modifier::ITALIC);

        let help_text = vec![
            Line::from(""),
            Line::from(Span::styled(crate::i18n::help_group_nav(), group_style)),
            Line::from(vec![
                Span::styled("    ↑/k   ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_nav_up(), desc_style),
            ]),
            Line::from(vec![
                Span::styled("    ↓/j   ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_nav_down(), desc_style),
            ]),
            Line::from(vec![
                Span::styled("    g     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_nav_top(), desc_style),
            ]),
            Line::from(vec![
                Span::styled("    G     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_nav_bottom(), desc_style),
            ]),
            Line::from(vec![
                Span::styled("    ←/→   ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_nav_scroll_cols(), desc_style),
            ]),
            Line::from(vec![
                Span::styled("    1-9   ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_nav_tab(), desc_style),
            ]),
            Line::from(""),
            Line::from(Span::styled(crate::i18n::help_group_search(), group_style)),
            Line::from(vec![
                Span::styled("    /     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_search_input(), desc_style),
            ]),
            Line::from(vec![
                Span::styled("    c     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_search_col(), desc_style),
            ]),
            Line::from(vec![
                Span::styled("    S     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_search_sql(), desc_style),
            ]),
            Line::from(vec![
                Span::styled("    Tab   ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_search_toggle(), desc_style),
            ]),
            Line::from(vec![
                Span::styled("    Enter ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_search_exec(), desc_style),
            ]),
            Line::from(""),
            Line::from(Span::styled(crate::i18n::help_group_general(), group_style)),
            Line::from(vec![
                Span::styled("    o     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_gen_open(), desc_style),
            ]),
            Line::from(vec![
                Span::styled("    d     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_gen_clear(), desc_style),
            ]),
            Line::from(vec![
                Span::styled("    s     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_gen_export(), desc_style),
            ]),
            Line::from(vec![
                Span::styled("    n     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_gen_more(), desc_style),
            ]),
            Line::from(vec![
                Span::styled("    ?     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_gen_toggle_help(), desc_style),
            ]),
            Line::from(vec![
                Span::styled("    q     ", key_style),
                Span::styled("···  ", sep_style),
                Span::styled(crate::i18n::help_gen_quit(), desc_style),
            ]),
            Line::from(""),
            Line::from(Span::styled(crate::i18n::help_close_hint(), footer_style)),
        ];

        let help_block = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(crate::i18n::help_title()),
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
                        .fg(theme().highlight)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme().text)
                };
                let meta = crate::i18n::filelist_meta(file.sheets.len(), file.total_rows);

                let lines = vec![
                    Line::from(vec![
                        Span::styled(prefix, name_style),
                        Span::styled(file.name.clone(), name_style),
                    ]),
                    Line::from(Span::styled(meta, Style::default().fg(theme().text_dim))),
                ];
                ListItem::new(lines)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(crate::i18n::filelist_title()),
            )
            .highlight_style(Style::default());

        frame.render_widget(Clear, area);
        frame.render_stateful_widget(list, area, &mut self.file_list_state);
    }

    pub(super) fn export_results(&mut self) {
        if self.results.is_empty() {
            self.status_message = crate::i18n::export_no_results().to_string();
            return;
        }

        #[cfg(feature = "file-dialog")]
        {
            let _ = restore_terminal();
            let file = rfd::FileDialog::new()
                .add_filter("CSV Files", &["csv"])
                .set_title(crate::i18n::export_dialog_title())
                .save_file();
            let _ = init_terminal();

            if let Some(path) = file {
                match crate::engine::export_results_csv(&self.results, &path) {
                    Ok(()) => {
                        self.status_message = crate::i18n::export_done(&path.display().to_string());
                    }
                    Err(e) => {
                        self.error_message = Some(crate::i18n::export_error(&e.to_string()));
                        self.status_message = crate::i18n::export_failed().to_string();
                    }
                }
            }
        }

        #[cfg(not(feature = "file-dialog"))]
        {
            self.status_message = crate::i18n::export_no_dialog().to_string();
        }
    }

    #[cfg(feature = "file-dialog")]
    pub(super) fn open_file_dialog(&mut self) {
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
}
