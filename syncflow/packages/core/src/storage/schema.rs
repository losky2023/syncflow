use sqlx::{Executor, SqlitePool};

use crate::error::{Result, SyncFlowError};

pub const CREATE_TABLES: &str = r#"
CREATE TABLE IF NOT EXISTS file_metadata (
    space_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    hash TEXT NOT NULL,
    size INTEGER NOT NULL,
    modified_at TEXT NOT NULL,
    version_vector TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (space_id, relative_path)
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

CREATE TABLE IF NOT EXISTS accounts (
    account_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    password_salt BLOB NOT NULL,
    encrypted_account_secret BLOB NOT NULL,
    created_at TEXT NOT NULL,
    last_unlocked_at TEXT
);

CREATE TABLE IF NOT EXISTS synced_spaces (
    id TEXT PRIMARY KEY,
    sync_key TEXT NOT NULL,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_scanned_at TEXT
);

CREATE TABLE IF NOT EXISTS sync_conflicts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    space_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    local_version TEXT NOT NULL,
    remote_version TEXT NOT NULL,
    remote_device_id TEXT NOT NULL,
    detected_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_conflict_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conflict_id INTEGER NOT NULL,
    space_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    snapshot_kind TEXT NOT NULL,
    content_text TEXT,
    content_truncated INTEGER NOT NULL DEFAULT 0,
    content_size INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cloud_api_configs (
    provider TEXT PRIMARY KEY,
    device_id TEXT,
    client_id TEXT NOT NULL,
    client_secret TEXT,
    redirect_uri TEXT NOT NULL,
    scopes TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cloud_accounts (
    provider TEXT PRIMARY KEY,
    account_id TEXT,
    display_name TEXT,
    access_token_encrypted BLOB NOT NULL,
    refresh_token_encrypted BLOB NOT NULL,
    expires_at TEXT,
    scopes TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cloud_space_bindings (
    space_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    remote_root_path TEXT NOT NULL,
    remote_root_id TEXT,
    sync_mode TEXT NOT NULL,
    plaintext INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (space_id, provider)
);

CREATE TABLE IF NOT EXISTS remote_file_metadata (
    space_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    remote_path TEXT NOT NULL,
    local_relative_path TEXT NOT NULL,
    remote_file_id TEXT,
    is_directory INTEGER NOT NULL DEFAULT 0,
    size INTEGER NOT NULL DEFAULT 0,
    md5 TEXT,
    server_mtime TEXT,
    remote_revision TEXT,
    last_remote_file_id TEXT,
    last_remote_md5 TEXT,
    last_remote_size INTEGER,
    last_remote_server_mtime TEXT,
    last_remote_revision TEXT,
    last_local_hash TEXT,
    last_local_modified_at TEXT,
    last_local_size INTEGER,
    last_seen_at TEXT NOT NULL,
    last_synced_at TEXT,
    tombstone INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (space_id, provider, local_relative_path)
);

CREATE TABLE IF NOT EXISTS cloud_sync_tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    space_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    task_kind TEXT NOT NULL,
    local_relative_path TEXT NOT NULL,
    remote_path TEXT NOT NULL,
    expected_remote_revision TEXT,
    payload_json TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    next_attempt_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_remote_file_metadata_remote_path
ON remote_file_metadata(space_id, provider, remote_path);

CREATE INDEX IF NOT EXISTS idx_cloud_sync_tasks_pending
ON cloud_sync_tasks(space_id, provider, next_attempt_at, created_at);
"#;

pub async fn initialize_schema(pool: &SqlitePool) -> Result<()> {
    pool.execute(CREATE_TABLES)
        .await
        .map_err(SyncFlowError::Database)?;
    migrate_file_metadata_schema(pool).await?;
    sqlx::query("ALTER TABLE synced_spaces ADD COLUMN sync_key TEXT")
        .execute(pool)
        .await
        .ok();
    sqlx::query("UPDATE synced_spaces SET sync_key = id WHERE sync_key IS NULL OR sync_key = ''")
        .execute(pool)
        .await
        .ok();
    sqlx::query("ALTER TABLE cloud_api_configs ADD COLUMN device_id TEXT")
        .execute(pool)
        .await
        .ok();
    sqlx::query("ALTER TABLE remote_file_metadata ADD COLUMN last_local_hash TEXT")
        .execute(pool)
        .await
        .ok();
    sqlx::query("ALTER TABLE remote_file_metadata ADD COLUMN last_remote_file_id TEXT")
        .execute(pool)
        .await
        .ok();
    sqlx::query("ALTER TABLE remote_file_metadata ADD COLUMN last_remote_md5 TEXT")
        .execute(pool)
        .await
        .ok();
    sqlx::query("ALTER TABLE remote_file_metadata ADD COLUMN last_remote_size INTEGER")
        .execute(pool)
        .await
        .ok();
    sqlx::query("ALTER TABLE remote_file_metadata ADD COLUMN last_remote_server_mtime TEXT")
        .execute(pool)
        .await
        .ok();
    sqlx::query("ALTER TABLE remote_file_metadata ADD COLUMN last_remote_revision TEXT")
        .execute(pool)
        .await
        .ok();
    sqlx::query("ALTER TABLE remote_file_metadata ADD COLUMN last_local_modified_at TEXT")
        .execute(pool)
        .await
        .ok();
    sqlx::query("ALTER TABLE remote_file_metadata ADD COLUMN last_local_size INTEGER")
        .execute(pool)
        .await
        .ok();
    Ok(())
}

async fn migrate_file_metadata_schema(pool: &SqlitePool) -> Result<()> {
    let columns = sqlx::query("PRAGMA table_info(file_metadata)")
        .fetch_all(pool)
        .await
        .map_err(SyncFlowError::Database)?;
    let column_names: Vec<String> = columns
        .iter()
        .filter_map(|row| sqlx::Row::try_get::<String, _>(row, "name").ok())
        .collect();

    if column_names.iter().any(|name| name == "space_id")
        && column_names.iter().any(|name| name == "relative_path")
    {
        return Ok(());
    }

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS file_metadata_new (
            space_id TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            hash TEXT NOT NULL,
            size INTEGER NOT NULL,
            modified_at TEXT NOT NULL,
            version_vector TEXT NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (space_id, relative_path)
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(SyncFlowError::Database)?;

    if column_names.iter().any(|name| name == "path") {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO file_metadata_new
                (space_id, relative_path, hash, size, modified_at, version_vector, created_at)
            SELECT
                '00000000-0000-0000-0000-000000000000',
                path,
                hash,
                size,
                modified_at,
                version_vector,
                created_at
            FROM file_metadata
            WHERE path IS NOT NULL AND path != ''
            "#,
        )
        .execute(pool)
        .await
        .map_err(SyncFlowError::Database)?;
    }

    sqlx::query("DROP TABLE file_metadata")
        .execute(pool)
        .await
        .map_err(SyncFlowError::Database)?;
    sqlx::query("ALTER TABLE file_metadata_new RENAME TO file_metadata")
        .execute(pool)
        .await
        .map_err(SyncFlowError::Database)?;

    Ok(())
}
