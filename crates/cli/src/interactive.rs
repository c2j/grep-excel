//! Interactive SQL REPL (`-i` / `--interactive`) powered by rustyline.
//!
//! Multi-line editing: Up/Down arrows move the cursor across lines within a
//! statement when the buffer contains newlines (via rustyline). Enter inserts
//! a newline until the Validator detects `;` termination (or a dot command).

use anyhow::Result;
use crossterm::{execute, terminal};
use rustyline::highlight::Highlighter;
use rustyline::history::{DefaultHistory, History};
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{Config, Editor};
use rustyline_derive::{Completer, Helper, Hinter};
use std::borrow::Cow;
use unicode_width::UnicodeWidthStr;

use crate::engine::SearchEngine;
use crate::i18n;
use crate::types::SqlResult;

const PROMPT: &str = "$ ";
const CONTINUATION_PROMPT: &str = "> ";
const SQL_ROW_LIMIT: usize = 1000;
const HISTORY_MAX: usize = 500;

#[derive(Completer, Helper, Hinter)]
struct SqlHelper;

impl Validator for SqlHelper {
    fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        let trimmed = ctx.input().trim();
        if trimmed.is_empty() || trimmed.ends_with(';') || trimmed.starts_with('.') {
            Ok(ValidationResult::Valid(None))
        } else {
            Ok(ValidationResult::Incomplete)
        }
    }
}

impl Highlighter for SqlHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        if default {
            Cow::Borrowed(prompt)
        } else {
            Cow::Borrowed(CONTINUATION_PROMPT)
        }
    }
}

pub fn run<Engine: SearchEngine>(db: &mut Engine) -> Result<()> {
    print_welcome(db);

    let config = Config::builder()
        .history_ignore_dups(true)?
        .max_history_size(HISTORY_MAX)?
        .build();

    let mut rl: Editor<SqlHelper, DefaultHistory> = Editor::with_config(config)?;
    rl.set_helper(Some(SqlHelper));

    loop {
        match rl.readline(PROMPT) {
            Ok(input) => {
                let trimmed = input.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(&input);

                if trimmed.starts_with('.') {
                    if handle_dot_command(trimmed, db, rl.history())? {
                        println!("{}", i18n::repl_goodbye());
                        break;
                    }
                } else {
                    let sql = trimmed.trim_end_matches(';').trim();
                    if !sql.is_empty() {
                        execute_and_print(db, sql);
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => continue,
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("{}", i18n::repl_goodbye());
                break;
            }
            Err(e) => return Err(anyhow::anyhow!(e)),
        }
    }
    Ok(())
}

fn handle_dot_command<Engine: SearchEngine>(
    raw: &str,
    db: &mut Engine,
    history: &DefaultHistory,
) -> Result<bool> {
    let mut parts = raw.splitn(2, char::is_whitespace);
    let cmd = parts.next().unwrap_or("");

    match cmd {
        ".help" => println!("{}", i18n::repl_help()),
        ".exit" | ".quit" => return Ok(true),
        ".tables" | ".schema" => print_tables(db),
        ".files" => print_files(db),
        ".clear" | ".cls" => clear_screen(),
        ".history" => print_history(history),
        other => println!("{}", i18n::repl_unknown_dot(other)),
    }
    Ok(false)
}

fn print_tables<Engine: SearchEngine>(db: &Engine) {
    let aliases = db.list_table_aliases();
    if aliases.is_empty() {
        println!("{}", i18n::repl_no_files());
        return;
    }
    println!("{}", i18n::cli_list_tables_header());
    for a in &aliases {
        let cols = a.columns.join(", ");
        println!(
            "  {}",
            i18n::cli_list_tables_entry(&a.alias, &a.table_name, a.row_count, &cols)
        );
    }
    println!();
    println!("{}", i18n::cli_list_tables_footer(aliases.len()));
}

fn print_files<Engine: SearchEngine>(db: &Engine) {
    let files = db.list_files();
    if files.is_empty() {
        println!("{}", i18n::repl_no_files());
        return;
    }
    for f in &files {
        println!(
            "{}",
            i18n::cli_imported(&f.name, f.sheets.len(), f.total_rows)
        );
    }
}

fn print_history(history: &DefaultHistory) {
    if history.is_empty() {
        println!("{}", i18n::repl_history_empty());
        return;
    }
    for (i, entry) in history.iter().enumerate() {
        let preview: String = entry
            .chars()
            .map(|c| if c == '\n' { ' ' } else { c })
            .collect();
        let preview = preview.trim();
        let max = 80;
        let display: String = if preview.chars().count() > max {
            format!("{}…", preview.chars().take(max).collect::<String>())
        } else {
            preview.to_string()
        };
        println!("{:>4}  {}", i + 1, display);
    }
}

fn clear_screen() {
    let mut stdout = std::io::stdout();
    let _ = execute!(stdout, terminal::Clear(terminal::ClearType::All));
    let _ = execute!(stdout, crossterm::cursor::MoveTo(0, 0));
}

fn execute_and_print<Engine: SearchEngine>(db: &Engine, sql: &str) {
    let result = match db.execute_sql(sql, SQL_ROW_LIMIT) {
        Ok(r) => r,
        Err(e) => {
            let msg = e.to_string();
            let first_line = msg.lines().next().unwrap_or(&msg);
            println!("{}", i18n::repl_sql_error(first_line));
            return;
        }
    };
    print_sql_result(&result);
}

fn print_sql_result(result: &SqlResult) {
    if result.rows.is_empty() {
        println!("{}", i18n::cli_sql_no_results());
        return;
    }

    let widths: Vec<usize> = result
        .columns
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let name_w = UnicodeWidthStr::width(name.as_str());
            let max_data_w = result
                .rows
                .iter()
                .take(200)
                .filter_map(|r| r.get(i))
                .map(|c| UnicodeWidthStr::width(c.as_str()))
                .max()
                .unwrap_or(0);
            name_w.max(max_data_w).min(40)
        })
        .collect();

    let header_parts: Vec<String> = result
        .columns
        .iter()
        .enumerate()
        .map(|(i, name)| pad_to(name, widths[i]))
        .collect();
    println!("  {}", header_parts.join(" │ "));

    let sep_parts: Vec<String> = widths.iter().map(|&w| "─".repeat(w)).collect();
    println!("  {}", sep_parts.join("─┼─"));

    for row in &result.rows {
        let parts: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| pad_to(cell, widths.get(i).copied().unwrap_or(10)))
            .collect();
        println!("  {}", parts.join(" │ "));
    }

    println!();
    println!(
        "{}",
        i18n::repl_sql_summary(
            result.rows.len(),
            result.row_count,
            result.truncated,
            result.duration.as_millis()
        )
    );
}

fn pad_to(s: &str, width: usize) -> String {
    let sw = UnicodeWidthStr::width(s);
    if sw > width {
        if width == 0 {
            String::new()
        } else {
            let truncated: String = s.chars().take(width.saturating_sub(1)).collect();
            format!("{}…", truncated)
        }
    } else {
        let mut out = s.to_string();
        for _ in 0..width - sw {
            out.push(' ');
        }
        out
    }
}

fn print_welcome<Engine: SearchEngine>(db: &Engine) {
    println!("{}", i18n::repl_welcome(env!("CARGO_PKG_VERSION")));
    println!();
    println!("{}", i18n::repl_hint());
    println!();
    let aliases = db.list_table_aliases();
    if aliases.is_empty() {
        println!("{}", i18n::repl_no_files());
    } else {
        println!("{}", i18n::cli_list_tables_header());
        for a in &aliases {
            let cols = a.columns.join(", ");
            println!(
                "  {}",
                i18n::cli_list_tables_entry(&a.alias, &a.table_name, a.row_count, &cols)
            );
        }
        println!();
    }
}
