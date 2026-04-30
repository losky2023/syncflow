use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRuntimeStatusDto {
    pub space_id: String,
    pub status: String,
    pub file_count: u64,
    pub pending_count: u64,
    pub conflict_count: u64,
    pub cloud_conflict_count: u64,
    pub connected_peer_count: u64,
    pub discovered_peer_count: u64,
    pub cloud_provider: Option<String>,
    pub cloud_remote_path: Option<String>,
    pub last_cloud_scan_at: Option<String>,
    pub last_indexed_at: Option<String>,
    pub last_transport_event: Option<String>,
    pub last_transport_event_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStateDto {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub state: String,
    pub ip: Option<String>,
    pub last_seen_at: Option<String>,
}
