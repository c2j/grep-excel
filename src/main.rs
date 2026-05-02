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
        value_parser = ["markdown", "pretty", "json", "simple"],
        help = "Output format: markdown, pretty, json, or simple (TSV)"
    )]
    format: String,

    #[arg(
        short = 'E',
        long,
        num_args = 0..=1,
        default_missing_value = "help",
        help = "Execute tool command(s) as JSON. Use --exec alone or --exec help to list all tools."
    )]
    exec: Option<String>,
}

fn main() -> Result<()> {
    grep_excel::i18n::init();

    let args: Vec<String> = std::env::args().collect();

    // Intercept --exec --help / --mcp --help BEFORE general --help
    let show_exec_help = {
        let has_exec = args.iter().any(|a| a == "--exec" || a == "-E");
        let has_mcp = args.iter().any(|a| a == "--mcp");
        let has_help = args.iter().any(|a| a == "--help" || a == "-h");
        (has_exec || has_mcp) && has_help
    };
    if show_exec_help {
        print_exec_help();
        return Ok(());
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{}", grep_excel::i18n::help_full_text());
        return Ok(());
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("grep_excel {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let args = Args::parse();

    if args.exec.is_some() {
        return run_exec(&args);
    }

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

fn run_exec(args: &Args) -> Result<()> {
    let exec_json = args.exec.as_ref().unwrap();

    if exec_json == "help" {
        print_exec_help();
        return Ok(());
    }

    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct ExecCommand {
        tool: String,
        params: serde_json::Value,
    }

    let exec_json = args.exec.as_ref().unwrap();
    let commands: Vec<ExecCommand> = if exec_json.trim_start().starts_with('[') {
        serde_json::from_str(exec_json)?
    } else {
        vec![serde_json::from_str(exec_json)?]
    };

    let raw_args: Vec<String> = std::env::args().collect();
    let exec_format = if raw_args.iter().any(|a| a == "--format" || a == "-f") {
        args.format.clone()
    } else {
        "json".to_string()
    };

    let mut db = DefaultEngine::new()?;
    let mut import_paths: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for file in &args.files {
        if !file.exists() {
            eprintln!("{}", grep_excel::i18n::cli_file_not_found(&file.display().to_string()));
            continue;
        }
        let canonical = std::fs::canonicalize(file)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| file.display().to_string());
        match db.import_excel(file, &|_, _| {}) {
            Ok(info) => {
                eprintln!("{}", grep_excel::i18n::cli_imported(&info.name, info.sheets.len(), info.total_rows));
                import_paths.insert(info.name.clone(), canonical);
            }
            Err(e) => eprintln!("{}", grep_excel::i18n::cli_import_failed(&file.display().to_string(), &e.to_string())),
        }
    }

    for (i, cmd) in commands.iter().enumerate() {
        if commands.len() > 1 {
            eprintln!("\n--- Step {} ---", i + 1);
        }
        let result = exec_dispatch(&mut db, &mut import_paths, &cmd.tool, &cmd.params);
        match result {
            Ok(output) => println!("{}", format_exec_output(&cmd.tool, &output, &exec_format)),
            Err(e) => {
                eprintln!("Error in step {} ({}): {}", i + 1, cmd.tool, e);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

fn print_exec_help() {
    let lang = grep_excel::i18n::current();
    match lang {
        grep_excel::i18n::Lang::Zh => {
            println!("grep_excel --exec 可用工具");
            println!();
            println!("用法:");
            println!("  grep_excel <文件...> --exec '<JSON>'");
            println!();
            println!("  JSON 可以是单条命令或命令数组:");
            println!("    单条: '{{\"tool\":\"search\",\"params\":{{\"query\":\"关键词\"}}}}'");
            println!("    数组: '[{{\"tool\":\"import_file\",\"params\":...}}, {{\"tool\":\"search\",\"params\":...}}]'");
            println!();
            println!("  位置参数中的文件会在执行命令前自动导入。");
            println!("  数组中的命令按顺序串行执行，共享同一数据状态 (导入/修改逐步累积)。");
            println!();
            println!("示例:");
            println!("  # 查看已导入文件");
            println!(r#"  grep_excel data.xlsx --exec '{{"tool":"list_files","params":{{}}}}'"#);
            println!();
            println!("  # 搜索 + 聚合统计");
            println!(r#"  grep_excel data.xlsx --exec '{{"tool":"search","params":{{"query":"张三","mode":"exact","aggregate":"City"}}}}'"#);
            println!();
            println!("  # 多步串行: 导入 → 查询元数据 → 采样 → 修改 → 保存");
            println!("  grep_excel --exec '\\");
            println!("    [\\");
            println!(r#"      {{"tool":"import_file","params":{{"file_path":"data.xlsx"}}}},"#);
            println!(r#"      {{"tool":"get_metadata","params":{{}}}},"#);
            println!(r#"      {{"tool":"get_sheet_sample","params":{{"file_name":"data.xlsx","sheet_name":"Sheet1","sample_size":3}}}},"#);
            println!(r#"      {{"tool":"update_cell","params":{{"file_name":"data.xlsx","sheet_name":"Sheet1","row":0,"column":"Name","value":"李四"}}}},"#);
            println!(r#"      {{"tool":"save","params":{{"file_name":"data.xlsx"}}}}"#);
            println!("    ]");
            println!();
            println!("工具列表:");
            println!();
            println!("  \x1b[1mimport_file\x1b[0m          导入 Excel/CSV 文件");
            println!("                       参数: file_path (文件路径)");
            println!();
            println!("  \x1b[1mlist_files\x1b[0m           列出所有已导入文件及其 sheet 信息");
            println!("                       参数: 无");
            println!();
            println!("  \x1b[1mget_metadata\x1b[0m         获取文件详细元数据 (sheet 名、列名)");
            println!("                       参数: file_name? (文件名, 省略则返回全部)");
            println!();
            println!("  \x1b[1mget_sheet_sample\x1b[0m     获取 sheet 的均匀采样行");
            println!("                       参数: file_name, sheet_name, sample_size? (默认 10)");
            println!();
            println!("  \x1b[1mget_sheet_data\x1b[0m       获取 sheet 的分页行数据");
            println!("                       参数: file_name, sheet_name, start_row?, end_row?, columns?");
            println!();
            println!("  \x1b[1msearch\x1b[0m               全文/精确/通配符/正则搜索");
            println!("                       参数: query, column?, sheet?, mode?, limit?, aggregate?, invert?");
            println!();
            println!("  \x1b[1mexecute_sql\x1b[0m          执行 SQL SELECT 查询");
            println!("                       参数: sql, limit? (默认 1000)");
            println!();
            println!("  \x1b[1mupdate_cell\x1b[0m          更新单个单元格");
            println!("                       参数: file_name, sheet_name, row (0-based), column, value");
            println!();
            println!("  \x1b[1mupdate_cells\x1b[0m         批量更新多个单元格");
            println!("                       参数: file_name, sheet_name, updates: [{{row, column, value}}]");
            println!();
            println!("  \x1b[1minsert_rows\x1b[0m          在指定位置插入行");
            println!("                       参数: file_name, sheet_name, start_row, rows: [[...]]");
            println!();
            println!("  \x1b[1mdelete_rows\x1b[0m          删除指定位置的行");
            println!("                       参数: file_name, sheet_name, start_row, count");
            println!();
            println!("  \x1b[1madd_column\x1b[0m           添加新列");
            println!("                       参数: file_name, sheet_name, column_name, default_value?");
            println!();
            println!("  \x1b[1mrename_column\x1b[0m        重命名列");
            println!("                       参数: file_name, sheet_name, old_name, new_name");
            println!();
            println!("  \x1b[1msave_as\x1b[0m              另存为新文件 (不修改原文件)");
            println!("                       参数: file_name, output_path, sheet_name?");
            println!();
            println!("  \x1b[1msave\x1b[0m                 保存回原文件 (覆盖)");
            println!("                       参数: file_name, sheet_name?");
        }
        grep_excel::i18n::Lang::En => {
            println!("grep_excel --exec available tools");
            println!();
            println!("Usage:");
            println!("  grep_excel <files...> --exec '<JSON>'");
            println!();
            println!("  JSON can be a single command or an array:");
            println!(r#"    Single: '{{"tool":"search","params":{{"query":"keyword"}}}}'"#);
            println!(r#"    Array:  '[{{"tool":"import_file","params":...}}, {{"tool":"search","params":...}}]'"#);
            println!();
            println!("  Files passed as positional args are auto-imported before commands run.");
            println!("  Array commands execute sequentially, sharing state (imports/edits accumulate).");
            println!();
            println!("Examples:");
            println!(r#"  # List imported files"#);
            println!(r#"  grep_excel data.xlsx --exec '{{"tool":"list_files","params":{{}}}}'"#);
            println!();
            println!("  # Search with aggregation");
            println!(r#"  grep_excel data.xlsx --exec '{{"tool":"search","params":{{"query":"Engineering","aggregate":"City"}}}}'"#);
            println!();
            println!("  # Multi-step pipeline: import → metadata → sample → edit → save");
            println!("  grep_excel --exec '\\");
            println!("    [\\");
            println!(r#"      {{"tool":"import_file","params":{{"file_path":"data.xlsx"}}}},"#);
            println!(r#"      {{"tool":"get_metadata","params":{{}}}},"#);
            println!(r#"      {{"tool":"get_sheet_sample","params":{{"file_name":"data.xlsx","sheet_name":"Sheet1","sample_size":3}}}},"#);
            println!(r#"      {{"tool":"update_cell","params":{{"file_name":"data.xlsx","sheet_name":"Sheet1","row":0,"column":"Name","value":"fixed"}}}},"#);
            println!(r#"      {{"tool":"save","params":{{"file_name":"data.xlsx"}}}}"#);
            println!("    ]");
            println!();
            println!("Tools:");
            println!();
            println!("  \x1b[1mimport_file\x1b[0m          Import an Excel/CSV file");
            println!("                       Params: file_path");
            println!();
            println!("  \x1b[1mlist_files\x1b[0m           List all imported files and their sheets");
            println!("                       Params: (none)");
            println!();
            println!("  \x1b[1mget_metadata\x1b[0m         Get detailed metadata (sheet names, column names)");
            println!("                       Params: file_name? (omit for all files)");
            println!();
            println!("  \x1b[1mget_sheet_sample\x1b[0m     Get evenly-spaced sample rows from a sheet");
            println!("                       Params: file_name, sheet_name, sample_size? (default: 10)");
            println!();
            println!("  \x1b[1mget_sheet_data\x1b[0m       Get paginated rows from a sheet");
            println!("                       Params: file_name, sheet_name, start_row?, end_row?, columns?");
            println!();
            println!("  \x1b[1msearch\x1b[0m               Search with fulltext/exact/wildcard/regex");
            println!("                       Params: query, column?, sheet?, mode?, limit?, aggregate?, invert?");
            println!();
            println!("  \x1b[1mexecute_sql\x1b[0m          Execute a SQL SELECT query");
            println!("                       Params: sql, limit? (default: 1000)");
            println!();
            println!("  \x1b[1mupdate_cell\x1b[0m          Update a single cell value");
            println!("                       Params: file_name, sheet_name, row (0-based), column, value");
            println!();
            println!("  \x1b[1mupdate_cells\x1b[0m         Batch update multiple cells");
            println!("                       Params: file_name, sheet_name, updates: [{{row, column, value}}]");
            println!();
            println!("  \x1b[1minsert_rows\x1b[0m          Insert rows at a specified position");
            println!("                       Params: file_name, sheet_name, start_row, rows: [[...]]");
            println!();
            println!("  \x1b[1mdelete_rows\x1b[0m          Delete rows from a specified position");
            println!("                       Params: file_name, sheet_name, start_row, count");
            println!();
            println!("  \x1b[1madd_column\x1b[0m           Add a new column");
            println!("                       Params: file_name, sheet_name, column_name, default_value?");
            println!();
            println!("  \x1b[1mrename_column\x1b[0m        Rename an existing column");
            println!("                       Params: file_name, sheet_name, old_name, new_name");
            println!();
            println!("  \x1b[1msave_as\x1b[0m              Save to a new file (does not modify original)");
            println!("                       Params: file_name, output_path, sheet_name?");
            println!();
            println!("  \x1b[1msave\x1b[0m                 Save back to the original file (overwrite)");
            println!("                       Params: file_name, sheet_name?");
        }
    }
}

fn format_table_output(val: &serde_json::Value, fmt: &str, stats_suffix: Option<&str>) -> String {
    let columns = val.get("columns").and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>())
        .unwrap_or_default();
    let rows = val.get("rows").and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_array().map(|arr| {
            arr.iter().filter_map(|v| v.as_str().map(String::from).or_else(|| if v.is_null() { Some("".to_string()) } else { Some(v.to_string()) })).collect::<Vec<_>>()
        })).collect::<Vec<_>>())
        .unwrap_or_default();

    if columns.is_empty() {
        return serde_json::to_string_pretty(val).unwrap_or_default();
    }

    match fmt {
        "simple" => {
            let mut out = columns.join("\t");
            for row in &rows {
                out.push('\n');
                out.push_str(&row.join("\t"));
            }
            out
        }
        "markdown" | "pretty" => {
            let mut out = String::new();
            let sep: Vec<&str> = columns.iter().map(|_| "---").collect();
            out.push_str(&format!("| {} |", columns.join(" | ")));
            out.push('\n');
            out.push_str(&format!("| {} |", sep.join(" | ")));
            for row in &rows {
                out.push('\n');
                out.push_str(&format!("| {} |", row.join(" | ")));
            }
            if let Some(suffix) = stats_suffix {
                out.push('\n');
                out.push_str(suffix);
            }
            out
        }
        _ => serde_json::to_string_pretty(val).unwrap_or_default(),
    }
}

fn format_search_output(val: &serde_json::Value, fmt: &str) -> String {
    let results = val.get("results").and_then(|v| v.as_array());
    let stats = val.get("stats");

    let results_arr = match results {
        Some(arr) => arr,
        None => return serde_json::to_string_pretty(val).unwrap_or_default(),
    };

    if results_arr.is_empty() {
        let msg = match fmt {
            "simple" => String::new(),
            _ => "No matches found.".to_string(),
        };
        return if let Some(s) = stats {
            match fmt {
                "simple" => format!("# {} rows searched, 0 matches, {}ms",
                    s.get("total_rows_searched").and_then(|v| v.as_u64()).unwrap_or(0),
                    s.get("search_duration_ms").and_then(|v| v.as_u64()).unwrap_or(0)),
                _ => format!("{}\n{} rows searched, 0 matches, {}ms",
                    msg,
                    s.get("total_rows_searched").and_then(|v| v.as_u64()).unwrap_or(0),
                    s.get("search_duration_ms").and_then(|v| v.as_u64()).unwrap_or(0)),
            }
        } else {
            msg
        };
    }

    let mut columns = vec!["file".to_string(), "sheet".to_string()];
    if let Some(col_names) = results_arr[0].get("col_names").and_then(|v| v.as_array()) {
        for c in col_names {
            columns.push(c.as_str().unwrap_or("").to_string());
        }
    }

    let mut rows = Vec::new();
    for r in results_arr {
        let mut row = Vec::new();
        row.push(r.get("file_name").and_then(|v| v.as_str()).unwrap_or("").to_string());
        row.push(r.get("sheet_name").and_then(|v| v.as_str()).unwrap_or("").to_string());
        if let Some(data) = r.get("row").and_then(|v| v.as_array()) {
            for cell in data {
                row.push(cell.as_str().unwrap_or("").to_string());
            }
        }
        rows.push(row);
    }

    let stats_suffix = stats.map(|s| {
        let matches = s.get("total_matches").and_then(|v| v.as_u64()).unwrap_or(0);
        let searched = s.get("total_rows_searched").and_then(|v| v.as_u64()).unwrap_or(0);
        let ms = s.get("search_duration_ms").and_then(|v| v.as_u64()).unwrap_or(0);
        format!("{} matches ({} rows searched, {}ms)", matches, searched, ms)
    });

    let table_val = serde_json::json!({"columns": columns, "rows": rows});
    let mut out = format_table_output(&table_val, fmt, stats_suffix.as_deref());

    if let Some(agg) = val.get("aggregate").and_then(|v| v.as_object()) {
        if let Some(counts) = agg.get("counts").and_then(|v| v.as_array()) {
            if !counts.is_empty() {
                match fmt {
                    "simple" => {
                        out.push('\n');
                        for c in counts {
                            let value = c.get("value").and_then(|v| v.as_str()).unwrap_or("");
                            let count = c.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
                            out.push_str(&format!("{}\t{}", value, count));
                            out.push('\n');
                        }
                    }
                    _ => {
                        let agg_parts: Vec<String> = counts.iter().map(|c| {
                            let value = c.get("value").and_then(|v| v.as_str()).unwrap_or("");
                            let count = c.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
                            format!("{} ({})", value, count)
                        }).collect();
                        out.push_str(&format!("\nAggregate: {}", agg_parts.join(", ")));
                    }
                }
            }
        }
    }

    out
}

fn format_list_files_output(val: &serde_json::Value, fmt: &str) -> String {
    let files = match val.get("files").and_then(|v| v.as_array()) {
        Some(f) => f,
        None => return serde_json::to_string_pretty(val).unwrap_or_default(),
    };

    if files.is_empty() {
        return match fmt {
            "simple" => String::new(),
            _ => "No files imported.".to_string(),
        };
    }

    match fmt {
        "simple" => {
            let mut out = String::new();
            out.push_str("name\tsheets\ttotal_rows");
            for f in files {
                let name = f.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let total_rows = f.get("total_rows").and_then(|v| v.as_u64()).unwrap_or(0);
                let sheet_count = f.get("sheets").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
                out.push_str(&format!("\n{}\t{}\t{}", name, sheet_count, total_rows));
            }
            out
        }
        "markdown" | "pretty" => {
            let mut out = String::new();
            out.push_str("| name | sheets | total_rows |");
            out.push('\n');
            out.push_str("| --- | --- | --- |");
            for f in files {
                let name = f.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let total_rows = f.get("total_rows").and_then(|v| v.as_u64()).unwrap_or(0);
                let sheet_count = f.get("sheets").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
                out.push_str(&format!("\n| {} | {} | {} |", name, sheet_count, total_rows));
            }
            out
        }
        _ => serde_json::to_string_pretty(val).unwrap_or_default(),
    }
}

fn format_metadata_output(val: &serde_json::Value, fmt: &str) -> String {
    let files = match val.get("files").and_then(|v| v.as_array()) {
        Some(f) => f,
        None => return serde_json::to_string_pretty(val).unwrap_or_default(),
    };

    match fmt {
        "simple" => {
            let mut out = String::new();
            out.push_str("file_name\tsheet_name\trow_count\tcolumns");
            for f in files {
                let file_name = f.get("file_name").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(sheets) = f.get("sheets").and_then(|v| v.as_array()) {
                    for s in sheets {
                        let sheet_name = s.get("sheet_name").and_then(|v| v.as_str()).unwrap_or("");
                        let row_count = s.get("row_count").and_then(|v| v.as_u64()).unwrap_or(0);
                        let cols = s.get("columns").and_then(|v| v.as_array())
                            .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(","))
                            .unwrap_or_default();
                        out.push_str(&format!("\n{}\t{}\t{}\t{}", file_name, sheet_name, row_count, cols));
                    }
                }
            }
            out
        }
        "markdown" | "pretty" => {
            let mut out = String::new();
            for f in files {
                let file_name = f.get("file_name").and_then(|v| v.as_str()).unwrap_or("");
                let sheet_count = f.get("sheet_count").and_then(|v| v.as_u64()).unwrap_or(0);
                out.push_str(&format!("**{}** ({} sheets)\n", file_name, sheet_count));
                if let Some(sheets) = f.get("sheets").and_then(|v| v.as_array()) {
                    for s in sheets {
                        let sheet_name = s.get("sheet_name").and_then(|v| v.as_str()).unwrap_or("");
                        let row_count = s.get("row_count").and_then(|v| v.as_u64()).unwrap_or(0);
                        let cols = s.get("columns").and_then(|v| v.as_array())
                            .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
                            .unwrap_or_default();
                        out.push_str(&format!("  - {} ({} rows): {}\n", sheet_name, row_count, cols));
                    }
                }
                out.push('\n');
            }
            out.trim_end().to_string()
        }
        _ => serde_json::to_string_pretty(val).unwrap_or_default(),
    }
}

fn format_import_output(val: &serde_json::Value, fmt: &str) -> String {
    match fmt {
        "simple" => {
            let name = val.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let total_rows = val.get("total_rows").and_then(|v| v.as_u64()).unwrap_or(0);
            let sheets = val.get("sheets").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
            format!("{}\t{}\t{}", name, sheets, total_rows)
        }
        _ => serde_json::to_string_pretty(val).unwrap_or_default(),
    }
}

fn format_exec_output(tool: &str, json_str: &str, fmt: &str) -> String {
    if fmt == "json" || fmt != "markdown" && fmt != "simple" && fmt != "pretty" {
        return json_str.to_string();
    }

    let val: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return json_str.to_string(),
    };

    match tool {
        "execute_sql" | "get_sheet_data" | "get_sheet_sample" => {
            format_table_output(&val, fmt, None)
        }
        "search" => format_search_output(&val, fmt),
        "list_files" => format_list_files_output(&val, fmt),
        "get_metadata" => format_metadata_output(&val, fmt),
        "import_file" => format_import_output(&val, fmt),
        _ => json_str.to_string(),
    }
}

fn exec_dispatch(
    db: &mut DefaultEngine,
    import_paths: &mut std::collections::HashMap<String, String>,
    tool: &str,
    params: &serde_json::Value,
) -> Result<String> {
    use grep_excel::engine::SearchEngine;
    use grep_excel::types::*;

    match tool {
        "import_file" => {
            let p: ImportFileParams = serde_json::from_value(params.clone())?;
            let path = std::path::PathBuf::from(&p.file_path);
            let canonical = std::fs::canonicalize(&path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| p.file_path.clone());
            let info = db.import_excel(&path, &|_, _| {})?;
            import_paths.insert(info.name.clone(), canonical);
            Ok(serde_json::to_string_pretty(&serde_json::json!({
                "name": info.name,
                "sheets": info.sheets,
                "total_rows": info.total_rows,
            }))?)
        }
        "list_files" => {
            let files = db.list_files();
            let file_infos: Vec<serde_json::Value> = files.iter().map(|f| serde_json::json!({
                "name": f.name,
                "sheets": f.sheets,
                "total_rows": f.total_rows,
            })).collect();
            Ok(serde_json::to_string_pretty(&serde_json::json!({ "files": file_infos }))?)
        }
        "get_metadata" => {
            let p: GetMetadataParams = serde_json::from_value(params.clone())?;
            if let Some(file_name) = p.file_name {
                let m = db.get_metadata(&file_name)?;
                let sheets_json: Vec<serde_json::Value> = m.sheets.iter().map(|s| serde_json::json!({
                    "sheet_name": s.sheet_name,
                    "row_count": s.row_count,
                    "columns": s.columns,
                })).collect();
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "file_name": m.file_name,
                    "sheet_count": m.sheet_count,
                    "sheets": sheets_json,
                }))?)
            } else {
                let files = db.list_files();
                let mut all = Vec::new();
                for f in files {
                    if let Ok(m) = db.get_metadata(&f.name) {
                        let sheets_json: Vec<serde_json::Value> = m.sheets.iter().map(|s| serde_json::json!({
                            "sheet_name": s.sheet_name,
                            "row_count": s.row_count,
                            "columns": s.columns,
                        })).collect();
                        all.push(serde_json::json!({
                            "file_name": m.file_name,
                            "sheet_count": m.sheet_count,
                            "sheets": sheets_json,
                        }));
                    }
                }
                Ok(serde_json::to_string_pretty(&serde_json::json!({ "files": all }))?)
            }
        }
        "get_sheet_sample" => {
            let p: GetSheetSampleParams = serde_json::from_value(params.clone())?;
            let r = db.get_sheet_sample(&p.file_name, &p.sheet_name, p.sample_size.unwrap_or(10))?;
            Ok(serde_json::to_string_pretty(&serde_json::json!({
                "file_name": r.file_name,
                "sheet_name": r.sheet_name,
                "columns": r.columns,
                "rows": r.rows,
                "row_count": r.row_count,
                "total_rows": r.total_rows,
                "truncated": r.truncated,
            }))?)
        }
        "get_sheet_data" => {
            let p: GetSheetDataParams = serde_json::from_value(params.clone())?;
            let r = db.get_sheet_data(&p.file_name, &p.sheet_name, p.start_row, p.end_row, p.columns.as_deref())?;
            Ok(serde_json::to_string_pretty(&serde_json::json!({
                "file_name": r.file_name,
                "sheet_name": r.sheet_name,
                "columns": r.columns,
                "rows": r.rows,
                "row_count": r.row_count,
                "total_rows": r.total_rows,
                "truncated": r.truncated,
            }))?)
        }
        "search" => {
            let p: SearchParams = serde_json::from_value(params.clone())?;
            let mode = p.mode.as_deref().map(|m| match m {
                "exact" => SearchMode::ExactMatch,
                "wildcard" => SearchMode::Wildcard,
                "regex" => SearchMode::Regex,
                _ => SearchMode::FullText,
            }).unwrap_or(SearchMode::FullText);
            let aggregate_col = p.aggregate.as_ref().cloned();
            let query = SearchQuery {
                text: p.query,
                column: p.column,
                mode,
                limit: p.limit.unwrap_or(100),
                sheet: p.sheet,
                invert: p.invert.unwrap_or(false),
            };
            let (results, stats) = db.search(&query)?;

            let aggregate = aggregate_col.as_ref().and_then(|col| {
                let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
                for result in &results {
                    if let Some(col_idx) = result.col_names.iter().position(|c| c == col) {
                        if let Some(value) = result.row.get(col_idx) {
                            if !value.is_empty() {
                                *counts.entry(value.clone()).or_insert(0) += 1;
                            }
                        }
                    }
                }
                if counts.is_empty() { None } else {
                    let mut sorted: Vec<_> = counts.into_iter().collect();
                    sorted.sort_by(|a, b| b.1.cmp(&a.1));
                    Some(sorted.into_iter().map(|(value, count)| serde_json::json!({"value": value, "count": count})).collect::<Vec<_>>())
                }
            });

            let results_json: Vec<serde_json::Value> = results.iter().map(|r| {
                let matched_cols: Vec<String> = r.matched_columns.iter()
                    .filter_map(|&idx| r.col_names.get(idx).cloned())
                    .collect();
                serde_json::json!({
                    "file_name": r.file_name,
                    "sheet_name": r.sheet_name,
                    "row": r.row,
                    "col_names": r.col_names,
                    "matched_column_names": matched_cols,
                })
            }).collect();

            Ok(serde_json::to_string_pretty(&serde_json::json!({
                "results": results_json,
                "stats": {
                    "total_rows_searched": stats.total_rows_searched,
                    "total_matches": stats.total_matches,
                    "search_duration_ms": stats.search_duration.as_millis(),
                    "truncated": stats.truncated,
                },
                "aggregate": aggregate.map(|a| serde_json::json!({"column": aggregate_col, "counts": a})),
            }))?)
        }
        "execute_sql" => {
            let p: SqlQueryParams = serde_json::from_value(params.clone())?;
            let result = db.execute_sql(&p.sql, p.limit.unwrap_or(1000))?;
            Ok(serde_json::to_string_pretty(&serde_json::json!({
                "columns": result.columns,
                "rows": result.rows,
                "row_count": result.row_count,
                "truncated": result.truncated,
                "duration_ms": result.duration.as_millis(),
            }))?)
        }
        "update_cell" => {
            let p: UpdateCellParams = serde_json::from_value(params.clone())?;
            db.update_cell(&p.file_name, &p.sheet_name, p.row, &p.column, &p.value)?;
            Ok(format!("Updated cell at row {}, column '{}' to '{}'", p.row, p.column, p.value))
        }
        "update_cells" => {
            let p: UpdateCellsParams = serde_json::from_value(params.clone())?;
            let updates: Vec<(usize, String, String)> = p.updates.into_iter().map(|u| (u.row, u.column, u.value)).collect();
            let total = updates.len();
            let count = db.update_cells(&p.file_name, &p.sheet_name, &updates)?;
            Ok(format!("Updated {}/{} cells", count, total))
        }
        "insert_rows" => {
            let p: InsertRowsParams = serde_json::from_value(params.clone())?;
            let count = p.rows.len();
            db.insert_rows(&p.file_name, &p.sheet_name, p.start_row, p.rows)?;
            Ok(format!("Inserted {} rows at position {}", count, p.start_row))
        }
        "delete_rows" => {
            let p: DeleteRowsParams = serde_json::from_value(params.clone())?;
            let actual = db.delete_rows(&p.file_name, &p.sheet_name, p.start_row, p.count)?;
            Ok(format!("Deleted {} rows starting at row {}", actual, p.start_row))
        }
        "add_column" => {
            let p: AddColumnParams = serde_json::from_value(params.clone())?;
            let default = p.default_value.unwrap_or_default();
            db.add_column(&p.file_name, &p.sheet_name, &p.column_name, &default)?;
            Ok(format!("Added column '{}' with default value '{}'", p.column_name, default))
        }
        "rename_column" => {
            let p: RenameColumnParams = serde_json::from_value(params.clone())?;
            db.rename_column(&p.file_name, &p.sheet_name, &p.old_name, &p.new_name)?;
            Ok(format!("Renamed column '{}' to '{}'", p.old_name, p.new_name))
        }
        "save_as" => {
            let p: SaveAsParams = serde_json::from_value(params.clone())?;
            #[cfg(feature = "mcp-server")]
            {
                if let Some(ref sheet_name) = p.sheet_name {
                    let data = db.get_sheet_data(&p.file_name, sheet_name, None, None, None)?;
                    use grep_excel::engine::write_xlsx;
                    write_xlsx(
                        &[(sheet_name.as_str(), &data.columns, &data.rows)],
                        std::path::Path::new(&p.output_path),
                    )?;
                } else {
                    db.save_as(&p.file_name, std::path::Path::new(&p.output_path))?;
                }
                Ok(format!("Successfully saved to '{}'", p.output_path))
            }
            #[cfg(not(feature = "mcp-server"))]
            {
                anyhow::bail!("save_as requires the mcp-server feature to be enabled")
            }
        }
        "save" => {
            let p: SaveParams = serde_json::from_value(params.clone())?;
            let original_path = import_paths.get(&p.file_name).cloned()
                .ok_or_else(|| anyhow::anyhow!("Original path for '{}' not found. File may not have been imported via import_file.", p.file_name))?;
            #[cfg(feature = "mcp-server")]
            {
                if let Some(ref sheet_name) = p.sheet_name {
                    let data = db.get_sheet_data(&p.file_name, sheet_name, None, None, None)?;
                    use grep_excel::engine::write_xlsx;
                    write_xlsx(
                        &[(sheet_name.as_str(), &data.columns, &data.rows)],
                        std::path::Path::new(&original_path),
                    )?;
                } else {
                    db.save_as(&p.file_name, std::path::Path::new(&original_path))?;
                }
                Ok(format!("Saved to '{}'", original_path))
            }
            #[cfg(not(feature = "mcp-server"))]
            {
                anyhow::bail!("save requires the mcp-server feature to be enabled")
            }
        }
        _ => anyhow::bail!(
            "Unknown tool: '{}'. Available: import_file, list_files, get_metadata, get_sheet_sample, get_sheet_data, search, execute_sql, save_as, save, update_cell, update_cells, insert_rows, delete_rows, add_column, rename_column",
            tool
        ),
    }
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
