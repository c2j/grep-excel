mod app;
mod database;
mod event;
mod excel;

use crate::app::App;
use crate::database::{Database, SearchMode};
use crate::event::create_event_channel;
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "search_excel")]
#[command(about = "TUI tool for searching Excel files")]
struct Args {
    #[arg(name = "FILES")]
    files: Vec<PathBuf>,

    #[arg(short, long)]
    query: Option<String>,

    #[arg(short, long)]
    column: Option<String>,

    #[arg(short = 'm', long, default_value = "fulltext", value_parser = ["fulltext", "exact", "wildcard"])]
    mode: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let database = Database::new()?;
    let (event_tx, event_rx) = create_event_channel();
    let mut app = App::new(database, event_tx, event_rx);

    for file in &args.files {
        if file.exists() {
            app.import_file(file.clone());
        }
    }

    if let Some(query) = &args.query {
        app.set_search_query(query.clone());

        if let Some(column) = &args.column {
            app.set_column_filter(column.clone());
        }

        let mode = match args.mode.as_str() {
            "exact" => SearchMode::ExactMatch,
            "wildcard" => SearchMode::Wildcard,
            _ => SearchMode::FullText,
        };
        app.set_search_mode(mode);
    }

    app.run()
}
