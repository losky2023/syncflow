use chrono::{DateTime, Utc};
use std::sync::Arc;
use syncflow_core::storage::SpaceId;
use syncflow_core::sync::SyncEngine;
use tokio::task::JoinHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeStatus {
    Stopped,
    Starting,
    Indexing,
    Watching,
    #[allow(dead_code)]
    Syncing,
    Error,
}

impl RuntimeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Indexing => "indexing",
            Self::Watching => "watching",
            Self::Syncing => "syncing",
            Self::Error => "error",
        }
    }

    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Stopped | Self::Error)
    }
}

pub struct SpaceRuntime {
    pub space_id: SpaceId,
    pub root_path: String,
    pub status: RuntimeStatus,
    pub file_count: u64,
    pub pending_count: u64,
    pub conflict_count: u64,
    pub cloud_conflict_count: u64,
    pub connected_peer_count: u64,
    pub discovered_peer_count: u64,
    pub cloud_provider: Option<String>,
    pub cloud_remote_path: Option<String>,
    pub last_cloud_scan_at: Option<DateTime<Utc>>,
    pub last_indexed_at: Option<DateTime<Utc>>,
    pub last_transport_event: Option<String>,
    pub last_transport_event_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub engine: Option<Arc<SyncEngine>>,
    pub watcher_task: Option<JoinHandle<()>>,
    pub queue_task: Option<JoinHandle<()>>,
}

impl SpaceRuntime {
    pub fn new(space_id: SpaceId, root_path: String) -> Self {
        Self {
            space_id,
            root_path,
            status: RuntimeStatus::Stopped,
            file_count: 0,
            pending_count: 0,
            conflict_count: 0,
            cloud_conflict_count: 0,
            connected_peer_count: 0,
            discovered_peer_count: 0,
            cloud_provider: None,
            cloud_remote_path: None,
            last_cloud_scan_at: None,
            last_indexed_at: None,
            last_transport_event: None,
            last_transport_event_at: None,
            last_error: None,
            engine: None,
            watcher_task: None,
            queue_task: None,
        }
    }
}
