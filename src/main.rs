use anyhow::Result;
use clap::Parser;
use grep_excel::app::App;
use grep_excel::engine::{DefaultEngine, SearchEngine};
use grep_excel::event::create_event_channel;
use grep_excel::types::{SearchMode, SearchQuery};
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

#[derive(Parser, Debug)]
#[command(
    name = "grep_excel",
    about = "",
    long_about = ""
)]
struct Args {
    #[arg(name = "FILES")]
    files: Vec<PathBuf>,

    #[arg(short, long, help = "Search query string")]
    query: Option<String>,

    #[arg(short, long, help = "Filter to a specific column name")]
    column: Option<String>,

    #[arg(
        short = 'm',
        long,
        default_value = "fulltext",
        value_parser = ["fulltext", "exact", "wildcard", "regex"],
        help = "Search mode: fulltext (substring), exact (precise), wildcard (SQL LIKE), regex"
    )]
    mode: String,

    #[arg(short = 'e', long, help = "Export search results to a CSV file")]
    export: Option<PathBuf>,
}

fn main() -> Result<()> {
    grep_excel::i18n::init();

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{}", grep_excel::i18n::help_full_text());
        return Ok(());
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("grep_excel {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let args = Args::parse();

    if args.query.is_some() {
        return run_cli(&args);
    }

    run_tui(&args)
}

fn run_tui(args: &Args) -> Result<()> {
    let database = DefaultEngine::new()?;
    let (event_tx, event_rx) = create_event_channel();
    let mut app = App::new(database, event_tx, event_rx);

    for file in &args.files {
        if file.exists() {
            app.import_file(file.clone());
        }
    }

    app.run()
}

fn run_cli(args: &Args) -> Result<()> {
    let mut db = DefaultEngine::new()?;

    for file in &args.files {
        if !file.exists() {
            eprintln!("{}", grep_excel::i18n::cli_file_not_found(&file.display().to_string()));
            continue;
        }
        match db.import_excel(file, &|_, _| {}) {
            Ok(info) => {
                eprintln!(
                    "{}",
                    grep_excel::i18n::cli_imported(&info.name, info.sheets.len(), info.total_rows)
                )
            }
            Err(e) => eprintln!("{}", grep_excel::i18n::cli_import_failed(&file.display().to_string(), &e.to_string())),
        }
    }

    let query = SearchQuery {
        text: args.query.clone().unwrap_or_default(),
        column: args.column.clone(),
        mode: match args.mode.as_str() {
            "exact" => SearchMode::ExactMatch,
            "wildcard" => SearchMode::Wildcard,
            "regex" => SearchMode::Regex,
            _ => SearchMode::FullText,
        },
        limit: usize::MAX,
    };

    let (results, stats) = match db.search(&query) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", grep_excel::i18n::cli_search_failed(&e.to_string()));
            return Ok(());
        }
    };

    if results.is_empty() {
        println!("{}", grep_excel::i18n::cli_no_matches(&query.text));
        return Ok(());
    }

    let mut last_file = String::new();
    let mut last_sheet = String::new();

    if let Some(ref export_path) = args.export {
        match grep_excel::engine::export_results_csv(&results, export_path) {
            Ok(()) => eprintln!("{}", grep_excel::i18n::cli_export_done(&export_path.display().to_string())),
            Err(e) => eprintln!("{}", grep_excel::i18n::cli_export_failed(&e.to_string())),
        }
    }

    if args.export.is_none() {
        for result in &results {
        if result.file_name != last_file || result.sheet_name != last_sheet {
            if !last_file.is_empty() {
                println!();
            }
            println!("{} / {}", result.file_name, result.sheet_name);
            last_file = result.file_name.clone();
            last_sheet = result.sheet_name.clone();

            let widths = compute_cli_col_widths(&result.col_names, &results);
            print_header(&result.col_names, &widths);
            print_separator(&widths);
        }

        let widths = compute_cli_col_widths(&result.col_names, &results);
        print_row(
            &result.col_names,
            &result.row,
            &result.matched_columns,
            &widths,
            query.mode,
             &query.text,
         );
     }
    }

    println!();
    println!(
        "{}",
        grep_excel::i18n::cli_match_summary(
            stats.total_matches,
            stats.total_rows_searched,
            stats.search_duration.as_millis()
        )
    );

    if !stats.matches_per_sheet.is_empty() {
        let per_sheet: Vec<String> = stats
            .matches_per_sheet
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect();
        println!("  [{}]", per_sheet.join(", "));
    }

    Ok(())
}

fn compute_cli_col_widths(
    col_names: &[String],
    results: &[grep_excel::types::SearchResult],
) -> Vec<usize> {
    let mut widths: Vec<usize> = col_names
        .iter()
        .map(|n| UnicodeWidthStr::width(n.as_str()))
        .collect();

    for result in results.iter().take(200) {
        for (i, cell) in result.row.iter().enumerate() {
            if i < widths.len() {
                let w = UnicodeWidthStr::width(cell.as_str());
                if w > widths[i] {
                    widths[i] = w;
                }
            }
        }
    }

    for w in &mut widths {
        *w = (*w).min(40);
    }

    widths
}

fn print_header(col_names: &[String], widths: &[usize]) {
    let parts: Vec<String> = col_names
        .iter()
        .enumerate()
        .map(|(i, name)| pad_to(name, widths[i]))
        .collect();
    println!("  {}", parts.join(" │ "));
}

fn print_separator(widths: &[usize]) {
    let parts: Vec<String> = widths.iter().map(|&w| "─".repeat(w)).collect();
    println!("  {}", parts.join("─┼─"));
}

fn print_row(
    col_names: &[String],
    row: &[String],
    matched: &[usize],
    widths: &[usize],
    mode: SearchMode,
    query_text: &str,
) {
    let parts: Vec<String> = col_names
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let value = row.get(i).cloned().unwrap_or_default();
            let padded = pad_to(&value, widths[i]);
            if matched.contains(&i) {
                let spans = grep_excel::engine::find_match_spans(mode, query_text, &value);
                if spans.is_empty() {
                    format!("\x1b[1;32m{}\x1b[0m", padded)
                } else {
                    highlight_ansi(&padded, &value, &spans, widths[i])
                }
            } else {
                padded
            }
        })
        .collect();
    println!("  {}", parts.join(" │ "));
}

fn highlight_ansi(
    padded: &str,
    original: &str,
    spans: &[(usize, usize)],
    _width: usize,
) -> String {
    let green = "\x1b[1;32m";
    let reset = "\x1b[0m";
    let mut result = String::new();
    let mut last_end = 0;
    for &(start, end) in spans {
        if start > last_end {
            result.push_str(&original[last_end..start]);
        }
        result.push_str(green);
        result.push_str(&original[start..end.min(original.len())]);
        result.push_str(reset);
        last_end = end.max(last_end);
    }
    if last_end < original.len() {
        result.push_str(&original[last_end..]);
    }
    if padded.len() > original.len() {
        result.push_str(&padded[original.len()..]);
    }
    result
}

fn pad_to(s: &str, width: usize) -> String {
    let sw = UnicodeWidthStr::width(s);
    if sw >= width {
        let truncated: String = s.chars().take(width - 1).collect();
        format!("{}…", truncated)
    } else {
        let mut out = s.to_string();
        for _ in 0..width - sw {
            out.push(' ');
        }
        out
    }
}
