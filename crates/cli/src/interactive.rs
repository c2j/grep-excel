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
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

use crate::engine::SearchEngine;
use crate::i18n;
use crate::types::SqlResult;

const PROMPT: &str = "$ ";
const CONTINUATION_PROMPT: &str = "> ";
const SQL_ROW_LIMIT: usize = 1000;
const HISTORY_MAX: usize = 500;

// ---------------------------------------------------------------------------
// OutputTarget — controls where SQL results go
// ---------------------------------------------------------------------------

/// Where to send SQL query results. Only SQL results are affected; dot
/// commands (`.tables`, `.help`, …) always go to the terminal.
enum OutputTarget {
    Stdout,
    File(BufWriter<File>),
}

impl OutputTarget {
    /// Returns `true` when output is currently redirected to a file.
    fn is_file(&self) -> bool {
        matches!(self, OutputTarget::File(_))
    }
}

// ---------------------------------------------------------------------------
// rustyline glue
// ---------------------------------------------------------------------------

fn history_path() -> Option<PathBuf> {
    let base = dirs::state_dir().or_else(dirs::config_dir)?;
    Some(base.join("grep-excel").join("history.txt"))
}

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

// ---------------------------------------------------------------------------
// Main REPL loop
// ---------------------------------------------------------------------------

pub fn run<Engine: SearchEngine>(db: &mut Engine, no_history: bool) -> Result<()> {
    print_welcome(db);

    let config = Config::builder()
        .history_ignore_dups(true)?
        .max_history_size(HISTORY_MAX)?
        .build();

    let mut rl: Editor<SqlHelper, DefaultHistory> = Editor::with_config(config)?;
    rl.set_helper(Some(SqlHelper));

    let hist_path: Option<PathBuf> = if no_history {
        None
    } else {
        history_path().inspect(|p| {
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
        })
    };
    if let Some(p) = &hist_path {
        let _ = rl.load_history(p);
    }

    // Per-session mutable state
    let mut output = OutputTarget::Stdout;
    let mut last_result: Option<SqlResult> = None;

    loop {
        match rl.readline(PROMPT) {
            Ok(input) => {
                let trimmed = input.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(&input);
                if let Some(p) = &hist_path {
                    let _ = rl.save_history(p);
                }

                    if trimmed.starts_with('.') {
                    if handle_dot_command(
                        trimmed,
                        db,
                        rl.history(),
                        &mut output,
                        &mut last_result,
                    )? {
                        // Close open output file on exit — dropping the
                        // BufWriter flushes buffered data.
                        if let OutputTarget::File(mut f) =
                            std::mem::replace(&mut output, OutputTarget::Stdout)
                        {
                            let _ = f.flush();
                        }
                        println!("{}", i18n::repl_goodbye());
                        break;
                    }
                } else {
                    let sql = trimmed.trim_end_matches(';').trim();
                    if !sql.is_empty() {
                        execute_and_print(db, sql, &mut output, &mut last_result);
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => continue,
            Err(rustyline::error::ReadlineError::Eof) => {
                if let OutputTarget::File(mut f) =
                    std::mem::replace(&mut output, OutputTarget::Stdout)
                {
                    let _ = f.flush();
                }
                println!("{}", i18n::repl_goodbye());
                break;
            }
            Err(e) => return Err(anyhow::anyhow!(e)),
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Dot-command dispatch
// ---------------------------------------------------------------------------

fn handle_dot_command<Engine: SearchEngine>(
    raw: &str,
    db: &mut Engine,
    history: &DefaultHistory,
    output: &mut OutputTarget,
    last_result: &mut Option<SqlResult>,
) -> Result<bool> {
    let mut parts = raw.splitn(2, char::is_whitespace);
    let cmd = parts.next().unwrap_or("");
    let args = parts.next().unwrap_or("").trim();

    match cmd {
        ".help" => println!("{}", i18n::repl_help()),
        ".exit" | ".quit" => return Ok(true),
        ".tables" | ".schema" => print_tables(db),
        ".files" => print_files(db),
        ".clear" | ".cls" => clear_screen(),
        ".history" => print_history(history),
        ".output" => handle_output_command(args, output),
        ".save" => handle_save_command(args, last_result),
        ".let" => handle_let_command(args, db),
        ".drop" => handle_drop_command(args, db),
        other => println!("{}", i18n::repl_unknown_dot(other)),
    }
    Ok(false)
}

// ---------------------------------------------------------------------------
// .output — continuous redirection
// ---------------------------------------------------------------------------

fn handle_output_command(args: &str, output: &mut OutputTarget) {
    let target = match parse_output_target(args) {
        Ok(t) => t,
        Err(e) => {
            println!("{}", i18n::repl_output_error(&e));
            return;
        }
    };

    // Close previous file if any
    if output.is_file() {
        *output = OutputTarget::Stdout;
    }

    match target {
        ParsedOutput::Stdout => {
            println!("{}", i18n::repl_output_off());
            *output = OutputTarget::Stdout;
        }
        ParsedOutput::File(path) => {
            match File::create(&path) {
                Ok(f) => {
                    println!("{}", i18n::repl_output_on(&path));
                    *output = OutputTarget::File(BufWriter::new(f));
                }
                Err(e) => {
                    println!("{}", i18n::repl_output_open_error(&path, &e.to_string()));
                }
            }
        }
    }
}

enum ParsedOutput {
    Stdout,
    File(String),
}

fn parse_output_target(args: &str) -> Result<ParsedOutput, String> {
    let args = args.trim();
    if args.is_empty() || args.eq_ignore_ascii_case("stdout") {
        Ok(ParsedOutput::Stdout)
    } else {
        // Accept any non-empty string as a file path
        Ok(ParsedOutput::File(args.to_string()))
    }
}

// ---------------------------------------------------------------------------
// .save — one-shot save of last result
// ---------------------------------------------------------------------------

fn handle_save_command(args: &str, last_result: &Option<SqlResult>) {
    let Some(result) = last_result else {
        println!("{}", i18n::repl_save_no_result());
        return;
    };

    let (path, format) = parse_save_args(args);
    let fmt = format.as_deref().unwrap_or("csv");

    if result.truncated {
        println!("{}", i18n::repl_save_truncated());
    }

    match write_save_file(result, &path, fmt) {
        Ok(()) => println!("{}", i18n::repl_save_done(&path, result.rows.len())),
        Err(e) => println!("{}", i18n::repl_save_error(&path, &e.to_string())),
    }
}

// ---------------------------------------------------------------------------
// .let — materialize query as temp table
// ---------------------------------------------------------------------------

/// Parse a `.let` command line: returns `(name, sql)`.
/// Expected format: `.let <name> AS <sql...>`
fn parse_let_args(args: &str) -> Option<(String, String)> {
    let args = args.trim();
    let lower = args.to_ascii_lowercase();
    // Find " as " as delimiter (case-insensitive)
    let as_pos = lower.find(" as ")?;
    let name = args[..as_pos].trim();
    let sql = args[as_pos + 4..].trim();
    if name.is_empty() || sql.is_empty() {
        return None;
    }
    Some((name.to_string(), sql.to_string()))
}

fn handle_let_command<Engine: SearchEngine>(args: &str, db: &mut Engine) {
    let (name, sql) = match parse_let_args(args) {
        Some(v) => v,
        None => {
            println!("{}", i18n::repl_let_usage());
            return;
        }
    };
    match db.materialize_query(&name, &sql, true, None) {
        Ok(info) => println!("{}", i18n::repl_let_ok(&info.name, info.row_count, info.columns.len())),
        Err(e) => println!("{}", i18n::repl_let_error(&name, &e.to_string())),
    }
}

// ---------------------------------------------------------------------------
// .drop — drop temp table
// ---------------------------------------------------------------------------

fn handle_drop_command<Engine: SearchEngine>(args: &str, db: &mut Engine) {
    let name = args.trim();
    if name.is_empty() {
        println!("{}", i18n::repl_drop_usage());
        return;
    }
    match db.drop_temp_table(name) {
        Ok(()) => println!("{}", i18n::repl_drop_ok(name)),
        Err(e) => println!("{}", i18n::repl_drop_error(name, &e.to_string())),
    }
}

// ---------------------------------------------------------------------------
// .save — one-shot save of last result
// ---------------------------------------------------------------------------

/// Extract `(path, format)` from args like `"result.csv csv"`.
fn parse_save_args(args: &str) -> (String, Option<String>) {
    let args = args.trim();
    if args.is_empty() {
        return (String::new(), None);
    }
    let mut parts = args.splitn(2, char::is_whitespace);
    let path = parts.next().unwrap_or("").to_string();
    let format = parts.next().map(|s| s.trim().to_lowercase());
    (path, format)
}

fn write_save_file(result: &SqlResult, path: &str, format: &str) -> Result<()> {
    if path.is_empty() {
        anyhow::bail!("no output path specified");
    }
    let mut f = BufWriter::new(File::create(path)?);
    match format {
        "csv" => write_csv(&mut f, result),
        "json" => write_json(&mut f, result),
        "tsv" => write_tsv(&mut f, result),
        "table" => write_table(&mut f, result),
        unknown => {
            eprintln!(
                "Unknown format '{}', falling back to csv. Supported: csv, json, tsv, table",
                unknown
            );
            write_csv(&mut f, result)
        }
    }?;
    f.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// SQL execution + output routing
// ---------------------------------------------------------------------------

fn execute_and_print<Engine: SearchEngine>(
    db: &Engine,
    sql: &str,
    output: &mut OutputTarget,
    last_result: &mut Option<SqlResult>,
) {
    let limit = if output.is_file() {
        usize::MAX
    } else {
        SQL_ROW_LIMIT
    };
    let result = match db.execute_sql(sql, limit) {
        Ok(r) => r,
        Err(e) => {
            let msg = e.to_string();
            let first_line = msg.lines().next().unwrap_or(&msg);
            println!("{}", i18n::repl_sql_error(first_line));
            return;
        }
    };

    // Cache for .save (only if non-empty)
    let has_rows = !result.rows.is_empty();
    *last_result = Some(result);

    // Route output
    match output {
        OutputTarget::Stdout => {
            let cached = last_result.as_ref().unwrap();
            print_sql_result(cached);
        }
        OutputTarget::File(writer) => {
            let cached = last_result.as_ref().unwrap();
            if let Err(e) = write_csv(writer, cached) {
                println!("{}", i18n::repl_output_write_error(&e.to_string()));
                return;
            }
            if let Err(e) = writer.flush() {
                println!("{}", i18n::repl_output_write_error(&e.to_string()));
                return;
            }
            if has_rows {
                println!(
                    "{}",
                    i18n::repl_sql_summary(
                        cached.rows.len(),
                        cached.row_count,
                        cached.truncated,
                        cached.duration.as_millis()
                    )
                );
            } else {
                println!("{}", i18n::cli_sql_no_results());
            }
        }
    }
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

// ---------------------------------------------------------------------------
// Format writers (used by both .output and .save)
// ---------------------------------------------------------------------------

fn write_csv<W: Write>(w: &mut W, result: &SqlResult) -> Result<()> {
    let mut writer = csv::Writer::from_writer(w);
    writer.write_record(&result.columns)?;
    for row in &result.rows {
        writer.write_record(row)?;
    }
    writer.flush()?;
    Ok(())
}

fn write_json<W: Write>(w: &mut W, result: &SqlResult) -> Result<()> {
    let json_rows: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            let map: serde_json::Map<String, serde_json::Value> = result
                .columns
                .iter()
                .zip(row.iter())
                .map(|(col, val)| (col.clone(), serde_json::Value::String(val.clone())))
                .collect();
            serde_json::Value::Object(map)
        })
        .collect();

    let output = serde_json::to_string_pretty(&json_rows)?;
    w.write_all(output.as_bytes())?;
    w.write_all(b"\n")?;
    Ok(())
}

fn write_tsv<W: Write>(w: &mut W, result: &SqlResult) -> Result<()> {
    writeln!(w, "{}", result.columns.join("\t"))?;
    for row in &result.rows {
        writeln!(w, "{}", row.join("\t"))?;
    }
    Ok(())
}

fn write_table<W: Write>(w: &mut W, result: &SqlResult) -> Result<()> {
    if result.rows.is_empty() {
        writeln!(w, "(empty)")?;
        return Ok(());
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

    // Header
    let header_parts: Vec<String> = result
        .columns
        .iter()
        .enumerate()
        .map(|(i, name)| pad_to(name, widths[i]))
        .collect();
    writeln!(w, "{}", header_parts.join(" | "))?;

    // Separator
    let sep_parts: Vec<String> = widths.iter().map(|&w| "-".repeat(w)).collect();
    writeln!(w, "{}", sep_parts.join("-+-"))?;

    // Data rows
    for row in &result.rows {
        let parts: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| pad_to(cell, widths.get(i).copied().unwrap_or(10)))
            .collect();
        writeln!(w, "{}", parts.join(" | "))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Dot-command helpers (unchanged from original)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SqlResult;
    use std::time::Duration;

    // -- parse_output_target -------------------------------------------------

    #[test]
    fn test_parse_output_stdout_empty() {
        let result = parse_output_target("").unwrap();
        assert!(matches!(result, ParsedOutput::Stdout));
    }

    #[test]
    fn test_parse_output_stdout_explicit() {
        let result = parse_output_target("stdout").unwrap();
        assert!(matches!(result, ParsedOutput::Stdout));
    }

    #[test]
    fn test_parse_output_stdout_case_insensitive() {
        let result = parse_output_target("STDOUT").unwrap();
        assert!(matches!(result, ParsedOutput::Stdout));
    }

    #[test]
    fn test_parse_output_file() {
        let result = parse_output_target("results.csv").unwrap();
        assert!(matches!(result, ParsedOutput::File(ref p) if p == "results.csv"));
    }

    #[test]
    fn test_parse_output_file_with_spaces() {
        let result = parse_output_target("/path/to/my file.csv").unwrap();
        assert!(matches!(result, ParsedOutput::File(ref p) if p == "/path/to/my file.csv"));
    }

    // -- parse_save_args -----------------------------------------------------

    #[test]
    fn test_parse_save_empty() {
        let (path, format) = parse_save_args("");
        assert_eq!(path, "");
        assert!(format.is_none());
    }

    #[test]
    fn test_parse_save_path_only() {
        let (path, format) = parse_save_args("result.csv");
        assert_eq!(path, "result.csv");
        assert!(format.is_none());
    }

    #[test]
    fn test_parse_save_path_and_format() {
        let (path, format) = parse_save_args("result.json json");
        assert_eq!(path, "result.json");
        assert_eq!(format.unwrap(), "json");
    }

    #[test]
    fn test_parse_save_extra_whitespace() {
        let (path, format) = parse_save_args("  data.tsv   tsv  ");
        assert_eq!(path, "data.tsv");
        assert_eq!(format.unwrap(), "tsv");
    }

    // -- CSV writer ----------------------------------------------------------

    fn make_fixture() -> SqlResult {
        SqlResult {
            columns: vec!["id".to_string(), "name".to_string(), "score".to_string()],
            rows: vec![
                vec!["1".to_string(), "Alice".to_string(), "95".to_string()],
                vec!["2".to_string(), "Bob".to_string(), "87".to_string()],
                vec!["3".to_string(), "Carol".to_string(), "92".to_string()],
            ],
            row_count: 3,
            truncated: false,
            duration: Duration::from_millis(42),
        }
    }

    #[test]
    fn test_write_csv_output() {
        let result = make_fixture();
        let mut buf = Vec::new();
        write_csv(&mut buf, &result).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 4); // header + 3 rows
        assert_eq!(lines[0], "id,name,score");
        assert_eq!(lines[1], "1,Alice,95");
        assert_eq!(lines[2], "2,Bob,87");
        assert_eq!(lines[3], "3,Carol,92");
    }

    #[test]
    fn test_write_csv_empty() {
        let result = SqlResult {
            columns: vec!["a".to_string()],
            rows: vec![],
            row_count: 0,
            truncated: false,
            duration: Duration::from_millis(0),
        };
        let mut buf = Vec::new();
        write_csv(&mut buf, &result).unwrap();
        let output = String::from_utf8(buf).unwrap();
        // Header only, trailing newline from csv writer
        assert!(output.starts_with("a\n") || output.starts_with("a\r\n"));
    }

    #[test]
    fn test_write_csv_special_chars() {
        let result = SqlResult {
            columns: vec!["col".to_string()],
            rows: vec![vec!["hello, \"world\"".to_string()]],
            row_count: 1,
            truncated: false,
            duration: Duration::from_millis(0),
        };
        let mut buf = Vec::new();
        write_csv(&mut buf, &result).unwrap();
        let output = String::from_utf8(buf).unwrap();
        // csv crate should properly quote
        assert!(output.contains("\"hello, \"\"world\"\"\""));
    }

    // -- JSON writer ---------------------------------------------------------

    #[test]
    fn test_write_json_output() {
        let result = make_fixture();
        let mut buf = Vec::new();
        write_json(&mut buf, &result).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let parsed: Vec<serde_json::Value> = serde_json::from_str(output.trim()).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0]["id"], "1");
        assert_eq!(parsed[0]["name"], "Alice");
        assert_eq!(parsed[1]["name"], "Bob");
    }

    #[test]
    fn test_write_json_empty() {
        let result = SqlResult {
            columns: vec!["a".to_string()],
            rows: vec![],
            row_count: 0,
            truncated: false,
            duration: Duration::from_millis(0),
        };
        let mut buf = Vec::new();
        write_json(&mut buf, &result).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(output.trim()).unwrap();
        assert!(parsed.is_empty());
    }

    // -- TSV writer ----------------------------------------------------------

    #[test]
    fn test_write_tsv_output() {
        let result = make_fixture();
        let mut buf = Vec::new();
        write_tsv(&mut buf, &result).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "id\tname\tscore");
        assert_eq!(lines[1], "1\tAlice\t95");
    }

    #[test]
    fn test_write_tsv_empty() {
        let result = SqlResult {
            columns: vec!["x".to_string(), "y".to_string()],
            rows: vec![],
            row_count: 0,
            truncated: false,
            duration: Duration::from_millis(0),
        };
        let mut buf = Vec::new();
        write_tsv(&mut buf, &result).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output.trim(), "x\ty");
    }

    // -- Table writer --------------------------------------------------------

    #[test]
    fn test_write_table_output() {
        let result = make_fixture();
        let mut buf = Vec::new();
        write_table(&mut buf, &result).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let lines: Vec<&str> = output.lines().collect();
        // Header, separator, 3 data rows
        assert_eq!(lines.len(), 5);
        assert!(lines[0].contains("id") && lines[0].contains("name"));
        assert!(lines[1].contains("-+-")); // separator
    }

    #[test]
    fn test_write_table_empty() {
        let result = SqlResult {
            columns: vec![],
            rows: vec![],
            row_count: 0,
            truncated: false,
            duration: Duration::from_millis(0),
        };
        let mut buf = Vec::new();
        write_table(&mut buf, &result).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output.trim(), "(empty)");
    }

    // -- Integration: write_save_file with temp file -------------------------

    #[test]
    fn test_write_save_file_csv() {
        let result = make_fixture();
        let dir = std::env::temp_dir();
        let path = dir.join("grep_excel_test_save.csv");
        let path_str = path.to_string_lossy().to_string();

        write_save_file(&result, &path_str, "csv").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("id,name,score"));
        assert!(content.contains("Alice"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_write_save_file_json() {
        let result = make_fixture();
        let dir = std::env::temp_dir();
        let path = dir.join("grep_excel_test_save.json");
        let path_str = path.to_string_lossy().to_string();

        write_save_file(&result, &path_str, "json").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed.len(), 3);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_write_save_file_unknown_format_falls_back_to_csv() {
        let result = make_fixture();
        let dir = std::env::temp_dir();
        let path = dir.join("grep_excel_test_save.xyz");
        let path_str = path.to_string_lossy().to_string();

        // Should succeed with csv fallback
        write_save_file(&result, &path_str, "parquet").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("id,name,score"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_write_save_file_empty_path() {
        let result = make_fixture();
        let err = write_save_file(&result, "", "csv").unwrap_err();
        assert!(err.to_string().contains("no output path"));
    }

    // -- parse_let_args -------------------------------------------------------

    #[test]
    fn test_parse_let_args_basic() {
        let (name, sql) = parse_let_args("my_table AS SELECT * FROM sheet_1_0").unwrap();
        assert_eq!(name, "my_table");
        assert_eq!(sql, "SELECT * FROM sheet_1_0");
    }

    #[test]
    fn test_parse_let_args_case_insensitive_as() {
        let (name, sql) = parse_let_args("foo As SELECT 1").unwrap();
        assert_eq!(name, "foo");
        assert_eq!(sql, "SELECT 1");
    }

    #[test]
    fn test_parse_let_args_uppercase_as() {
        let (name, sql) = parse_let_args("foo AS SELECT 1").unwrap();
        assert_eq!(name, "foo");
        assert_eq!(sql, "SELECT 1");
    }

    #[test]
    fn test_parse_let_args_sql_contains_as() {
        let (name, sql) = parse_let_args("t AS SELECT x AS y FROM z").unwrap();
        assert_eq!(name, "t");
        assert_eq!(sql, "SELECT x AS y FROM z");
    }

    #[test]
    fn test_parse_let_args_empty_name() {
        assert!(parse_let_args(" AS SELECT 1").is_none());
    }

    #[test]
    fn test_parse_let_args_empty_sql() {
        assert!(parse_let_args("foo AS ").is_none());
    }

    #[test]
    fn test_parse_let_args_no_as() {
        assert!(parse_let_args("foo bar").is_none());
    }

    #[test]
    fn test_parse_let_args_empty_string() {
        assert!(parse_let_args("").is_none());
    }
}
