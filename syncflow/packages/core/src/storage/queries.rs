use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::models::*;
use crate::error::{Result, SyncFlowError};

#[derive(Clone)]
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

    pub async fn get_local_account(&self) -> Result<Option<AccountRecord>> {
        let row = sqlx::query(
            "SELECT account_id, display_name, password_salt, encrypted_account_secret, created_at, last_unlocked_at FROM accounts ORDER BY created_at ASC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        row.map(row_to_account_record).transpose()
    }

    pub async fn save_account(&self, account: &AccountRecord) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO accounts (account_id, display_name, password_salt, encrypted_account_secret, created_at, last_unlocked_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(account_id) DO UPDATE SET
                display_name = excluded.display_name,
                password_salt = excluded.password_salt,
                encrypted_account_secret = excluded.encrypted_account_secret,
                last_unlocked_at = excluded.last_unlocked_at
            "#,
        )
        .bind(account.account_id.to_string())
        .bind(&account.display_name)
        .bind(&account.password_salt)
        .bind(&account.encrypted_account_secret)
        .bind(account.created_at.to_rfc3339())
        .bind(account.last_unlocked_at.map(|t| t.to_rfc3339()))
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(())
    }

    pub async fn replace_local_account(&self, account: &AccountRecord) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(SyncFlowError::Database)?;
        sqlx::query("DELETE FROM accounts")
            .execute(&mut *tx)
            .await
            .map_err(SyncFlowError::Database)?;
        sqlx::query(
            r#"
            INSERT INTO accounts (account_id, display_name, password_salt, encrypted_account_secret, created_at, last_unlocked_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(account.account_id.to_string())
        .bind(&account.display_name)
        .bind(&account.password_salt)
        .bind(&account.encrypted_account_secret)
        .bind(account.created_at.to_rfc3339())
        .bind(account.last_unlocked_at.map(|t| t.to_rfc3339()))
        .execute(&mut *tx)
        .await
        .map_err(SyncFlowError::Database)?;
        tx.commit().await.map_err(SyncFlowError::Database)?;
        Ok(())
    }

    pub async fn save_cloud_api_config(&self, config: &CloudApiConfig) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO cloud_api_configs
                (provider, device_id, client_id, client_secret, redirect_uri, scopes, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(provider) DO UPDATE SET
                device_id = excluded.device_id,
                client_id = excluded.client_id,
                client_secret = excluded.client_secret,
                redirect_uri = excluded.redirect_uri,
                scopes = excluded.scopes,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&config.provider)
        .bind(&config.device_id)
        .bind(&config.client_id)
        .bind(&config.client_secret)
        .bind(&config.redirect_uri)
        .bind(
            serde_json::to_string(&config.scopes)
                .map_err(|e| SyncFlowError::Cloud(e.to_string()))?,
        )
        .bind(config.created_at.to_rfc3339())
        .bind(config.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(())
    }

    pub async fn get_cloud_api_config(&self, provider: &str) -> Result<Option<CloudApiConfig>> {
        let row = sqlx::query(
            r#"
            SELECT provider, device_id, client_id, client_secret, redirect_uri, scopes, created_at, updated_at
            FROM cloud_api_configs
            WHERE provider = ?
            "#,
        )
        .bind(provider)
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        row.map(row_to_cloud_api_config).transpose()
    }

    pub async fn remove_cloud_api_config(&self, provider: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM cloud_api_configs WHERE provider = ?")
            .bind(provider)
            .execute(&self.pool)
            .await
            .map_err(SyncFlowError::Database)?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn save_cloud_account(&self, account: &CloudAccount) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO cloud_accounts
                (provider, account_id, display_name, access_token_encrypted, refresh_token_encrypted, expires_at, scopes, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(provider) DO UPDATE SET
                account_id = excluded.account_id,
                display_name = excluded.display_name,
                access_token_encrypted = excluded.access_token_encrypted,
                refresh_token_encrypted = excluded.refresh_token_encrypted,
                expires_at = excluded.expires_at,
                scopes = excluded.scopes,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&account.provider)
        .bind(&account.account_id)
        .bind(&account.display_name)
        .bind(&account.access_token_encrypted)
        .bind(&account.refresh_token_encrypted)
        .bind(account.expires_at.map(|t| t.to_rfc3339()))
        .bind(serde_json::to_string(&account.scopes).map_err(|e| SyncFlowError::Cloud(e.to_string()))?)
        .bind(account.created_at.to_rfc3339())
        .bind(account.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(())
    }

    pub async fn get_cloud_account(&self, provider: &str) -> Result<Option<CloudAccount>> {
        let row = sqlx::query(
            r#"
            SELECT provider, account_id, display_name, access_token_encrypted, refresh_token_encrypted, expires_at, scopes, created_at, updated_at
            FROM cloud_accounts
            WHERE provider = ?
            "#,
        )
        .bind(provider)
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        row.map(row_to_cloud_account).transpose()
    }

    pub async fn remove_cloud_account(&self, provider: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM cloud_accounts WHERE provider = ?")
            .bind(provider)
            .execute(&self.pool)
            .await
            .map_err(SyncFlowError::Database)?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn save_cloud_space_binding(&self, binding: &CloudSpaceBinding) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO cloud_space_bindings
                (space_id, provider, remote_root_path, remote_root_id, sync_mode, plaintext, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(space_id, provider) DO UPDATE SET
                remote_root_path = excluded.remote_root_path,
                remote_root_id = excluded.remote_root_id,
                sync_mode = excluded.sync_mode,
                plaintext = excluded.plaintext,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(binding.space_id.to_string())
        .bind(&binding.provider)
        .bind(&binding.remote_root_path)
        .bind(&binding.remote_root_id)
        .bind(&binding.sync_mode)
        .bind(binding.plaintext)
        .bind(binding.created_at.to_rfc3339())
        .bind(binding.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(())
    }

    pub async fn get_cloud_space_binding(
        &self,
        space_id: &Uuid,
        provider: &str,
    ) -> Result<Option<CloudSpaceBinding>> {
        let row = sqlx::query(
            r#"
            SELECT space_id, provider, remote_root_path, remote_root_id, sync_mode, plaintext, created_at, updated_at
            FROM cloud_space_bindings
            WHERE space_id = ? AND provider = ?
            "#,
        )
        .bind(space_id.to_string())
        .bind(provider)
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        row.map(row_to_cloud_space_binding).transpose()
    }

    pub async fn get_cloud_space_bindings_for_provider(
        &self,
        provider: &str,
    ) -> Result<Vec<CloudSpaceBinding>> {
        let rows = sqlx::query(
            r#"
            SELECT space_id, provider, remote_root_path, remote_root_id, sync_mode, plaintext, created_at, updated_at
            FROM cloud_space_bindings
            WHERE provider = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(provider)
        .fetch_all(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        rows.into_iter().map(row_to_cloud_space_binding).collect()
    }

    pub async fn save_remote_file_metadata(&self, metadata: &RemoteFileMetadata) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO remote_file_metadata
                (space_id, provider, remote_path, local_relative_path, remote_file_id, is_directory, size, md5, server_mtime, remote_revision, last_remote_file_id, last_remote_md5, last_remote_size, last_remote_server_mtime, last_remote_revision, last_local_hash, last_local_modified_at, last_local_size, last_seen_at, last_synced_at, tombstone)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(space_id, provider, local_relative_path) DO UPDATE SET
                remote_path = excluded.remote_path,
                remote_file_id = excluded.remote_file_id,
                is_directory = excluded.is_directory,
                size = excluded.size,
                md5 = excluded.md5,
                server_mtime = excluded.server_mtime,
                remote_revision = excluded.remote_revision,
                last_remote_file_id = excluded.last_remote_file_id,
                last_remote_md5 = excluded.last_remote_md5,
                last_remote_size = excluded.last_remote_size,
                last_remote_server_mtime = excluded.last_remote_server_mtime,
                last_remote_revision = excluded.last_remote_revision,
                last_local_hash = excluded.last_local_hash,
                last_local_modified_at = excluded.last_local_modified_at,
                last_local_size = excluded.last_local_size,
                last_seen_at = excluded.last_seen_at,
                last_synced_at = excluded.last_synced_at,
                tombstone = excluded.tombstone
            "#,
        )
        .bind(metadata.space_id.to_string())
        .bind(&metadata.provider)
        .bind(&metadata.remote_path)
        .bind(&metadata.local_relative_path)
        .bind(&metadata.remote_file_id)
        .bind(metadata.is_directory)
        .bind(metadata.size as i64)
        .bind(&metadata.md5)
        .bind(metadata.server_mtime.map(|t| t.to_rfc3339()))
        .bind(&metadata.remote_revision)
        .bind(&metadata.last_remote_file_id)
        .bind(&metadata.last_remote_md5)
        .bind(metadata.last_remote_size.map(|size| size as i64))
        .bind(metadata.last_remote_server_mtime.map(|t| t.to_rfc3339()))
        .bind(&metadata.last_remote_revision)
        .bind(&metadata.last_local_hash)
        .bind(metadata.last_local_modified_at.map(|t| t.to_rfc3339()))
        .bind(metadata.last_local_size.map(|size| size as i64))
        .bind(metadata.last_seen_at.to_rfc3339())
        .bind(metadata.last_synced_at.map(|t| t.to_rfc3339()))
        .bind(metadata.tombstone)
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(())
    }

    pub async fn get_remote_file_metadata(
        &self,
        space_id: &Uuid,
        provider: &str,
        local_relative_path: &str,
    ) -> Result<Option<RemoteFileMetadata>> {
        let row = sqlx::query(
            r#"
            SELECT space_id, provider, remote_path, local_relative_path, remote_file_id, is_directory, size, md5, server_mtime, remote_revision, last_remote_file_id, last_remote_md5, last_remote_size, last_remote_server_mtime, last_remote_revision, last_local_hash, last_local_modified_at, last_local_size, last_seen_at, last_synced_at, tombstone
            FROM remote_file_metadata
            WHERE space_id = ? AND provider = ? AND local_relative_path = ?
            "#,
        )
        .bind(space_id.to_string())
        .bind(provider)
        .bind(local_relative_path)
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        row.map(row_to_remote_file_metadata).transpose()
    }

    pub async fn list_remote_file_metadata(
        &self,
        space_id: &Uuid,
        provider: &str,
    ) -> Result<Vec<RemoteFileMetadata>> {
        let rows = sqlx::query(
            r#"
            SELECT space_id, provider, remote_path, local_relative_path, remote_file_id, is_directory, size, md5, server_mtime, remote_revision, last_remote_file_id, last_remote_md5, last_remote_size, last_remote_server_mtime, last_remote_revision, last_local_hash, last_local_modified_at, last_local_size, last_seen_at, last_synced_at, tombstone
            FROM remote_file_metadata
            WHERE space_id = ? AND provider = ?
            ORDER BY local_relative_path ASC
            "#,
        )
        .bind(space_id.to_string())
        .bind(provider)
        .fetch_all(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        rows.into_iter().map(row_to_remote_file_metadata).collect()
    }

    pub async fn enqueue_cloud_sync_task(&self, task: &CloudSyncTask) -> Result<i64> {
        if let Some(row) = sqlx::query(
            r#"
            SELECT id
            FROM cloud_sync_tasks
            WHERE space_id = ?
              AND provider = ?
              AND task_kind = ?
              AND local_relative_path = ?
              AND remote_path = ?
            ORDER BY created_at ASC
            LIMIT 1
            "#,
        )
        .bind(task.space_id.to_string())
        .bind(&task.provider)
        .bind(&task.task_kind)
        .bind(&task.local_relative_path)
        .bind(&task.remote_path)
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?
        {
            let id = row.try_get::<i64, _>("id").unwrap_or_default();
            sqlx::query(
                r#"
                UPDATE cloud_sync_tasks
                SET expected_remote_revision = ?,
                    payload_json = ?,
                    attempts = ?,
                    last_error = ?,
                    updated_at = ?,
                    next_attempt_at = ?
                WHERE id = ?
                "#,
            )
            .bind(&task.expected_remote_revision)
            .bind(&task.payload_json)
            .bind(task.attempts as i64)
            .bind(&task.last_error)
            .bind(task.updated_at.to_rfc3339())
            .bind(task.next_attempt_at.map(|t| t.to_rfc3339()))
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(SyncFlowError::Database)?;
            return Ok(id);
        }

        let result = sqlx::query(
            r#"
            INSERT INTO cloud_sync_tasks
                (space_id, provider, task_kind, local_relative_path, remote_path, expected_remote_revision, payload_json, attempts, last_error, created_at, updated_at, next_attempt_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(task.space_id.to_string())
        .bind(&task.provider)
        .bind(&task.task_kind)
        .bind(&task.local_relative_path)
        .bind(&task.remote_path)
        .bind(&task.expected_remote_revision)
        .bind(&task.payload_json)
        .bind(task.attempts as i64)
        .bind(&task.last_error)
        .bind(task.created_at.to_rfc3339())
        .bind(task.updated_at.to_rfc3339())
        .bind(task.next_attempt_at.map(|t| t.to_rfc3339()))
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(result.last_insert_rowid())
    }

    pub async fn get_due_cloud_sync_tasks(
        &self,
        provider: &str,
        now: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<CloudSyncTask>> {
        let rows = sqlx::query(
            r#"
            SELECT id, space_id, provider, task_kind, local_relative_path, remote_path, expected_remote_revision, payload_json, attempts, last_error, created_at, updated_at, next_attempt_at
            FROM cloud_sync_tasks
            WHERE provider = ? AND (next_attempt_at IS NULL OR next_attempt_at <= ?)
            ORDER BY created_at ASC
            LIMIT ?
            "#,
        )
        .bind(provider)
        .bind(now.to_rfc3339())
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        rows.into_iter().map(row_to_cloud_sync_task).collect()
    }

    pub async fn list_cloud_sync_tasks_for_space(
        &self,
        space_id: &Uuid,
        provider: &str,
        limit: u32,
    ) -> Result<Vec<CloudSyncTask>> {
        let rows = sqlx::query(
            r#"
            SELECT id, space_id, provider, task_kind, local_relative_path, remote_path, expected_remote_revision, payload_json, attempts, last_error, created_at, updated_at, next_attempt_at
            FROM cloud_sync_tasks
            WHERE space_id = ? AND provider = ?
            ORDER BY created_at ASC
            LIMIT ?
            "#,
        )
        .bind(space_id.to_string())
        .bind(provider)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        rows.into_iter().map(row_to_cloud_sync_task).collect()
    }

    pub async fn remove_cloud_sync_task(&self, task_id: i64) -> Result<bool> {
        let result = sqlx::query("DELETE FROM cloud_sync_tasks WHERE id = ?")
            .bind(task_id)
            .execute(&self.pool)
            .await
            .map_err(SyncFlowError::Database)?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn retry_cloud_sync_task(
        &self,
        task_id: i64,
        space_id: &Uuid,
        provider: &str,
        retry_at: DateTime<Utc>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE cloud_sync_tasks
            SET attempts = 0, last_error = NULL, updated_at = ?, next_attempt_at = ?
            WHERE id = ? AND space_id = ? AND provider = ?
            "#,
        )
        .bind(Utc::now().to_rfc3339())
        .bind(retry_at.to_rfc3339())
        .bind(task_id)
        .bind(space_id.to_string())
        .bind(provider)
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn remove_cloud_sync_task_for_space(
        &self,
        task_id: i64,
        space_id: &Uuid,
        provider: &str,
    ) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM cloud_sync_tasks WHERE id = ? AND space_id = ? AND provider = ?",
        )
        .bind(task_id)
        .bind(space_id.to_string())
        .bind(provider)
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_cloud_sync_task_failed(
        &self,
        task_id: i64,
        attempts: u32,
        last_error: &str,
        next_attempt_at: Option<DateTime<Utc>>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE cloud_sync_tasks
            SET attempts = ?, last_error = ?, updated_at = ?, next_attempt_at = ?
            WHERE id = ?
            "#,
        )
        .bind(attempts as i64)
        .bind(last_error)
        .bind(Utc::now().to_rfc3339())
        .bind(next_attempt_at.map(|value| value.to_rfc3339()))
        .bind(task_id)
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn count_cloud_sync_tasks_for_space(
        &self,
        space_id: &Uuid,
        provider: &str,
    ) -> Result<u64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM cloud_sync_tasks WHERE space_id = ? AND provider = ?",
        )
        .bind(space_id.to_string())
        .bind(provider)
        .fetch_one(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(count as u64)
    }

    pub async fn update_account_last_unlocked_at(
        &self,
        account_id: &Uuid,
        unlocked_at: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query("UPDATE accounts SET last_unlocked_at = ? WHERE account_id = ?")
            .bind(unlocked_at.to_rfc3339())
            .bind(account_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(SyncFlowError::Database)?;
        Ok(())
    }

    pub async fn add_synced_space(&self, space: &SyncedSpace) -> Result<SyncedSpace> {
        let existing = self.get_synced_space_by_root_path(&space.root_path).await?;
        if let Some(existing) = existing {
            return Ok(existing);
        }

        sqlx::query(
            r#"
            INSERT INTO synced_spaces (id, sync_key, name, root_path, status, created_at, last_scanned_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(space.id.to_string())
        .bind(&space.sync_key)
        .bind(&space.name)
        .bind(&space.root_path)
        .bind(&space.status)
        .bind(space.created_at.to_rfc3339())
        .bind(space.last_scanned_at.map(|t| t.to_rfc3339()))
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        Ok(space.clone())
    }

    pub async fn get_synced_spaces(&self) -> Result<Vec<SyncedSpace>> {
        let rows = sqlx::query(
            "SELECT id, sync_key, name, root_path, status, created_at, last_scanned_at FROM synced_spaces ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        rows.into_iter().map(row_to_synced_space).collect()
    }

    pub async fn get_synced_space(&self, id: &Uuid) -> Result<Option<SyncedSpace>> {
        let row = sqlx::query(
            "SELECT id, sync_key, name, root_path, status, created_at, last_scanned_at FROM synced_spaces WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        row.map(row_to_synced_space).transpose()
    }

    pub async fn get_synced_space_by_sync_key(
        &self,
        sync_key: &str,
    ) -> Result<Option<SyncedSpace>> {
        let row = sqlx::query(
            "SELECT id, sync_key, name, root_path, status, created_at, last_scanned_at FROM synced_spaces WHERE sync_key = ?",
        )
        .bind(sync_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        row.map(row_to_synced_space).transpose()
    }

    pub async fn remove_synced_space(&self, id: &Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM synced_spaces WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(SyncFlowError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    async fn get_synced_space_by_root_path(&self, root_path: &str) -> Result<Option<SyncedSpace>> {
        let row = sqlx::query(
            "SELECT id, sync_key, name, root_path, status, created_at, last_scanned_at FROM synced_spaces WHERE root_path = ?",
        )
        .bind(root_path)
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        row.map(row_to_synced_space).transpose()
    }

    pub async fn save_file_meta(&self, meta: &FileMetadata) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO file_metadata (space_id, relative_path, hash, size, modified_at, version_vector, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(space_id, relative_path) DO UPDATE SET
                hash = excluded.hash, size = excluded.size,
                modified_at = excluded.modified_at, version_vector = excluded.version_vector
            "#,
        )
        .bind(meta.space_id.to_string())
        .bind(&meta.relative_path)
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

    pub async fn get_file_meta(
        &self,
        space_id: &Uuid,
        relative_path: &str,
    ) -> Result<Option<FileMetadata>> {
        let row = sqlx::query(
            "SELECT space_id, relative_path, hash, size, modified_at, version_vector, created_at FROM file_metadata WHERE space_id = ? AND relative_path = ?",
        )
        .bind(space_id.to_string())
        .bind(relative_path)
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        row.map(row_to_file_metadata).transpose()
    }

    pub async fn remove_file_meta(&self, space_id: &Uuid, relative_path: &str) -> Result<bool> {
        let result =
            sqlx::query("DELETE FROM file_metadata WHERE space_id = ? AND relative_path = ?")
                .bind(space_id.to_string())
                .bind(relative_path)
                .execute(&self.pool)
                .await
                .map_err(SyncFlowError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn count_files_for_space(&self, space_id: &Uuid) -> Result<u64> {
        let row = sqlx::query("SELECT COUNT(*) AS count FROM file_metadata WHERE space_id = ?")
            .bind(space_id.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(SyncFlowError::Database)?;

        Ok(row.try_get::<i64, _>("count").unwrap_or(0) as u64)
    }

    pub async fn update_space_last_scanned_at(
        &self,
        space_id: &Uuid,
        scanned_at: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query("UPDATE synced_spaces SET last_scanned_at = ? WHERE id = ?")
            .bind(scanned_at.to_rfc3339())
            .bind(space_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(SyncFlowError::Database)?;
        Ok(())
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
                peer_id: r
                    .try_get("peer_id")
                    .ok()
                    .and_then(|s: String| Uuid::parse_str(&s).ok())
                    .unwrap_or_default(),
                last_sync_at: r
                    .try_get::<Option<String>, _>("last_sync_at")
                    .ok()
                    .flatten()
                    .map(|s| parse_rfc3339(&s)),
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

        Ok(rows
            .into_iter()
            .map(|r| FileVersion {
                file_path: r.try_get("file_path").unwrap(),
                hash: r.try_get("hash").unwrap(),
                version_vector: r.try_get("version_vector").unwrap(),
                device_id: r.try_get("device_id").unwrap(),
                is_conflict: r.try_get("is_conflict").unwrap(),
                created_at: parse_rfc3339(&r.try_get::<String, _>("created_at").unwrap()),
            })
            .collect())
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

    pub async fn save_conflict(&self, conflict: &SyncConflict) -> Result<()> {
        let existing = sqlx::query(
            r#"
            SELECT id, space_id, relative_path, local_version, remote_version, remote_device_id, detected_at
            FROM sync_conflicts
            WHERE space_id = ?
              AND relative_path = ?
              AND local_version = ?
              AND remote_version = ?
              AND remote_device_id = ?
            ORDER BY detected_at DESC
            LIMIT 1
            "#,
        )
        .bind(conflict.space_id.to_string())
        .bind(&conflict.relative_path)
        .bind(&conflict.local_version)
        .bind(&conflict.remote_version)
        .bind(&conflict.remote_device_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        if existing.is_some() {
            return Ok(());
        }

        sqlx::query(
            r#"
            INSERT INTO sync_conflicts (space_id, relative_path, local_version, remote_version, remote_device_id, detected_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(conflict.space_id.to_string())
        .bind(&conflict.relative_path)
        .bind(&conflict.local_version)
        .bind(&conflict.remote_version)
        .bind(&conflict.remote_device_id)
        .bind(conflict.detected_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(())
    }

    pub async fn find_matching_conflict(
        &self,
        conflict: &SyncConflict,
    ) -> Result<Option<SyncConflict>> {
        let row = sqlx::query(
            r#"
            SELECT id, space_id, relative_path, local_version, remote_version, remote_device_id, detected_at
            FROM sync_conflicts
            WHERE space_id = ?
              AND relative_path = ?
              AND local_version = ?
              AND remote_version = ?
              AND remote_device_id = ?
            ORDER BY detected_at DESC
            LIMIT 1
            "#,
        )
        .bind(conflict.space_id.to_string())
        .bind(&conflict.relative_path)
        .bind(&conflict.local_version)
        .bind(&conflict.remote_version)
        .bind(&conflict.remote_device_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        row.map(row_to_sync_conflict).transpose()
    }

    pub async fn get_conflict_by_id(&self, conflict_id: i64) -> Result<Option<SyncConflict>> {
        let row = sqlx::query(
            "SELECT id, space_id, relative_path, local_version, remote_version, remote_device_id, detected_at FROM sync_conflicts WHERE id = ?",
        )
        .bind(conflict_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        row.map(row_to_sync_conflict).transpose()
    }

    pub async fn remove_conflict(&self, conflict_id: i64) -> Result<bool> {
        let mut tx = self.pool.begin().await.map_err(SyncFlowError::Database)?;
        sqlx::query("DELETE FROM sync_conflict_snapshots WHERE conflict_id = ?")
            .bind(conflict_id)
            .execute(&mut *tx)
            .await
            .map_err(SyncFlowError::Database)?;
        let result = sqlx::query("DELETE FROM sync_conflicts WHERE id = ?")
            .bind(conflict_id)
            .execute(&mut *tx)
            .await
            .map_err(SyncFlowError::Database)?;
        tx.commit().await.map_err(SyncFlowError::Database)?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_conflicts_for_space(&self, space_id: &Uuid) -> Result<Vec<SyncConflict>> {
        let rows = sqlx::query(
            "SELECT id, space_id, relative_path, local_version, remote_version, remote_device_id, detected_at FROM sync_conflicts WHERE space_id = ? ORDER BY detected_at DESC",
        )
        .bind(space_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        rows.into_iter().map(row_to_sync_conflict).collect()
    }

    pub async fn get_all_conflicts(&self) -> Result<Vec<SyncConflict>> {
        let rows = sqlx::query(
            "SELECT id, space_id, relative_path, local_version, remote_version, remote_device_id, detected_at FROM sync_conflicts ORDER BY detected_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        rows.into_iter().map(row_to_sync_conflict).collect()
    }

    pub async fn count_conflicts_for_space(&self, space_id: &Uuid) -> Result<u64> {
        let row = sqlx::query("SELECT COUNT(*) AS count FROM sync_conflicts WHERE space_id = ?")
            .bind(space_id.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(SyncFlowError::Database)?;

        Ok(row.try_get::<i64, _>("count").unwrap_or(0) as u64)
    }

    pub async fn save_conflict_snapshot(&self, snapshot: &ConflictSnapshot) -> Result<()> {
        let existing = sqlx::query(
            r#"
            SELECT id
            FROM sync_conflict_snapshots
            WHERE conflict_id = ? AND snapshot_kind = ?
            LIMIT 1
            "#,
        )
        .bind(snapshot.conflict_id)
        .bind(&snapshot.snapshot_kind)
        .fetch_optional(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        if existing.is_some() {
            return Ok(());
        }

        sqlx::query(
            r#"
            INSERT INTO sync_conflict_snapshots (
                conflict_id, space_id, relative_path, snapshot_kind,
                content_text, content_truncated, content_size, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(snapshot.conflict_id)
        .bind(snapshot.space_id.to_string())
        .bind(&snapshot.relative_path)
        .bind(&snapshot.snapshot_kind)
        .bind(&snapshot.content_text)
        .bind(snapshot.content_truncated)
        .bind(snapshot.content_size as i64)
        .bind(snapshot.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;
        Ok(())
    }

    pub async fn get_conflict_snapshots(&self, conflict_id: i64) -> Result<Vec<ConflictSnapshot>> {
        let rows = sqlx::query(
            r#"
            SELECT id, conflict_id, space_id, relative_path, snapshot_kind, content_text, content_truncated, content_size, created_at
            FROM sync_conflict_snapshots
            WHERE conflict_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(conflict_id)
        .fetch_all(&self.pool)
        .await
        .map_err(SyncFlowError::Database)?;

        rows.into_iter().map(row_to_conflict_snapshot).collect()
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

fn row_to_account_record(row: sqlx::sqlite::SqliteRow) -> Result<AccountRecord> {
    let account_id_str: String = row.try_get("account_id").map_err(SyncFlowError::Database)?;
    let account_id = Uuid::parse_str(&account_id_str)
        .map_err(|e| SyncFlowError::Database(sqlx::Error::Decode(Box::new(e))))?;
    let created_at: String = row.try_get("created_at").map_err(SyncFlowError::Database)?;
    let last_unlocked_at = row
        .try_get::<Option<String>, _>("last_unlocked_at")
        .map_err(SyncFlowError::Database)?
        .map(|t| parse_rfc3339(&t));

    Ok(AccountRecord {
        account_id,
        display_name: row
            .try_get("display_name")
            .map_err(SyncFlowError::Database)?,
        password_salt: row
            .try_get("password_salt")
            .map_err(SyncFlowError::Database)?,
        encrypted_account_secret: row
            .try_get("encrypted_account_secret")
            .map_err(SyncFlowError::Database)?,
        created_at: parse_rfc3339(&created_at),
        last_unlocked_at,
    })
}

fn row_to_file_metadata(row: sqlx::sqlite::SqliteRow) -> Result<FileMetadata> {
    let space_id_str: String = row.try_get("space_id").map_err(SyncFlowError::Database)?;
    let space_id = Uuid::parse_str(&space_id_str)
        .map_err(|e| SyncFlowError::Database(sqlx::Error::Decode(Box::new(e))))?;

    Ok(FileMetadata {
        space_id,
        relative_path: row
            .try_get("relative_path")
            .map_err(SyncFlowError::Database)?,
        hash: row.try_get("hash").map_err(SyncFlowError::Database)?,
        size: row
            .try_get::<i64, _>("size")
            .map_err(SyncFlowError::Database)? as u64,
        modified_at: parse_rfc3339(
            &row.try_get::<String, _>("modified_at")
                .map_err(SyncFlowError::Database)?,
        ),
        version_vector: row
            .try_get("version_vector")
            .map_err(SyncFlowError::Database)?,
        created_at: parse_rfc3339(
            &row.try_get::<String, _>("created_at")
                .map_err(SyncFlowError::Database)?,
        ),
    })
}

fn row_to_sync_conflict(row: sqlx::sqlite::SqliteRow) -> Result<SyncConflict> {
    let space_id_str: String = row.try_get("space_id").map_err(SyncFlowError::Database)?;
    let space_id = Uuid::parse_str(&space_id_str)
        .map_err(|e| SyncFlowError::Database(sqlx::Error::Decode(Box::new(e))))?;

    Ok(SyncConflict {
        id: row.try_get("id").map_err(SyncFlowError::Database)?,
        space_id,
        relative_path: row
            .try_get("relative_path")
            .map_err(SyncFlowError::Database)?,
        local_version: row
            .try_get("local_version")
            .map_err(SyncFlowError::Database)?,
        remote_version: row
            .try_get("remote_version")
            .map_err(SyncFlowError::Database)?,
        remote_device_id: row
            .try_get("remote_device_id")
            .map_err(SyncFlowError::Database)?,
        detected_at: parse_rfc3339(
            &row.try_get::<String, _>("detected_at")
                .map_err(SyncFlowError::Database)?,
        ),
    })
}

fn row_to_conflict_snapshot(row: sqlx::sqlite::SqliteRow) -> Result<ConflictSnapshot> {
    let space_id_str: String = row.try_get("space_id").map_err(SyncFlowError::Database)?;
    let space_id = Uuid::parse_str(&space_id_str)
        .map_err(|e| SyncFlowError::Database(sqlx::Error::Decode(Box::new(e))))?;

    Ok(ConflictSnapshot {
        id: row.try_get("id").map_err(SyncFlowError::Database)?,
        conflict_id: row
            .try_get("conflict_id")
            .map_err(SyncFlowError::Database)?,
        space_id,
        relative_path: row
            .try_get("relative_path")
            .map_err(SyncFlowError::Database)?,
        snapshot_kind: row
            .try_get("snapshot_kind")
            .map_err(SyncFlowError::Database)?,
        content_text: row
            .try_get("content_text")
            .map_err(SyncFlowError::Database)?,
        content_truncated: row
            .try_get("content_truncated")
            .map_err(SyncFlowError::Database)?,
        content_size: row
            .try_get::<i64, _>("content_size")
            .map_err(SyncFlowError::Database)? as u64,
        created_at: parse_rfc3339(
            &row.try_get::<String, _>("created_at")
                .map_err(SyncFlowError::Database)?,
        ),
    })
}

fn row_to_synced_space(row: sqlx::sqlite::SqliteRow) -> Result<SyncedSpace> {
    let id_str: String = row.try_get("id").map_err(SyncFlowError::Database)?;
    let id = Uuid::parse_str(&id_str)
        .map_err(|e| SyncFlowError::Database(sqlx::Error::Decode(Box::new(e))))?;
    let created_at: String = row.try_get("created_at").map_err(SyncFlowError::Database)?;
    let last_scanned_at = row
        .try_get::<Option<String>, _>("last_scanned_at")
        .map_err(SyncFlowError::Database)?
        .map(|t| parse_rfc3339(&t));

    Ok(SyncedSpace {
        id,
        sync_key: row.try_get("sync_key").map_err(SyncFlowError::Database)?,
        name: row.try_get("name").map_err(SyncFlowError::Database)?,
        root_path: row.try_get("root_path").map_err(SyncFlowError::Database)?,
        status: row.try_get("status").map_err(SyncFlowError::Database)?,
        created_at: parse_rfc3339(&created_at),
        last_scanned_at,
    })
}

fn row_to_cloud_api_config(row: sqlx::sqlite::SqliteRow) -> Result<CloudApiConfig> {
    let scopes_json: String = row.try_get("scopes").map_err(SyncFlowError::Database)?;
    let scopes = serde_json::from_str(&scopes_json)
        .map_err(|e| SyncFlowError::Cloud(format!("invalid cloud API config scopes: {e}")))?;
    Ok(CloudApiConfig {
        provider: row.try_get("provider").map_err(SyncFlowError::Database)?,
        device_id: row.try_get("device_id").map_err(SyncFlowError::Database)?,
        client_id: row.try_get("client_id").map_err(SyncFlowError::Database)?,
        client_secret: row
            .try_get("client_secret")
            .map_err(SyncFlowError::Database)?,
        redirect_uri: row
            .try_get("redirect_uri")
            .map_err(SyncFlowError::Database)?,
        scopes,
        created_at: parse_rfc3339(
            &row.try_get::<String, _>("created_at")
                .map_err(SyncFlowError::Database)?,
        ),
        updated_at: parse_rfc3339(
            &row.try_get::<String, _>("updated_at")
                .map_err(SyncFlowError::Database)?,
        ),
    })
}

fn row_to_cloud_account(row: sqlx::sqlite::SqliteRow) -> Result<CloudAccount> {
    let scopes_json: String = row.try_get("scopes").map_err(SyncFlowError::Database)?;
    let scopes = serde_json::from_str(&scopes_json)
        .map_err(|e| SyncFlowError::Cloud(format!("invalid cloud account scopes: {e}")))?;
    Ok(CloudAccount {
        provider: row.try_get("provider").map_err(SyncFlowError::Database)?,
        account_id: row.try_get("account_id").map_err(SyncFlowError::Database)?,
        display_name: row
            .try_get("display_name")
            .map_err(SyncFlowError::Database)?,
        access_token_encrypted: row
            .try_get("access_token_encrypted")
            .map_err(SyncFlowError::Database)?,
        refresh_token_encrypted: row
            .try_get("refresh_token_encrypted")
            .map_err(SyncFlowError::Database)?,
        expires_at: row
            .try_get::<Option<String>, _>("expires_at")
            .map_err(SyncFlowError::Database)?
            .map(|t| parse_rfc3339(&t)),
        scopes,
        created_at: parse_rfc3339(
            &row.try_get::<String, _>("created_at")
                .map_err(SyncFlowError::Database)?,
        ),
        updated_at: parse_rfc3339(
            &row.try_get::<String, _>("updated_at")
                .map_err(SyncFlowError::Database)?,
        ),
    })
}

fn row_to_cloud_space_binding(row: sqlx::sqlite::SqliteRow) -> Result<CloudSpaceBinding> {
    let space_id = parse_uuid_from_row(&row, "space_id")?;
    Ok(CloudSpaceBinding {
        space_id,
        provider: row.try_get("provider").map_err(SyncFlowError::Database)?,
        remote_root_path: row
            .try_get("remote_root_path")
            .map_err(SyncFlowError::Database)?,
        remote_root_id: row
            .try_get("remote_root_id")
            .map_err(SyncFlowError::Database)?,
        sync_mode: row.try_get("sync_mode").map_err(SyncFlowError::Database)?,
        plaintext: row.try_get("plaintext").map_err(SyncFlowError::Database)?,
        created_at: parse_rfc3339(
            &row.try_get::<String, _>("created_at")
                .map_err(SyncFlowError::Database)?,
        ),
        updated_at: parse_rfc3339(
            &row.try_get::<String, _>("updated_at")
                .map_err(SyncFlowError::Database)?,
        ),
    })
}

fn row_to_remote_file_metadata(row: sqlx::sqlite::SqliteRow) -> Result<RemoteFileMetadata> {
    let space_id = parse_uuid_from_row(&row, "space_id")?;
    Ok(RemoteFileMetadata {
        space_id,
        provider: row.try_get("provider").map_err(SyncFlowError::Database)?,
        remote_path: row
            .try_get("remote_path")
            .map_err(SyncFlowError::Database)?,
        local_relative_path: row
            .try_get("local_relative_path")
            .map_err(SyncFlowError::Database)?,
        remote_file_id: row
            .try_get("remote_file_id")
            .map_err(SyncFlowError::Database)?,
        is_directory: row
            .try_get("is_directory")
            .map_err(SyncFlowError::Database)?,
        size: row
            .try_get::<i64, _>("size")
            .map_err(SyncFlowError::Database)? as u64,
        md5: row.try_get("md5").map_err(SyncFlowError::Database)?,
        server_mtime: row
            .try_get::<Option<String>, _>("server_mtime")
            .map_err(SyncFlowError::Database)?
            .map(|t| parse_rfc3339(&t)),
        remote_revision: row
            .try_get("remote_revision")
            .map_err(SyncFlowError::Database)?,
        last_remote_file_id: row
            .try_get("last_remote_file_id")
            .map_err(SyncFlowError::Database)?,
        last_remote_md5: row
            .try_get("last_remote_md5")
            .map_err(SyncFlowError::Database)?,
        last_remote_size: row
            .try_get::<Option<i64>, _>("last_remote_size")
            .map_err(SyncFlowError::Database)?
            .map(|size| size as u64),
        last_remote_server_mtime: row
            .try_get::<Option<String>, _>("last_remote_server_mtime")
            .map_err(SyncFlowError::Database)?
            .map(|t| parse_rfc3339(&t)),
        last_remote_revision: row
            .try_get("last_remote_revision")
            .map_err(SyncFlowError::Database)?,
        last_local_hash: row
            .try_get("last_local_hash")
            .map_err(SyncFlowError::Database)?,
        last_local_modified_at: row
            .try_get::<Option<String>, _>("last_local_modified_at")
            .map_err(SyncFlowError::Database)?
            .map(|t| parse_rfc3339(&t)),
        last_local_size: row
            .try_get::<Option<i64>, _>("last_local_size")
            .map_err(SyncFlowError::Database)?
            .map(|size| size as u64),
        last_seen_at: parse_rfc3339(
            &row.try_get::<String, _>("last_seen_at")
                .map_err(SyncFlowError::Database)?,
        ),
        last_synced_at: row
            .try_get::<Option<String>, _>("last_synced_at")
            .map_err(SyncFlowError::Database)?
            .map(|t| parse_rfc3339(&t)),
        tombstone: row.try_get("tombstone").map_err(SyncFlowError::Database)?,
    })
}

fn row_to_cloud_sync_task(row: sqlx::sqlite::SqliteRow) -> Result<CloudSyncTask> {
    let space_id = parse_uuid_from_row(&row, "space_id")?;
    Ok(CloudSyncTask {
        id: row.try_get("id").map_err(SyncFlowError::Database)?,
        space_id,
        provider: row.try_get("provider").map_err(SyncFlowError::Database)?,
        task_kind: row.try_get("task_kind").map_err(SyncFlowError::Database)?,
        local_relative_path: row
            .try_get("local_relative_path")
            .map_err(SyncFlowError::Database)?,
        remote_path: row
            .try_get("remote_path")
            .map_err(SyncFlowError::Database)?,
        expected_remote_revision: row
            .try_get("expected_remote_revision")
            .map_err(SyncFlowError::Database)?,
        payload_json: row
            .try_get("payload_json")
            .map_err(SyncFlowError::Database)?,
        attempts: row
            .try_get::<i64, _>("attempts")
            .map_err(SyncFlowError::Database)? as u32,
        last_error: row.try_get("last_error").map_err(SyncFlowError::Database)?,
        created_at: parse_rfc3339(
            &row.try_get::<String, _>("created_at")
                .map_err(SyncFlowError::Database)?,
        ),
        updated_at: parse_rfc3339(
            &row.try_get::<String, _>("updated_at")
                .map_err(SyncFlowError::Database)?,
        ),
        next_attempt_at: row
            .try_get::<Option<String>, _>("next_attempt_at")
            .map_err(SyncFlowError::Database)?
            .map(|t| parse_rfc3339(&t)),
    })
}

fn parse_uuid_from_row(row: &sqlx::sqlite::SqliteRow, column: &str) -> Result<Uuid> {
    let value: String = row.try_get(column).map_err(SyncFlowError::Database)?;
    Uuid::parse_str(&value).map_err(|e| SyncFlowError::Database(sqlx::Error::Decode(Box::new(e))))
}

fn parse_rfc3339(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .unwrap_or_else(|_| Utc::now().into())
        .with_timezone(&Utc)
}
