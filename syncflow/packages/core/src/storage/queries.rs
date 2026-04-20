use sqlx::{SqlitePool, Row};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::{Result, SyncFlowError};
use super::models::*;

pub struct StorageEngine {
    pool: SqlitePool,
}

impl StorageEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn save_file_meta(&self, meta: &FileMetadata) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO file_metadata (path, hash, size, modified_at, version_vector, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(path) DO UPDATE SET
                hash = excluded.hash, size = excluded.size,
                modified_at = excluded.modified_at, version_vector = excluded.version_vector
            "#,
        )
        .bind(&meta.path)
        .bind(&meta.hash)
        .bind(meta.size as i64)
        .bind(meta.modified_at.to_rfc3339())
        .bind(&meta.version_vector)
        .bind(meta.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(())
    }

    pub async fn get_file_meta(&self, path: &str) -> Result<Option<FileMetadata>> {
        let row = sqlx::query(
            "SELECT path, hash, size, modified_at, version_vector, created_at FROM file_metadata WHERE path = ?",
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        Ok(row.map(|r| FileMetadata {
            path: r.try_get("path").unwrap(),
            hash: r.try_get("hash").unwrap(),
            size: r.try_get::<i64, _>("size").unwrap() as u64,
            modified_at: parse_rfc3339(&r.try_get::<String, _>("modified_at").unwrap()),
            version_vector: r.try_get("version_vector").unwrap(),
            created_at: parse_rfc3339(&r.try_get::<String, _>("created_at").unwrap()),
        }))
    }

    pub async fn save_sync_state(&self, state: &SyncState) -> Result<()> {
        let status_str = match state.sync_status {
            SyncStatus::Idle => "idle",
            SyncStatus::Syncing => "syncing",
            SyncStatus::Conflict => "conflict",
            SyncStatus::Error => "error",
        };

        sqlx::query(
            r#"
            INSERT INTO sync_state (peer_id, last_sync_at, sync_status, pending_changes)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(peer_id) DO UPDATE SET
                last_sync_at = excluded.last_sync_at,
                sync_status = excluded.sync_status,
                pending_changes = excluded.pending_changes
            "#,
        )
        .bind(state.peer_id.to_string())
        .bind(state.last_sync_at.map(|t| t.to_rfc3339()))
        .bind(status_str)
        .bind(state.pending_changes as i64)
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(())
    }

    pub async fn get_sync_state(&self, peer_id: &Uuid) -> Result<Option<SyncState>> {
        let row = sqlx::query(
            "SELECT peer_id, last_sync_at, sync_status, pending_changes FROM sync_state WHERE peer_id = ?",
        )
        .bind(peer_id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        Ok(row.map(|r| {
            let status_str: String = r.try_get("sync_status").unwrap();
            let sync_status = match status_str.as_str() {
                "idle" => SyncStatus::Idle,
                "syncing" => SyncStatus::Syncing,
                "conflict" => SyncStatus::Conflict,
                "error" => SyncStatus::Error,
                _ => SyncStatus::Idle,
            };
            SyncState {
                peer_id: r.try_get("peer_id").ok().and_then(|s: String| Uuid::parse_str(&s).ok()).unwrap_or_default(),
                last_sync_at: r.try_get::<Option<String>, _>("last_sync_at").ok().flatten().map(|s| parse_rfc3339(&s)),
                sync_status,
                pending_changes: r.try_get::<i64, _>("pending_changes").unwrap() as u32,
            }
        }))
    }

    pub async fn save_version(&self, version: &FileVersion) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO file_versions (file_path, hash, version_vector, device_id, is_conflict, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&version.file_path)
        .bind(&version.hash)
        .bind(&version.version_vector)
        .bind(&version.device_id)
        .bind(version.is_conflict)
        .bind(version.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(())
    }

    pub async fn get_version_history(&self, path: &str) -> Result<Vec<FileVersion>> {
        let rows = sqlx::query(
            "SELECT file_path, hash, version_vector, device_id, is_conflict, created_at FROM file_versions WHERE file_path = ? ORDER BY created_at DESC",
        )
        .bind(path)
        .fetch_all(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        Ok(rows.into_iter().map(|r| FileVersion {
            file_path: r.try_get("file_path").unwrap(),
            hash: r.try_get("hash").unwrap(),
            version_vector: r.try_get("version_vector").unwrap(),
            device_id: r.try_get("device_id").unwrap(),
            is_conflict: r.try_get("is_conflict").unwrap(),
            created_at: parse_rfc3339(&r.try_get::<String, _>("created_at").unwrap()),
        }).collect())
    }

    pub async fn save_device_info(&self, info: &DeviceInfo) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO devices (device_id, device_name, platform, public_key, last_seen_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(device_id) DO UPDATE SET
                device_name = excluded.device_name, platform = excluded.platform,
                public_key = excluded.public_key, last_seen_at = excluded.last_seen_at
            "#,
        )
        .bind(info.device_id.to_string())
        .bind(&info.device_name)
        .bind(&info.platform)
        .bind(&info.public_key)
        .bind(info.last_seen_at.map(|t| t.to_rfc3339()))
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(())
    }

    pub async fn get_known_devices(&self) -> Result<Vec<DeviceInfo>> {
        let rows = sqlx::query(
            "SELECT device_id, device_name, platform, public_key, last_seen_at FROM devices",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        let mut devices = Vec::new();
        for r in rows {
            let device_id_str: String = r.try_get("device_id").unwrap();
            let device_id = Uuid::parse_str(&device_id_str)
                .map_err(|e| SyncFlowError::Database(sqlx::Error::Decode(Box::new(e))))?;
            devices.push(DeviceInfo {
                device_id,
                device_name: r.try_get("device_name").unwrap(),
                platform: r.try_get("platform").unwrap(),
                public_key: r.try_get("public_key").unwrap(),
                last_seen_at: r
                    .try_get::<Option<String>, _>("last_seen_at")
                    .ok()
                    .flatten()
                    .map(|t| parse_rfc3339(&t)),
            });
        }
        Ok(devices)
    }
}

fn parse_rfc3339(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .unwrap_or_else(|_| Utc::now().into())
        .with_timezone(&Utc)
}

