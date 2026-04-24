use crate::app::theme::theme;
use crate::types::{FileSample, SearchMode, SearchResult};
use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    Terminal,
};
use std::io;
use unicode_width::UnicodeWidthStr;

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len - 1).collect();
        format!("{}\u{2026}", truncated)
    }
}

pub fn pad_to_width(s: &str, width: usize) -> String {
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

pub fn compute_col_widths(
    col_names: &[String],
    results: &[SearchResult],
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

pub fn format_sample_preview(sample: &FileSample, max_width: u16) -> Vec<Line<'static>> {
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
            header_spans.push(Span::styled(SEP, Style::default().fg(theme().text_dim)));
        }
        header_spans.push(Span::styled(
            pad_to(h, widths[i]),
            Style::default()
                .fg(theme().label)
                .add_modifier(Modifier::BOLD),
        ));
    }
    lines.push(Line::from(header_spans));

    let sep_len = total - indent_w;
    let sep_str = format!("{}{}", INDENT, "─".repeat(sep_len));
    lines.push(Line::from(Span::styled(
        sep_str,
        Style::default().fg(theme().text_dim),
    )));

    for row in sample.rows.iter().take(MAX_ROWS) {
        let mut row_spans = vec![Span::styled(INDENT.to_string(), Style::default())];
        for i in 0..visible {
            if i > 0 {
                row_spans.push(Span::styled(SEP, Style::default().fg(theme().text_dim)));
            }
            let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
            row_spans.push(Span::styled(
                pad_to(cell, widths[i]),
                Style::default().fg(theme().text_dim),
            ));
        }
        lines.push(Line::from(row_spans));
    }

    lines
}

pub fn unicode_wrap(text: &str, max_width: usize) -> Vec<String> {
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

pub fn find_match_spans(mode: SearchMode, query: &str, value: &str) -> Vec<(usize, usize)> {
    crate::engine::find_match_spans(mode, query, value)
}

pub fn find_match_spans_cached(
    mode: SearchMode,
    value: &str,
    compiled_regex: Option<&regex::Regex>,
) -> Vec<(usize, usize)> {
    if value.is_empty() {
        return vec![];
    }
    match mode {
        SearchMode::FullText | SearchMode::Regex => {
            compiled_regex
                .map(|re| re.find_iter(value).map(|m| (m.start(), m.end())).collect())
                .unwrap_or_default()
        }
        SearchMode::ExactMatch => vec![(0, value.len())],
        SearchMode::Wildcard => vec![(0, value.len())],
    }
}

pub fn make_highlighted_spans(
    value: &str,
    spans: &[(usize, usize)],
    match_style: Style,
    normal_style: Style,
) -> Vec<Span<'static>> {
    if spans.is_empty() {
        return vec![Span::styled(value.to_string(), normal_style)];
    }
    let mut result = Vec::new();
    let mut last_end = 0;
    for &(start, end) in spans {
        if start > last_end && last_end < value.len() {
            let end_bound = start.min(value.len());
            result.push(Span::styled(
                value[last_end..end_bound].to_string(),
                normal_style,
            ));
        }
        if start < value.len() {
            let end_bound = end.min(value.len());
            result.push(Span::styled(
                value[start..end_bound].to_string(),
                match_style,
            ));
        }
        last_end = end.max(last_end);
    }
    if last_end < value.len() {
        result.push(Span::styled(
            value[last_end..].to_string(),
            normal_style,
        ));
    }
    result
}
