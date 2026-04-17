mod app;
mod database;
mod event;
mod excel;

use crate::app::App;
use crate::database::{Database, SearchMode, SearchQuery};
use crate::event::create_event_channel;
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

#[derive(Parser, Debug)]
#[command(name = "grep_excel")]
#[command(about = "TUI tool for searching Excel/CSV files with grep-like patterns")]
struct Args {
    #[arg(name = "FILES")]
    files: Vec<PathBuf>,

    #[arg(short, long)]
    query: Option<String>,

    #[arg(short, long)]
    column: Option<String>,

    #[arg(short = 'm', long, default_value = "fulltext", value_parser = ["fulltext", "exact", "wildcard", "regex"])]
    mode: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.query.is_some() {
        return run_cli(&args);
    }

    run_tui(&args)
}

fn run_tui(args: &Args) -> Result<()> {
    let database = Database::new()?;
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
    let mut db = Database::new()?;

    for file in &args.files {
        if !file.exists() {
            eprintln!("文件不存在: {}", file.display());
            continue;
        }
        match db.import_excel(file, |_, _| {}) {
            Ok(info) => {
                eprintln!(
                    "已导入: {} ({} 工作表, {} 行)",
                    info.name,
                    info.sheets.len(),
                    info.total_rows
                )
            }
            Err(e) => eprintln!("导入失败 {}: {}", file.display(), e),
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
            eprintln!("搜索失败: {}", e);
            return Ok(());
        }
    };

    if results.is_empty() {
        println!("未找到匹配项: \"{}\"", query.text);
        return Ok(());
    }

    let mut last_file = String::new();
    let mut last_sheet = String::new();

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
        );
    }

    println!();
    println!(
        "共 {} 条匹配 (搜索 {} 行, 耗时 {}ms)",
        stats.total_matches,
        stats.total_rows_searched,
        stats.search_duration.as_millis()
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
    results: &[crate::database::SearchResult],
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

fn print_row(col_names: &[String], row: &[String], matched: &[usize], widths: &[usize]) {
    let parts: Vec<String> = col_names
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let value = row.get(i).cloned().unwrap_or_default();
            let is_matched = matched.contains(&i);
            let padded = pad_to(&value, widths[i]);
            if is_matched {
                format!("\x1b[1;32m{}\x1b[0m", padded)
            } else {
                padded
            }
        })
        .collect();
    println!("  {}", parts.join(" │ "));
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
