use crate::types::{FileInfo, SearchResult, SearchStats, SqlResult};
use anyhow::Result;
use crossterm::event::KeyEvent;
use std::sync::mpsc;

pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    FileImported(Result<FileInfo>),
    SearchCompleted(Result<(Vec<SearchResult>, SearchStats)>),
    SqlCompleted(Result<SqlResult>),
    Progress(usize, usize),
}

pub type EventSender = mpsc::Sender<AppEvent>;
pub type EventReceiver = mpsc::Receiver<AppEvent>;

pub fn create_event_channel() -> (EventSender, EventReceiver) {
    mpsc::channel()
}
