use crate::error::{Result, SyncFlowError};
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind, Debouncer};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;
use tokio::sync::mpsc as tokio_mpsc;

#[derive(Debug, Clone)]
pub enum FileEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
}

impl FileEvent {
    pub fn path(&self) -> &str {
        match self {
            FileEvent::Created(p) | FileEvent::Modified(p) | FileEvent::Deleted(p) => {
                p.to_str().unwrap_or("")
            }
        }
    }
}

fn debounced_event_to_file_events(event: notify_debouncer_mini::DebouncedEvent) -> Vec<FileEvent> {
    match event.kind {
        DebouncedEventKind::Any | DebouncedEventKind::AnyContinuous => {
            vec![FileEvent::Modified(event.path)]
        }
        _ => vec![],
    }
}

pub fn start_watcher(
    paths: Vec<PathBuf>,
    event_tx: tokio_mpsc::Sender<FileEvent>,
) -> Result<Debouncer<notify::RecommendedWatcher>> {
    let (inner_tx, inner_rx) = mpsc::channel();

    let mut debouncer =
        new_debouncer(Duration::from_millis(500), inner_tx).map_err(|e| SyncFlowError::from(e))?;

    let watcher = debouncer.watcher();
    for path in &paths {
        watcher
            .watch(path, RecursiveMode::Recursive)
            .map_err(|e| SyncFlowError::from(e))?;
    }

    // Spawn a task to forward debounced events to the tokio channel
    tokio::spawn(async move {
        while let Ok(result) = inner_rx.recv() {
            if let Ok(events) = result {
                for event in events {
                    for file_event in debounced_event_to_file_events(event) {
                        let _ = event_tx.send(file_event).await;
                    }
                }
            }
        }
    });

    Ok(debouncer)
}
