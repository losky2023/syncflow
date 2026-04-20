use thiserror::Error;

#[derive(Error, Debug)]
pub enum SyncFlowError {
    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("WebRTC error: {0}")]
    WebRtc(String),

    #[error("Signal error: {0}")]
    Signal(String),

    #[error("Auth error: {0}")]
    Auth(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Conflict detected: {0}")]
    Conflict(String),

    #[error("File watcher error: {0}")]
    Watcher(#[from] notify::Error),
}

pub type Result<T> = std::result::Result<T, SyncFlowError>;
