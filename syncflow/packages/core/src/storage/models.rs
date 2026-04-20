use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type FolderId = Uuid;
pub type DeviceId = Uuid;

/// Metadata about a synced file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub path: String,
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
