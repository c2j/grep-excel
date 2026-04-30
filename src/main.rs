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

    #[arg(short, long, help = "Filter to a specific sheet name")]
    sheet: Option<String>,

    #[arg(
        short = 'm',
        long,
        default_value = "fulltext",
        value_parser = ["fulltext", "exact", "wildcard", "regex"],
        help = "Search mode: fulltext (substring), exact (precise), wildcard (SQL LIKE), regex"
    )]
    mode: String,

    #[cfg(feature = "mcp-server")]
    #[arg(long, help = "Start MCP server mode (stdio)")]
    mcp: bool,

    #[arg(short = 'e', long, help = "Export search results to a CSV file")]
    export: Option<PathBuf>,

    #[arg(short = 'x', long, help = "Execute a SQL SELECT query against imported data")]
    sql: Option<String>,

    #[arg(short = 'g', long, help = "Aggregate column: count distinct values in matched rows")]
    aggregate: Option<String>,

    #[arg(short = 'v', long, help = "Invert match: show rows that do NOT match the query")]
    invert: bool,

    #[arg(short = 't', long, help = "List imported tables with friendly names and columns")]
    list_tables: bool,

    #[arg(
        short = 'f',
        long,
        default_value = "markdown",
        value_parser = ["markdown", "pretty"],
        help = "Output format: markdown (default) or pretty (unicode table)"
    )]
    format: String,
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

    #[cfg(feature = "mcp-server")]
    if args.mcp {
        return tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(grep_excel::mcp::run_mcp_server());
    }

    if args.list_tables {
        return run_list_tables_cli(&args);
    }

    if args.sql.is_some() {
        return run_sql_cli(&args);
    }

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
        sheet: args.sheet.clone(),
        invert: args.invert,
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
            Err(e) => eprintln!("{}", grep_excel::i18n::cli_export_failed()),
        }
    }

    if args.export.is_none() {
        if args.format == "pretty" {
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
        } else {
            let mut first = true;
            for result in &results {
                if result.file_name != last_file || result.sheet_name != last_sheet {
                    if !first {
                        println!();
                    }
                    first = false;
                    println!("**{} / {}**", result.file_name, result.sheet_name);
                    last_file = result.file_name.clone();
                    last_sheet = result.sheet_name.clone();

                    let sep: Vec<String> = result.col_names.iter().map(|_| "---".to_string()).collect();
                    println!("| {} |", result.col_names.join(" | "));
                    println!("| {} |", sep.join(" | "));
                }

                println!("| {} |", result.row.join(" | "));
            }
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

    if let Some(ref agg_col) = args.aggregate {
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for result in &results {
            if let Some(col_idx) = result.col_names.iter().position(|c| c == agg_col) {
                if let Some(value) = result.row.get(col_idx) {
                    if !value.is_empty() {
                        *counts.entry(value.clone()).or_insert(0) += 1;
                    }
                }
            }
        }

        if !counts.is_empty() {
            let mut sorted_counts: Vec<_> = counts.iter().collect();
            sorted_counts.sort_by(|a, b| b.1.cmp(a.1));
            let agg_parts: Vec<String> = sorted_counts
                .into_iter()
                .map(|(k, v)| format!("{} ({})", k, v))
                .collect();
            println!(
                "  {}: {}",
                grep_excel::i18n::cli_aggregate_label(agg_col),
                agg_parts.join(", ")
            );
        } else {
            println!(
                "  {}",
                grep_excel::i18n::cli_aggregate_no_data(agg_col)
            );
        }
    }

    Ok(())
}

fn run_sql_cli(args: &Args) -> Result<()> {
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
                );
            }
            Err(e) => eprintln!(
                "{}",
                grep_excel::i18n::cli_import_failed(&file.display().to_string(), &e.to_string())
            ),
        }
    }

    let aliases = db.list_table_aliases();
    if !aliases.is_empty() {
        eprintln!();
        for alias in &aliases {
            eprintln!("  {}", alias.table_name);
        }
        eprintln!();
    }

    let sql = args.sql.as_ref().unwrap();
    let result = match db.execute_sql(sql, 10000) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", grep_excel::i18n::cli_sql_failed(&e.to_string()));
            return Ok(());
        }
    };

    if result.rows.is_empty() {
        println!("{}", grep_excel::i18n::cli_sql_no_results());
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

    if let Some(ref export_path) = args.export {
        let mut wtr = csv::Writer::from_path(export_path)?;
        wtr.write_record(&result.columns)?;
        for row in &result.rows {
            wtr.write_record(row)?;
        }
        wtr.flush()?;
        eprintln!(
            "{}",
            grep_excel::i18n::cli_export_done(&export_path.display().to_string())
        );
    } else if args.format == "pretty" {
        print_header(&result.columns, &widths);
        print_separator(&widths);

        for row in &result.rows {
            let parts: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(i, cell)| pad_to(cell, widths.get(i).copied().unwrap_or(10)))
                .collect();
            println!("  {}", parts.join(" │ "));
        }
    } else {
        let sep: Vec<String> = result.columns.iter().map(|_| "---".to_string()).collect();
        println!("| {} |", result.columns.join(" | "));
        println!("| {} |", sep.join(" | "));
        for row in &result.rows {
            println!("| {} |", row.join(" | "));
        }
    }

    println!();
    println!(
        "{}",
        grep_excel::i18n::cli_match_summary(
            result.row_count,
            result.row_count,
            result.duration.as_millis()
        )
    );

    Ok(())
}

fn run_list_tables_cli(args: &Args) -> Result<()> {
    let mut db = DefaultEngine::new()?;

    for file in &args.files {
        if !file.exists() {
            eprintln!(
                "{}",
                grep_excel::i18n::cli_file_not_found(&file.display().to_string())
            );
            continue;
        }
        match db.import_excel(file, &|_, _| {}) {
            Ok(info) => {
                eprintln!(
                    "{}",
                    grep_excel::i18n::cli_imported(
                        &info.name,
                        info.sheets.len(),
                        info.total_rows
                    )
                );
            }
            Err(e) => eprintln!(
                "{}",
                grep_excel::i18n::cli_import_failed(
                    &file.display().to_string(),
                    &e.to_string()
                )
            ),
        }
    }

    let aliases = db.list_table_aliases();
    if aliases.is_empty() {
        println!("{}", grep_excel::i18n::cli_list_tables_empty());
        return Ok(());
    }

    println!("{}", grep_excel::i18n::cli_list_tables_header());
    for alias in &aliases {
        let cols_str = alias.columns.join(", ");
        println!(
            "  {}",
            grep_excel::i18n::cli_list_tables_entry(
                &alias.alias,
                &alias.table_name,
                alias.row_count,
                &cols_str,
            )
        );
    }

    println!();
    println!(
        "{}",
        grep_excel::i18n::cli_list_tables_footer(aliases.len())
    );

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
