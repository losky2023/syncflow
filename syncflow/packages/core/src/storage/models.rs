use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type FolderId = Uuid;
pub type DeviceId = Uuid;
pub type SpaceId = Uuid;
pub type AccountId = Uuid;

/// Local account identity. Passwords only unlock encrypted account_secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    pub account_id: AccountId,
    pub display_name: String,
    pub password_salt: Vec<u8>,
    pub encrypted_account_secret: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub last_unlocked_at: Option<DateTime<Utc>>,
}

/// A registered local sync space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncedSpace {
    pub id: SpaceId,
    pub sync_key: String,
    pub name: String,
    pub root_path: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub last_scanned_at: Option<DateTime<Utc>>,
}

/// Metadata about a synced file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub space_id: SpaceId,
    pub relative_path: String,
    pub hash: String,
    pub size: u64,
    pub modified_at: DateTime<Utc>,
    pub version_vector: String,
    pub created_at: DateTime<Utc>,
}

/// Sync state with a specific peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub peer_id: DeviceId,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub sync_status: SyncStatus,
    pub pending_changes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum SyncStatus {
    #[default]
    Idle,
    Syncing,
    Conflict,
    Error,
}

/// Version history entry for a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileVersion {
    pub file_path: String,
    pub hash: String,
    pub version_vector: String,
    pub device_id: String,
    pub is_conflict: bool,
    pub created_at: DateTime<Utc>,
}

/// Known device information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: DeviceId,
    pub device_name: String,
    pub platform: String,
    pub public_key: String,
    pub last_seen_at: Option<DateTime<Utc>>,
}

/// Persisted conflict record for a synced file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConflict {
    pub id: i64,
    pub space_id: SpaceId,
    pub relative_path: String,
    pub local_version: String,
    pub remote_version: String,
    pub remote_device_id: String,
    pub detected_at: DateTime<Utc>,
}

/// Stored snapshot payload for a conflict.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConflictSnapshot {
    pub id: i64,
    pub conflict_id: i64,
    pub space_id: SpaceId,
    pub relative_path: String,
    pub snapshot_kind: String,
    pub content_text: Option<String>,
    pub content_truncated: bool,
    pub content_size: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudApiConfig {
    pub provider: String,
    pub device_id: Option<String>,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudAccount {
    pub provider: String,
    pub account_id: Option<String>,
    pub display_name: Option<String>,
    pub access_token_encrypted: Vec<u8>,
    pub refresh_token_encrypted: Vec<u8>,
    pub expires_at: Option<DateTime<Utc>>,
    pub scopes: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudSpaceBinding {
    pub space_id: SpaceId,
    pub provider: String,
    pub remote_root_path: String,
    pub remote_root_id: Option<String>,
    pub sync_mode: String,
    pub plaintext: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteFileMetadata {
    pub space_id: SpaceId,
    pub provider: String,
    pub remote_path: String,
    pub local_relative_path: String,
    pub remote_file_id: Option<String>,
    pub is_directory: bool,
    pub size: u64,
    pub md5: Option<String>,
    pub server_mtime: Option<DateTime<Utc>>,
    pub remote_revision: Option<String>,
    pub last_remote_file_id: Option<String>,
    pub last_remote_md5: Option<String>,
    pub last_remote_size: Option<u64>,
    pub last_remote_server_mtime: Option<DateTime<Utc>>,
    pub last_remote_revision: Option<String>,
    pub last_local_hash: Option<String>,
    pub last_local_modified_at: Option<DateTime<Utc>>,
    pub last_local_size: Option<u64>,
    pub last_seen_at: DateTime<Utc>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub tombstone: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudSyncTask {
    pub id: i64,
    pub space_id: SpaceId,
    pub provider: String,
    pub task_kind: String,
    pub local_relative_path: String,
    pub remote_path: String,
    pub expected_remote_revision: Option<String>,
    pub payload_json: Option<String>,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub next_attempt_at: Option<DateTime<Utc>>,
}
