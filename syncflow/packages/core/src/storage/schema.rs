use sqlx::{Executor, SqlitePool};

use crate::error::{Result, SyncFlowError};

pub const CREATE_TABLES: &str = r#"
CREATE TABLE IF NOT EXISTS file_metadata (
    path TEXT PRIMARY KEY,
    hash TEXT NOT NULL,
    size INTEGER NOT NULL,
    modified_at TEXT NOT NULL,
    version_vector TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_state (
    peer_id TEXT PRIMARY KEY,
    last_sync_at TEXT,
    sync_status TEXT NOT NULL DEFAULT 'idle',
    pending_changes INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS file_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL,
    hash TEXT NOT NULL,
    version_vector TEXT NOT NULL,
    device_id TEXT NOT NULL,
    is_conflict INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS devices (
    device_id TEXT PRIMARY KEY,
    device_name TEXT NOT NULL,
    platform TEXT NOT NULL,
    public_key TEXT NOT NULL,
    last_seen_at TEXT
);

CREATE TABLE IF NOT EXISTS synced_spaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_scanned_at TEXT
);
"#;

pub async fn initialize_schema(pool: &SqlitePool) -> Result<()> {
    pool.execute(CREATE_TABLES)
        .await
        .map_err(SyncFlowError::Database)?;
    Ok(())
}
