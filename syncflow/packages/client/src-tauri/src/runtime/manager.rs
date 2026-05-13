use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use syncflow_core::cloud::{
    BaiduNetdiskProvider, BaiduOAuthConfig, CloudProvider, CloudRemoteEntry, FakeCloudProvider,
    BAIDU_PROVIDER,
};
use syncflow_core::crypto::hash_data;
use syncflow_core::storage::{
    CloudSpaceBinding, CloudSyncTask, ConflictSnapshot, FileMetadata, RemoteFileMetadata, SpaceId,
    StorageEngine, SyncConflict,
};
use syncflow_core::sync::{start_watcher, FileEvent, SyncEngine, VersionVector};
use syncflow_core::transport::{DiscoveredDevice, TransportEvent, TransportLayer};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::dto::{DeviceStateDto, SyncRuntimeStatusDto};
use super::space_runtime::{RuntimeStatus, SpaceRuntime};

const CONTROL_SPACE_READY: &str = "space_ready";
const SYNCFLOW_META_DIR: &str = ".syncflow";
const SYNCFLOW_MANIFEST_FILE: &str = "manifest.json";
const SYNCFLOW_MANIFEST_VERSION: u32 = 1;
const CLOUD_REMOTE_DELETED_DEVICE_ID: &str = "baidu_netdisk:remote_deleted";
const CLOUD_TASK_TIMEOUT_SECONDS: u64 = 45;
const CLOUD_SCAN_DOWNLOAD_TIMEOUT_SECONDS: u64 = 20;

#[derive(Debug, Default)]
pub struct SessionSyncContext {
    account_id: Option<Uuid>,
    account_secret: Option<[u8; 32]>,
    root_key: Option<[u8; 32]>,
    device_name: Option<String>,
}

impl SessionSyncContext {
    pub fn initialize(
        &mut self,
        account_id: Uuid,
        account_secret: [u8; 32],
        root_key: [u8; 32],
        device_name: String,
    ) {
        self.account_id = Some(account_id);
        self.account_secret = Some(account_secret);
        self.root_key = Some(root_key);
        self.device_name = Some(device_name);
    }

    pub fn root_key(&self) -> Option<[u8; 32]> {
        self.root_key
    }

    pub fn account_id(&self) -> Option<Uuid> {
        self.account_id
    }

    pub fn account_secret(&self) -> Option<[u8; 32]> {
        self.account_secret
    }

    pub fn clear(&mut self) {
        self.account_id = None;
        self.account_secret = None;
        self.root_key = None;
        self.device_name = None;
    }
}

pub struct SyncRuntimeManager {
    storage: Arc<StorageEngine>,
    transport: Arc<TransportLayer>,
    device_id: String,
    runtimes: Mutex<HashMap<SpaceId, SpaceRuntime>>,
}

impl SyncRuntimeManager {
    pub fn new(
        storage: Arc<StorageEngine>,
        transport: Arc<TransportLayer>,
        device_id: String,
    ) -> Self {
        Self {
            storage,
            transport,
            device_id,
            runtimes: Mutex::new(HashMap::new()),
        }
    }

    pub async fn start_space(&self, space_id: SpaceId) -> Result<SyncRuntimeStatusDto, String> {
        let space = self
            .storage
            .get_synced_space(&space_id)
            .await
            .map_err(|e| format!("Failed to load synced space: {e}"))?
            .ok_or_else(|| "Synced space not found".to_string())?;

        {
            let mut runtimes = self.runtimes.lock().await;
            let runtime = runtimes
                .entry(space_id)
                .or_insert_with(|| SpaceRuntime::new(space_id, space.root_path.clone()));
            refresh_runtime_liveness(runtime);
            if runtime.status.is_active() {
                return Ok(status_to_dto(runtime));
            }
            runtime.status = RuntimeStatus::Starting;
            runtime.root_path = space.root_path.clone();
            runtime.last_error = None;
        }

        let result = self.start_space_inner(space_id, &space.root_path).await;
        if let Err(error) = result {
            let mut runtimes = self.runtimes.lock().await;
            let runtime = runtimes
                .entry(space_id)
                .or_insert_with(|| SpaceRuntime::new(space_id, space.root_path));
            runtime.status = RuntimeStatus::Error;
            runtime.last_error = Some(error.clone());
            return Err(error);
        }

        self.notify_space_ready(&space.sync_key).await;
        self.get_status(space_id).await
    }

    pub async fn start_cloud_spaces(&self) {
        let bindings = match self
            .storage
            .get_cloud_space_bindings_for_provider(BAIDU_PROVIDER)
            .await
        {
            Ok(bindings) => bindings,
            Err(error) => {
                tracing::warn!("failed to load cloud spaces for auto-start: {error}");
                return;
            }
        };
        for binding in bindings {
            let space_exists = self
                .storage
                .get_synced_space(&binding.space_id)
                .await
                .ok()
                .flatten()
                .is_some();
            if !space_exists {
                tracing::warn!(
                    "skipping orphaned cloud binding for missing space {}",
                    binding.space_id
                );
                continue;
            }
            if let Err(error) = self.start_space(binding.space_id).await {
                tracing::warn!(
                    "failed to auto-start cloud space {}: {error}",
                    binding.space_id
                );
            }
        }
    }

    pub async fn stop_space(&self, space_id: SpaceId) -> Result<SyncRuntimeStatusDto, String> {
        let mut runtimes = self.runtimes.lock().await;
        let runtime = runtimes
            .entry(space_id)
            .or_insert_with(|| SpaceRuntime::new(space_id, String::new()));
        if let Some(task) = runtime.watcher_task.take() {
            task.abort();
        }
        if let Some(task) = runtime.queue_task.take() {
            task.abort();
        }
        runtime.engine = None;
        runtime.status = RuntimeStatus::Stopped;
        runtime.last_error = None;
        Ok(status_to_dto(runtime))
    }

    pub async fn stop_all(&self) {
        let mut runtimes = self.runtimes.lock().await;
        for runtime in runtimes.values_mut() {
            if let Some(task) = runtime.watcher_task.take() {
                task.abort();
            }
            if let Some(task) = runtime.queue_task.take() {
                task.abort();
            }
            runtime.engine = None;
            runtime.status = RuntimeStatus::Stopped;
        }
    }

    pub async fn get_status(&self, space_id: SpaceId) -> Result<SyncRuntimeStatusDto, String> {
        self.refresh_counts(space_id).await;
        let mut runtimes = self.runtimes.lock().await;
        let root_path = if runtimes.contains_key(&space_id) {
            None
        } else {
            Some(
                self.storage
                    .get_synced_space(&space_id)
                    .await
                    .map_err(|e| format!("Failed to load synced space: {e}"))?
                    .map(|space| space.root_path)
                    .unwrap_or_default(),
            )
        };
        if let Some(root_path) = root_path {
            runtimes.insert(space_id, SpaceRuntime::new(space_id, root_path));
        }
        if let Some(runtime) = runtimes.get_mut(&space_id) {
            refresh_runtime_liveness(runtime);
        }
        Ok(status_to_dto(
            runtimes.get(&space_id).expect("runtime inserted"),
        ))
    }

    pub async fn get_all_statuses(&self) -> Result<Vec<SyncRuntimeStatusDto>, String> {
        let spaces = self
            .storage
            .get_synced_spaces()
            .await
            .map_err(|e| format!("Failed to load synced spaces: {e}"))?;
        for space in &spaces {
            self.refresh_counts(space.id).await;
        }

        let mut runtimes = self.runtimes.lock().await;
        for space in spaces {
            runtimes
                .entry(space.id)
                .or_insert_with(|| SpaceRuntime::new(space.id, space.root_path));
        }
        for runtime in runtimes.values_mut() {
            refresh_runtime_liveness(runtime);
        }
        Ok(runtimes.values().map(status_to_dto).collect())
    }

    pub async fn aggregate_devices(
        &self,
        self_device_id: &Uuid,
    ) -> Result<Vec<DeviceStateDto>, String> {
        let known = self
            .storage
            .get_known_devices()
            .await
            .map_err(|e| format!("Failed to load device list: {e}"))?;
        let discovered = self.transport.get_discovered_devices().await;
        let connected = self.transport.connected_peers().await;
        let now = Utc::now();
        let mut devices: HashMap<String, DeviceStateDto> = HashMap::new();

        for device in known {
            if device.device_id == *self_device_id {
                continue;
            }
            devices.insert(
                device.device_id.to_string(),
                DeviceStateDto {
                    device_id: device.device_id.to_string(),
                    device_name: device.device_name,
                    platform: device.platform,
                    state: "offline".to_string(),
                    ip: None,
                    last_seen_at: device.last_seen_at.map(|value| value.to_rfc3339()),
                },
            );
        }

        for device in discovered {
            if device.device_id == self_device_id.to_string() {
                continue;
            }
            self.save_discovered_device(&device, now).await;
            devices.insert(
                device.device_id.clone(),
                DeviceStateDto {
                    device_id: device.device_id,
                    device_name: device.device_name,
                    platform: device.platform,
                    state: "discovered".to_string(),
                    ip: Some(device.ip),
                    last_seen_at: Some(now.to_rfc3339()),
                },
            );
        }

        for peer_id in connected {
            if peer_id == self_device_id.to_string() {
                continue;
            }
            devices
                .entry(peer_id.clone())
                .and_modify(|device| {
                    device.state = "connected".to_string();
                    device.last_seen_at = Some(now.to_rfc3339());
                })
                .or_insert_with(|| DeviceStateDto {
                    device_id: peer_id.clone(),
                    device_name: peer_id,
                    platform: "unknown".to_string(),
                    state: "connected".to_string(),
                    ip: None,
                    last_seen_at: Some(now.to_rfc3339()),
                });
        }

        let mut values: Vec<_> = devices.into_values().collect();
        values.sort_by(|left, right| left.device_name.cmp(&right.device_name));
        Ok(values)
    }

    async fn start_space_inner(&self, space_id: SpaceId, root_path: &str) -> Result<(), String> {
        let root = std::fs::canonicalize(root_path)
            .map_err(|e| format!("Sync space root is not accessible: {e}"))?;
        ensure_local_manifest_dir(&root).await?;
        self.set_status(space_id, RuntimeStatus::Indexing, None)
            .await;
        let engine = Arc::new(SyncEngine::new(
            self.storage.clone(),
            self.transport.clone(),
            self.device_id.clone(),
        ));
        let indexed_files = index_root(&self.storage, &engine, space_id, &root).await?;
        let cloud_binding = self
            .storage
            .get_cloud_space_binding(&space_id, BAIDU_PROVIDER)
            .await
            .map_err(|e| format!("Failed to load cloud sync binding: {e}"))?;
        import_local_sync_manifest(&self.storage, space_id, &root).await?;
        if let Some(binding) = &cloud_binding {
            if let Ok(provider) = create_cloud_provider(&self.storage).await {
                if let Err(error) = import_cloud_sync_manifest(
                    self.storage.as_ref(),
                    provider.as_ref(),
                    space_id,
                    &root,
                    binding,
                )
                .await
                {
                    tracing::warn!("failed to import cloud sync manifest: {error}");
                }
            }
        }
        let file_count = indexed_files.len() as u64;
        let indexed_at = Utc::now();
        write_local_sync_manifest(&self.storage, space_id, &root, cloud_binding.as_ref()).await?;
        self.storage
            .update_space_last_scanned_at(&space_id, indexed_at)
            .await
            .map_err(|e| format!("Failed to update scan time: {e}"))?;

        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<FileEvent>(100);
        let watcher = start_watcher(vec![root.clone()], event_tx)
            .map_err(|e| format!("Failed to start file watcher: {e}"))?;
        let storage = self.storage.clone();
        let engine_for_task = engine.clone();
        let cloud_binding_for_task = cloud_binding.clone();
        let task_space_id = space_id;
        let task_root = root.clone();
        let watcher_task = tokio::spawn(async move {
            let _watcher = watcher;
            while let Some(event) = event_rx.recv().await {
                let path = PathBuf::from(event.path());
                let Ok(relative_path) = strip_root_prefix(&task_root, &path) else {
                    continue;
                };
                if is_syncflow_metadata_path(&relative_path) {
                    continue;
                }
                if is_ignored_local_sync_relative_path(&relative_path) {
                    continue;
                }
                if let Some(binding) = &cloud_binding_for_task {
                    if matches!(event, FileEvent::Modified(_)) && path.exists() && path.is_dir() {
                        continue;
                    }
                    if let Err(error) = enqueue_cloud_task_for_file_event(
                        &storage,
                        binding,
                        task_space_id,
                        &relative_path,
                        &event,
                    )
                    .await
                    {
                        tracing::warn!("failed to enqueue cloud sync task: {error}");
                    }
                } else if let Err(error) = engine_for_task
                    .handle_space_file_event(task_space_id, &relative_path, &path, &event)
                    .await
                {
                    tracing::warn!("failed to process file event: {error}");
                }
                let _ = storage.count_files_for_space(&task_space_id).await;
            }
        });
        let queue_task = if cloud_binding.is_some() {
            let cloud_storage = self.storage.clone();
            let cloud_root = root.clone();
            let cloud_binding_for_queue = cloud_binding.clone();
            let cloud_device_id = self.device_id.clone();
            Some(tokio::spawn(async move {
                loop {
                    if let Err(error) = process_due_cloud_tasks_for_provider(
                        cloud_storage.as_ref(),
                        task_space_id,
                        &cloud_root,
                        cloud_binding_for_queue.as_ref(),
                        &cloud_device_id,
                    )
                    .await
                    {
                        tracing::warn!("failed to process cloud sync queue: {error}");
                    }
                    if let Some(binding) = &cloud_binding_for_queue {
                        if let Err(error) = scan_remote_cloud_changes_for_provider(
                            cloud_storage.as_ref(),
                            task_space_id,
                            &cloud_root,
                            binding,
                        )
                        .await
                        {
                            tracing::warn!("failed to scan cloud sync changes: {error}");
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }))
        } else {
            let queue_engine = engine.clone();
            Some(tokio::spawn(async move {
                loop {
                    if let Err(error) = queue_engine.process_queue().await {
                        tracing::warn!("failed to process sync queue: {error}");
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                }
            }))
        };

        if let Some(binding) = &cloud_binding {
            enqueue_existing_directories_for_cloud(&self.storage, binding, space_id, &root).await?;
            enqueue_existing_files_for_cloud(&self.storage, binding, space_id, &indexed_files)
                .await?;
        } else {
            let connected_peers = self.transport.connected_peers().await;
            enqueue_existing_files_for_peers(&engine, space_id, &indexed_files, connected_peers)
                .await;
        }

        let conflict_count = self.count_lan_conflicts(space_id).await;
        cleanup_redundant_cloud_conflicts(&self.storage, space_id).await?;
        let cloud_conflict_count = self.count_cloud_conflicts(space_id).await;
        let mut runtimes = self.runtimes.lock().await;
        let runtime = runtimes
            .entry(space_id)
            .or_insert_with(|| SpaceRuntime::new(space_id, root_path.to_string()));
        if let Some(task) = runtime.watcher_task.take() {
            task.abort();
        }
        runtime.status = RuntimeStatus::Watching;
        runtime.file_count = file_count;
        runtime.pending_count = self
            .storage
            .count_cloud_sync_tasks_for_space(&space_id, BAIDU_PROVIDER)
            .await
            .unwrap_or(0);
        runtime.conflict_count = conflict_count;
        runtime.cloud_conflict_count = cloud_conflict_count;
        runtime.cloud_provider = cloud_binding
            .as_ref()
            .map(|binding| binding.provider.clone());
        runtime.cloud_remote_path = cloud_binding
            .as_ref()
            .map(|binding| binding.remote_root_path.clone());
        runtime.last_indexed_at = Some(indexed_at);
        runtime.last_error = None;
        runtime.engine = Some(engine);
        runtime.watcher_task = Some(watcher_task);
        runtime.queue_task = queue_task;
        Ok(())
    }

    async fn set_status(&self, space_id: SpaceId, status: RuntimeStatus, error: Option<String>) {
        let mut runtimes = self.runtimes.lock().await;
        if let Some(runtime) = runtimes.get_mut(&space_id) {
            runtime.status = status;
            runtime.last_error = error;
        }
    }

    async fn refresh_counts(&self, space_id: SpaceId) {
        let file_count = self
            .storage
            .count_files_for_space(&space_id)
            .await
            .unwrap_or(0);
        let conflict_count = self.count_lan_conflicts(space_id).await;
        let cloud_conflict_count = self.count_cloud_conflicts(space_id).await;
        let pending_count = self
            .storage
            .count_cloud_sync_tasks_for_space(&space_id, BAIDU_PROVIDER)
            .await
            .unwrap_or(0);
        let connected_peer_count = self.transport.connected_peers().await.len() as u64;
        let discovered_peer_count = self.transport.get_discovered_devices().await.len() as u64;
        let mut runtimes = self.runtimes.lock().await;
        if let Some(runtime) = runtimes.get_mut(&space_id) {
            runtime.file_count = file_count;
            runtime.pending_count = pending_count;
            runtime.conflict_count = conflict_count;
            runtime.cloud_conflict_count = cloud_conflict_count;
            runtime.connected_peer_count = connected_peer_count;
            runtime.discovered_peer_count = discovered_peer_count;
        }
    }

    pub async fn refresh_space_counts(&self, space_id: SpaceId) {
        self.refresh_counts(space_id).await;
    }

    async fn count_cloud_conflicts(&self, space_id: SpaceId) -> u64 {
        self.storage
            .get_conflicts_for_space(&space_id)
            .await
            .map(|conflicts| {
                conflicts
                    .into_iter()
                    .filter(|conflict| conflict.remote_device_id == BAIDU_PROVIDER)
                    .count() as u64
            })
            .unwrap_or(0)
    }

    async fn count_lan_conflicts(&self, space_id: SpaceId) -> u64 {
        let _ = space_id;
        0
    }

    pub async fn resolve_cloud_conflict_keep_local(
        &self,
        conflict_id: i64,
    ) -> Result<bool, String> {
        let conflict = self
            .storage
            .get_conflict_by_id(conflict_id)
            .await
            .map_err(|e| format!("Failed to load cloud conflict: {e}"))?
            .ok_or_else(|| "Cloud conflict not found".to_string())?;
        if conflict.remote_device_id != BAIDU_PROVIDER {
            return Err("This is not a cloud conflict".to_string());
        }
        let space = self
            .storage
            .get_synced_space(&conflict.space_id)
            .await
            .map_err(|e| format!("Failed to load synced space: {e}"))?
            .ok_or_else(|| "Synced space not found".to_string())?;
        let binding = self
            .storage
            .get_cloud_space_binding(&conflict.space_id, BAIDU_PROVIDER)
            .await
            .map_err(|e| format!("Failed to load cloud binding: {e}"))?
            .ok_or_else(|| "This space is not bound to Baidu Netdisk".to_string())?;
        let root = std::fs::canonicalize(&space.root_path)
            .map_err(|e| format!("Sync space root is not accessible: {e}"))?;
        let local_path = safe_local_task_path(&root, &conflict.relative_path)?;
        let local_baseline = read_local_cloud_baseline(&local_path)
            .await?
            .ok_or_else(|| "Local file is missing".to_string())?;
        let provider = create_cloud_provider(&self.storage).await?;
        let remote_path = join_remote_path(&binding.remote_root_path, &conflict.relative_path)?;
        let upload_result = provider
            .upload_file(&local_path, &remote_path, None)
            .await
            .map_err(|e| format!("Failed to upload local file to resolve cloud conflict: {e}"))?;
        let task = cloud_metadata_task(
            &binding,
            conflict.space_id,
            &conflict.relative_path,
            &remote_path,
        );
        self.storage
            .save_remote_file_metadata(&remote_metadata_from_entry(
                &task,
                &upload_result.entry,
                Some(Utc::now()),
                false,
                Some(local_baseline),
            ))
            .await
            .map_err(|e| format!("Failed to save resolved cloud metadata: {e}"))?;
        write_local_sync_manifest(&self.storage, conflict.space_id, &root, Some(&binding)).await?;
        sync_manifest_to_cloud(
            &self.storage,
            provider.as_ref(),
            conflict.space_id,
            &root,
            &binding,
            &self.device_id,
        )
        .await?;
        let removed = self
            .storage
            .remove_conflict(conflict_id)
            .await
            .map_err(|e| format!("Failed to remove cloud conflict: {e}"))?;
        self.refresh_space_counts(conflict.space_id).await;
        Ok(removed)
    }

    pub async fn resolve_cloud_conflict_keep_remote(
        &self,
        conflict_id: i64,
    ) -> Result<bool, String> {
        let conflict = self
            .storage
            .get_conflict_by_id(conflict_id)
            .await
            .map_err(|e| format!("Failed to load cloud conflict: {e}"))?
            .ok_or_else(|| "Cloud conflict not found".to_string())?;
        if conflict.remote_device_id != BAIDU_PROVIDER {
            return Err("This is not a cloud conflict".to_string());
        }
        let space = self
            .storage
            .get_synced_space(&conflict.space_id)
            .await
            .map_err(|e| format!("Failed to load synced space: {e}"))?
            .ok_or_else(|| "Synced space not found".to_string())?;
        let binding = self
            .storage
            .get_cloud_space_binding(&conflict.space_id, BAIDU_PROVIDER)
            .await
            .map_err(|e| format!("Failed to load cloud binding: {e}"))?
            .ok_or_else(|| "This space is not bound to Baidu Netdisk".to_string())?;
        let root = std::fs::canonicalize(&space.root_path)
            .map_err(|e| format!("Sync space root is not accessible: {e}"))?;
        let local_path = safe_local_task_path(&root, &conflict.relative_path)?;
        let provider = create_cloud_provider(&self.storage).await?;
        let remote_path = join_remote_path(&binding.remote_root_path, &conflict.relative_path)?;
        let entry = provider
            .get_metadata(&remote_path)
            .await
            .map_err(|e| format!("Failed to inspect cloud file: {e}"))?
            .ok_or_else(|| "Cloud file no longer exists".to_string())?;
        let _ =
            download_cloud_file_if_changed(provider.as_ref(), &entry, &root, &local_path).await?;
        index_downloaded_cloud_file(
            &self.storage,
            conflict.space_id,
            &conflict.relative_path,
            &local_path,
        )
        .await?;
        let task = cloud_metadata_task(
            &binding,
            conflict.space_id,
            &conflict.relative_path,
            &remote_path,
        );
        self.storage
            .save_remote_file_metadata(&remote_metadata_from_entry(
                &task,
                &entry,
                Some(Utc::now()),
                false,
                read_local_cloud_baseline(&local_path).await?,
            ))
            .await
            .map_err(|e| format!("Failed to save resolved cloud metadata: {e}"))?;
        write_local_sync_manifest(&self.storage, conflict.space_id, &root, Some(&binding)).await?;
        sync_manifest_to_cloud(
            &self.storage,
            provider.as_ref(),
            conflict.space_id,
            &root,
            &binding,
            &self.device_id,
        )
        .await?;
        let removed = self
            .storage
            .remove_conflict(conflict_id)
            .await
            .map_err(|e| format!("Failed to remove cloud conflict: {e}"))?;
        self.refresh_space_counts(conflict.space_id).await;
        Ok(removed)
    }

    async fn save_discovered_device(
        &self,
        device: &DiscoveredDevice,
        seen_at: chrono::DateTime<Utc>,
    ) {
        if let Ok(device_id) = Uuid::parse_str(&device.device_id) {
            let info = syncflow_core::storage::DeviceInfo {
                device_id,
                device_name: device.device_name.clone(),
                platform: device.platform.clone(),
                public_key: String::new(),
                last_seen_at: Some(seen_at),
            };
            let _ = self.storage.save_device_info(&info).await;
        }
    }

    pub async fn handle_transport_event(&self, event: TransportEvent) {
        match event {
            TransportEvent::PeerConnected { device_id } => {
                self.record_transport_event(format!("peer connected: {device_id}"))
                    .await;
                self.notify_active_spaces_ready_for_peer(&device_id).await;
                if let Err(error) = self.enqueue_existing_files_for_peer(&device_id).await {
                    tracing::warn!(
                        "failed to enqueue existing files for peer {device_id}: {error}"
                    );
                    self.record_transport_error(error).await;
                }
            }
            TransportEvent::PeerDisconnected { device_id } => {
                self.record_transport_event(format!("peer disconnected: {device_id}"))
                    .await;
            }
            TransportEvent::DataReceived { from, data } => {
                self.record_transport_event(format!(
                    "data received from {from}: {} bytes",
                    data.len()
                ))
                .await;
                if let Err(error) = self.handle_incoming_transport_data(&from, &data).await {
                    tracing::warn!("failed to handle incoming transport payload: {error}");
                    self.record_transport_error(error).await;
                }
            }
        }
    }

    async fn enqueue_existing_files_for_peer(&self, peer_id: &str) -> Result<(), String> {
        let snapshots = {
            let runtimes = self.runtimes.lock().await;
            runtimes
                .values()
                .filter(|runtime| runtime.status.is_active())
                .filter_map(|runtime| {
                    runtime
                        .engine
                        .clone()
                        .map(|engine| (runtime.space_id, runtime.root_path.clone(), engine))
                })
                .collect::<Vec<_>>()
        };

        for (space_id, root_path, engine) in snapshots {
            let root = std::fs::canonicalize(&root_path)
                .map_err(|e| format!("Sync space root is not accessible: {e}"))?;
            let files = collect_indexed_files(&root)?;
            enqueue_existing_files_for_peers(&engine, space_id, &files, vec![peer_id.to_string()])
                .await;
        }

        Ok(())
    }

    async fn enqueue_existing_files_for_peer_and_sync_key(
        &self,
        peer_id: &str,
        sync_key: &str,
    ) -> Result<(), String> {
        let Some(space) = self
            .storage
            .get_synced_space_by_sync_key(sync_key)
            .await
            .map_err(|e| format!("Failed to load synced space by sync key: {e}"))?
        else {
            return Ok(());
        };

        let snapshot = {
            let runtimes = self.runtimes.lock().await;
            runtimes.get(&space.id).and_then(|runtime| {
                if !runtime.status.is_active() {
                    return None;
                }
                runtime
                    .engine
                    .clone()
                    .map(|engine| (runtime.space_id, runtime.root_path.clone(), engine))
            })
        };

        let Some((space_id, root_path, engine)) = snapshot else {
            return Ok(());
        };

        let root = std::fs::canonicalize(&root_path)
            .map_err(|e| format!("Sync space root is not accessible: {e}"))?;
        let files = collect_indexed_files(&root)?;
        enqueue_existing_files_for_peers(&engine, space_id, &files, vec![peer_id.to_string()])
            .await;
        tracing::info!(
            "queued existing files for ready space {} to {}",
            space_id,
            peer_id
        );
        Ok(())
    }

    async fn notify_space_ready(&self, sync_key: &str) {
        let peers = self.transport.connected_peers().await;
        for peer_id in peers {
            self.notify_space_ready_for_peer(sync_key, &peer_id).await;
        }
    }

    async fn notify_active_spaces_ready_for_peer(&self, peer_id: &str) {
        let spaces = self
            .storage
            .get_synced_spaces()
            .await
            .unwrap_or_else(|_| Vec::new());
        let active_space_ids = {
            let runtimes = self.runtimes.lock().await;
            runtimes
                .values()
                .filter(|runtime| runtime.status.is_active())
                .map(|runtime| runtime.space_id)
                .collect::<std::collections::HashSet<_>>()
        };

        for space in spaces {
            if active_space_ids.contains(&space.id) {
                self.notify_space_ready_for_peer(&space.sync_key, peer_id)
                    .await;
            }
        }
    }

    async fn notify_space_ready_for_peer(&self, sync_key: &str, peer_id: &str) {
        let data = serde_json::json!({
            "type": CONTROL_SPACE_READY,
            "sync_key": sync_key,
        })
        .to_string()
        .into_bytes();

        if let Err(error) = self.transport.send_data(peer_id, &data).await {
            tracing::warn!("failed to send space ready to {peer_id}: {error}");
        }
    }

    async fn record_transport_event(&self, message: String) {
        let connected = self.transport.connected_peers().await.len() as u64;
        let discovered = self.transport.get_discovered_devices().await.len() as u64;
        let now = Utc::now();
        let mut runtimes = self.runtimes.lock().await;
        for runtime in runtimes.values_mut() {
            runtime.connected_peer_count = connected;
            runtime.discovered_peer_count = discovered;
            runtime.last_transport_event = Some(message.clone());
            runtime.last_transport_event_at = Some(now);
        }
        tracing::info!("{message}");
    }

    async fn record_transport_error(&self, error: String) {
        let mut runtimes = self.runtimes.lock().await;
        for runtime in runtimes.values_mut() {
            runtime.last_error = Some(error.clone());
        }
    }

    async fn handle_incoming_transport_data(&self, from: &str, data: &[u8]) -> Result<(), String> {
        let Some(sync_key) = extract_sync_key(data) else {
            return Ok(());
        };
        let Some(space) = self
            .storage
            .get_synced_space_by_sync_key(&sync_key)
            .await
            .map_err(|e| format!("Failed to load synced space by sync key: {e}"))?
        else {
            return Ok(());
        };
        let space_id = space.id;

        if is_control_message(data, CONTROL_SPACE_READY) {
            self.enqueue_existing_files_for_peer_and_sync_key(from, &sync_key)
                .await?;
            return Ok(());
        }

        let runtime_snapshot = {
            let runtimes = self.runtimes.lock().await;
            runtimes.get(&space_id).and_then(|runtime| {
                runtime
                    .engine
                    .clone()
                    .map(|engine| (runtime.root_path.clone(), engine, runtime.status.clone()))
            })
        };

        let Some((root_path, engine, status)) = runtime_snapshot else {
            return Ok(());
        };

        if !status.is_active() {
            return Ok(());
        }

        let root = std::fs::canonicalize(&root_path)
            .map_err(|e| format!("Sync space root is not accessible: {e}"))?;
        engine
            .receive_space_file(from, Some(&root), Some(space_id), data)
            .await
            .map_err(|e| format!("Failed to process remote sync data: {e}"))?;

        self.refresh_counts(space_id).await;
        Ok(())
    }
}

async fn index_root(
    storage: &StorageEngine,
    engine: &SyncEngine,
    space_id: SpaceId,
    root: &Path,
) -> Result<Vec<(String, PathBuf)>, String> {
    let files = collect_indexed_files(root)?;
    for (relative_path, file) in &files {
        let content = tokio::fs::read(file)
            .await
            .map_err(|e| format!("Failed to read file: {e}"))?;
        let vv = VersionVector::new("index")
            .to_json()
            .map_err(|e| e.to_string())?;
        let meta = FileMetadata {
            space_id,
            relative_path: relative_path.clone(),
            hash: hash_data(&content),
            size: content.len() as u64,
            modified_at: Utc::now(),
            version_vector: vv,
            created_at: Utc::now(),
        };
        storage
            .save_file_meta(&meta)
            .await
            .map_err(|e| format!("Failed to save file metadata: {e}"))?;
        let _ = engine
            .index_local_file(space_id, relative_path, file)
            .await
            .map_err(|e| format!("Failed to index file: {e}"))?;
    }
    Ok(files)
}

fn collect_indexed_files(root: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let files = collect_files(root)?;
    files
        .into_iter()
        .map(|file| strip_root_prefix(root, &file).map(|relative_path| (relative_path, file)))
        .filter(|result| {
            result
                .as_ref()
                .map(|(relative_path, _)| !is_ignored_local_sync_relative_path(relative_path))
                .unwrap_or(true)
        })
        .collect()
}

async fn enqueue_existing_files_for_peers(
    engine: &SyncEngine,
    space_id: SpaceId,
    files: &[(String, PathBuf)],
    peer_ids: Vec<String>,
) {
    for peer_id in peer_ids {
        for (relative_path, file) in files {
            engine
                .enqueue_existing_file_for_peer(space_id, relative_path, file, peer_id.clone())
                .await;
        }
    }
}

async fn enqueue_existing_files_for_cloud(
    storage: &StorageEngine,
    binding: &CloudSpaceBinding,
    space_id: SpaceId,
    files: &[(String, PathBuf)],
) -> Result<(), String> {
    for (relative_path, local_path) in files {
        let existing = storage
            .get_remote_file_metadata(&space_id, &binding.provider, relative_path)
            .await
            .map_err(|e| format!("Failed to load remote metadata: {e}"))?;
        if existing
            .as_ref()
            .map(|metadata| !metadata.tombstone)
            .unwrap_or(false)
            && !local_file_changed_since_cloud_sync(existing.as_ref(), local_path).await?
        {
            continue;
        }
        enqueue_cloud_task(storage, binding, space_id, relative_path, "upload", None).await?;
    }
    Ok(())
}

async fn enqueue_existing_directories_for_cloud(
    storage: &StorageEngine,
    binding: &CloudSpaceBinding,
    space_id: SpaceId,
    root: &Path,
) -> Result<(), String> {
    for relative_path in collect_directories(root)? {
        if directory_has_synced_child_file(storage, binding, space_id, &relative_path).await? {
            continue;
        }
        let existing = storage
            .get_remote_file_metadata(&space_id, &binding.provider, &relative_path)
            .await
            .map_err(|e| format!("Failed to load remote directory metadata: {e}"))?;
        if existing
            .as_ref()
            .map(|metadata| !metadata.tombstone)
            .unwrap_or(false)
        {
            continue;
        }
        enqueue_cloud_task(storage, binding, space_id, &relative_path, "mkdir", None).await?;
    }
    Ok(())
}

async fn directory_has_synced_child_file(
    storage: &StorageEngine,
    binding: &CloudSpaceBinding,
    space_id: SpaceId,
    relative_path: &str,
) -> Result<bool, String> {
    let prefix = format!("{}/", relative_path.trim_end_matches('/'));
    let known = storage
        .list_remote_file_metadata(&space_id, &binding.provider)
        .await
        .map_err(|e| format!("Failed to load remote metadata: {e}"))?;
    Ok(known.into_iter().any(|metadata| {
        !metadata.tombstone
            && !metadata.is_directory
            && metadata.local_relative_path.starts_with(&prefix)
    }))
}

async fn enqueue_cloud_task_for_file_event(
    storage: &StorageEngine,
    binding: &CloudSpaceBinding,
    space_id: SpaceId,
    relative_path: &str,
    event: &FileEvent,
) -> Result<i64, String> {
    let event_path = PathBuf::from(event.path());
    if matches!(event, FileEvent::Deleted(_)) {
        let existing = storage
            .get_remote_file_metadata(&space_id, &binding.provider, relative_path)
            .await
            .map_err(|e| format!("Failed to load remote metadata: {e}"))?;
        if existing
            .as_ref()
            .map(|metadata| metadata.is_directory)
            .unwrap_or(false)
        {
            tracing::warn!(
                "cloud directory deletion detected for {}, remote deletion is disabled",
                relative_path
            );
            return Ok(0);
        }
    }
    let task_kind = match event {
        FileEvent::Created(_) if event_path.is_dir() => "mkdir",
        FileEvent::Created(_) | FileEvent::Modified(_) => "upload",
        FileEvent::Deleted(_) => "delete",
    };
    let existing = storage
        .get_remote_file_metadata(&space_id, &binding.provider, relative_path)
        .await
        .map_err(|e| format!("Failed to load remote metadata: {e}"))?;
    if task_kind == "upload"
        && existing
            .as_ref()
            .map(|metadata| !metadata.tombstone)
            .unwrap_or(false)
        && !local_file_changed_since_cloud_sync(existing.as_ref(), &event_path).await?
    {
        return Ok(0);
    }
    if task_kind == "mkdir"
        && existing
            .as_ref()
            .map(|metadata| metadata.is_directory && !metadata.tombstone)
            .unwrap_or(false)
    {
        return Ok(0);
    }
    let expected_remote_revision = existing.and_then(|metadata| metadata.remote_revision);
    enqueue_cloud_task(
        storage,
        binding,
        space_id,
        relative_path,
        task_kind,
        expected_remote_revision,
    )
    .await
}

async fn enqueue_cloud_task(
    storage: &StorageEngine,
    binding: &CloudSpaceBinding,
    space_id: SpaceId,
    relative_path: &str,
    task_kind: &str,
    expected_remote_revision: Option<String>,
) -> Result<i64, String> {
    let now = Utc::now();
    let task = CloudSyncTask {
        id: 0,
        space_id,
        provider: binding.provider.clone(),
        task_kind: task_kind.to_string(),
        local_relative_path: relative_path.to_string(),
        remote_path: join_remote_path(&binding.remote_root_path, relative_path)?,
        expected_remote_revision,
        payload_json: None,
        attempts: 0,
        last_error: None,
        created_at: now,
        updated_at: now,
        next_attempt_at: Some(now),
    };
    storage
        .enqueue_cloud_sync_task(&task)
        .await
        .map_err(|e| format!("Failed to enqueue cloud sync task: {e}"))
}

async fn process_due_cloud_tasks(
    storage: &StorageEngine,
    provider: &dyn CloudProvider,
    space_id: SpaceId,
    root: &Path,
) -> Result<(), String> {
    let tasks = storage
        .get_due_cloud_sync_tasks(BAIDU_PROVIDER, Utc::now(), 25)
        .await
        .map_err(|e| format!("Failed to load cloud sync tasks: {e}"))?;
    for task in tasks.into_iter().filter(|task| task.space_id == space_id) {
        tracing::info!(
            "processing cloud sync task {} {} {}",
            task.id,
            task.task_kind,
            task.local_relative_path
        );
        let outcome = tokio::time::timeout(
            std::time::Duration::from_secs(CLOUD_TASK_TIMEOUT_SECONDS),
            process_cloud_task(storage, provider, root, &task),
        )
        .await
        .map_err(|_| {
            format!(
                "Cloud sync task timed out after {}s",
                CLOUD_TASK_TIMEOUT_SECONDS
            )
        })
        .and_then(|result| result);
        if let Err(error) = outcome {
            let next_attempt =
                Utc::now() + chrono::Duration::seconds(cloud_retry_delay_seconds(task.attempts));
            storage
                .mark_cloud_sync_task_failed(
                    task.id,
                    task.attempts.saturating_add(1),
                    &error,
                    Some(next_attempt),
                )
                .await
                .map_err(|e| format!("Failed to update cloud sync task failure state: {e}"))?;
        }
    }
    Ok(())
}

async fn process_due_cloud_tasks_for_provider(
    storage: &StorageEngine,
    space_id: SpaceId,
    root: &Path,
    binding: Option<&CloudSpaceBinding>,
    device_id: &str,
) -> Result<(), String> {
    let provider = create_cloud_provider(storage).await?;
    process_due_cloud_tasks(storage, provider.as_ref(), space_id, root).await?;
    if let Some(binding) = binding {
        sync_manifest_to_cloud(
            storage,
            provider.as_ref(),
            space_id,
            root,
            binding,
            device_id,
        )
        .await?;
    }
    Ok(())
}

async fn create_cloud_provider(storage: &StorageEngine) -> Result<Box<dyn CloudProvider>, String> {
    if use_fake_cloud_provider() {
        return Ok(Box::new(FakeCloudProvider::new()));
    }
    let config = if let Some(config) = storage
        .get_cloud_api_config(BAIDU_PROVIDER)
        .await
        .map_err(|e| format!("Failed to load Baidu API config: {e}"))?
    {
        BaiduOAuthConfig {
            device_id: config.device_id,
            client_id: config.client_id,
            client_secret: config.client_secret,
            redirect_uri: config.redirect_uri,
            scopes: config.scopes,
        }
    } else {
        BaiduOAuthConfig::from_env().map_err(|e| e.to_string())?
    };
    let account = storage
        .get_cloud_account(BAIDU_PROVIDER)
        .await
        .map_err(|e| format!("Failed to load Baidu Netdisk account: {e}"))?
        .ok_or_else(|| {
            "Baidu Netdisk account is not connected; cannot process cloud sync tasks".to_string()
        })?;
    let provider = BaiduNetdiskProvider::from_cloud_account(&account, &config.client_id)
        .map_err(|e| format!("Failed to create Baidu Netdisk provider: {e}"))?;
    Ok(Box::new(provider))
}

fn use_fake_cloud_provider() -> bool {
    std::env::var("SYNCFLOW_CLOUD_PROVIDER")
        .ok()
        .map(|value| value.trim().eq_ignore_ascii_case("fake"))
        .unwrap_or(false)
}

async fn scan_remote_cloud_changes_for_provider(
    storage: &StorageEngine,
    space_id: SpaceId,
    root: &Path,
    binding: &CloudSpaceBinding,
) -> Result<(), String> {
    let provider = create_cloud_provider(storage).await?;
    scan_remote_cloud_changes(storage, provider.as_ref(), space_id, root, binding).await
}

async fn scan_remote_cloud_changes(
    storage: &StorageEngine,
    provider: &dyn CloudProvider,
    space_id: SpaceId,
    root: &Path,
    binding: &CloudSpaceBinding,
) -> Result<(), String> {
    let entries = list_cloud_tree(provider, &binding.remote_root_path).await?;
    let space_root_name = cloud_remote_root_name(&binding.remote_root_path);
    let remote_paths: std::collections::HashSet<String> = entries
        .iter()
        .filter(|entry| !entry.is_directory)
        .filter_map(|entry| remote_entry_relative_path(&binding.remote_root_path, entry))
        .filter(|relative_path| {
            !is_ignored_cloud_sync_relative_path(relative_path, space_root_name.as_deref())
        })
        .collect();
    for entry in entries.into_iter().filter(|entry| !entry.is_directory) {
        let Some(relative_path) = remote_entry_relative_path(&binding.remote_root_path, &entry)
        else {
            continue;
        };
        if is_ignored_cloud_sync_relative_path(&relative_path, space_root_name.as_deref()) {
            continue;
        }
        let existing_remote = storage
            .get_remote_file_metadata(&space_id, &binding.provider, &relative_path)
            .await
            .map_err(|e| format!("Failed to load remote metadata: {e}"))?;
        let local_path = safe_local_task_path(root, &relative_path)?;
        if !remote_changed_since_cloud_sync(existing_remote.as_ref(), &entry) {
            let local_meta = storage
                .get_file_meta(&space_id, &relative_path)
                .await
                .map_err(|e| format!("Failed to load local metadata: {e}"))?;
            if local_path.exists() || local_meta.is_some() {
                continue;
            }
        }
        if local_changed_since_baseline(storage, space_id, &relative_path, &local_path).await? {
            save_cloud_conflict(
                storage,
                provider,
                root,
                space_id,
                &relative_path,
                existing_remote.as_ref(),
                &entry,
            )
            .await?;
            continue;
        }
        let downloaded = match tokio::time::timeout(
            std::time::Duration::from_secs(CLOUD_SCAN_DOWNLOAD_TIMEOUT_SECONDS),
            download_cloud_file_if_changed(provider, &entry, root, &local_path),
        )
        .await
        {
            Ok(Ok(downloaded)) => downloaded,
            Ok(Err(error)) => {
                tracing::warn!(
                    "failed to download cloud file {}: {error}",
                    entry.remote_path
                );
                continue;
            }
            Err(_) => {
                tracing::warn!(
                    "timed out downloading cloud file {} after {}s",
                    entry.remote_path,
                    CLOUD_SCAN_DOWNLOAD_TIMEOUT_SECONDS
                );
                continue;
            }
        };
        if downloaded {
            index_downloaded_cloud_file(storage, space_id, &relative_path, &local_path).await?;
            remove_cloud_conflicts_for_path(storage, space_id, &relative_path).await?;
        }
        let task = CloudSyncTask {
            id: 0,
            space_id,
            provider: binding.provider.clone(),
            task_kind: "download".to_string(),
            local_relative_path: relative_path.clone(),
            remote_path: entry.remote_path.clone(),
            expected_remote_revision: entry.remote_revision.clone(),
            payload_json: None,
            attempts: 0,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            next_attempt_at: None,
        };
        storage
            .save_remote_file_metadata(&remote_metadata_from_entry(
                &task,
                &entry,
                Some(Utc::now()),
                false,
                read_local_cloud_baseline(&local_path).await?,
            ))
            .await
            .map_err(|e| format!("Failed to save cloud scan metadata: {e}"))?;
        write_local_sync_manifest(storage, space_id, root, Some(binding)).await?;
    }
    apply_remote_deletions(storage, space_id, root, binding, &remote_paths).await?;
    Ok(())
}

async fn list_cloud_tree(
    provider: &dyn CloudProvider,
    remote_root_path: &str,
) -> Result<Vec<CloudRemoteEntry>, String> {
    let mut all_entries = Vec::new();
    let mut pending_directories = vec![remote_root_path.to_string()];
    let mut scanned_directories = std::collections::HashSet::new();
    let mut seen_entries = std::collections::HashSet::new();
    while let Some(remote_path) = pending_directories.pop() {
        if !scanned_directories.insert(remote_path.clone()) {
            continue;
        }
        let entries = provider
            .list_directory(&remote_path)
            .await
            .map_err(|e| format!("Failed to scan cloud directory: {e}"))?;
        for entry in entries {
            if !seen_entries.insert(entry.remote_path.clone()) {
                continue;
            }
            if entry.is_directory {
                pending_directories.push(entry.remote_path.clone());
            }
            all_entries.push(entry);
        }
    }
    Ok(all_entries)
}

fn remote_entry_relative_path(remote_root_path: &str, entry: &CloudRemoteEntry) -> Option<String> {
    let root = remote_root_path.trim_end_matches('/');
    if entry.remote_path != root && !entry.remote_path.starts_with(&format!("{root}/")) {
        return None;
    }
    let path = entry.remote_path[root.len()..].trim_start_matches('/');
    if path.is_empty() || path.starts_with("../") || path.contains("/../") {
        None
    } else {
        Some(path.replace('\\', "/"))
    }
}

fn remote_changed_since_cloud_sync(
    existing: Option<&RemoteFileMetadata>,
    entry: &CloudRemoteEntry,
) -> bool {
    let Some(existing) = existing else {
        return true;
    };
    if existing.tombstone {
        return true;
    }
    if existing.last_remote_file_id.is_none()
        && existing.last_remote_md5.is_none()
        && existing.last_remote_size.is_none()
        && existing.last_remote_server_mtime.is_none()
        && existing.last_remote_revision.is_none()
    {
        return existing.remote_file_id != entry.remote_file_id
            || existing.md5 != entry.md5
            || existing.size != entry.size
            || existing.server_mtime != entry.server_mtime
            || existing.remote_revision != entry.remote_revision;
    }

    existing.last_remote_file_id != entry.remote_file_id
        || existing.last_remote_md5 != entry.md5
        || existing.last_remote_size != Some(entry.size)
        || existing.last_remote_server_mtime != entry.server_mtime
        || existing.last_remote_revision != entry.remote_revision
}

async fn local_changed_since_baseline(
    storage: &StorageEngine,
    space_id: SpaceId,
    relative_path: &str,
    local_path: &Path,
) -> Result<bool, String> {
    let local_meta = storage
        .get_file_meta(&space_id, relative_path)
        .await
        .map_err(|e| format!("Failed to load local metadata: {e}"))?;
    if let Some(remote_meta) = storage
        .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, relative_path)
        .await
        .map_err(|e| format!("Failed to load cloud sync baseline: {e}"))?
    {
        if !local_path.exists() && local_meta.is_none() {
            return Ok(false);
        }
        return local_file_changed_since_cloud_sync(Some(&remote_meta), local_path).await;
    }

    let Some(local_meta) = local_meta else {
        return Ok(local_path.exists());
    };
    if !local_path.exists() {
        return Ok(true);
    }
    let content = tokio::fs::read(local_path)
        .await
        .map_err(|e| format!("Failed to read local file: {e}"))?;
    Ok(hash_data(&content) != local_meta.hash)
}

async fn local_file_changed_since_cloud_sync(
    existing: Option<&RemoteFileMetadata>,
    local_path: &Path,
) -> Result<bool, String> {
    let Some(existing) = existing else {
        return Ok(local_path.exists());
    };
    if existing.tombstone {
        return Ok(local_path.exists());
    }
    if !local_path.exists() {
        return Ok(true);
    }

    let metadata = tokio::fs::metadata(local_path)
        .await
        .map_err(|e| format!("Failed to read local file metadata: {e}"))?;
    if !metadata.is_file() {
        return Ok(false);
    }
    let size = metadata.len();
    if existing.last_local_size != Some(size) {
        return Ok(true);
    }

    let content = tokio::fs::read(local_path)
        .await
        .map_err(|e| format!("Failed to read local file: {e}"))?;
    Ok(existing.last_local_hash.as_deref() != Some(hash_data(&content).as_str()))
}

async fn download_cloud_file_if_changed(
    provider: &dyn CloudProvider,
    entry: &CloudRemoteEntry,
    root: &Path,
    local_path: &Path,
) -> Result<bool, String> {
    let temp_dir = root.join(SYNCFLOW_META_DIR).join("downloads");
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| format!("Failed to create local cloud download temp directory: {e}"))?;
    let temp_path = temp_dir.join(format!("{}.tmp", Uuid::new_v4()));
    provider
        .download_file(&entry.remote_path, &temp_path)
        .await
        .map_err(|e| format!("Failed to download cloud file: {e}"))?;

    let downloaded = tokio::fs::read(&temp_path)
        .await
        .map_err(|e| format!("Failed to read downloaded cloud file: {e}"))?;
    if local_path.exists() {
        let local = tokio::fs::read(local_path)
            .await
            .map_err(|e| format!("Failed to read local file before cloud download: {e}"))?;
        if hash_data(&local) == hash_data(&downloaded) {
            tokio::fs::remove_file(&temp_path).await.ok();
            return Ok(false);
        }
    }

    if let Some(parent) = local_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create local cloud download directory: {e}"))?;
    }
    tokio::fs::rename(&temp_path, local_path)
        .await
        .map_err(|e| format!("Failed to replace local file with cloud download: {e}"))?;
    Ok(true)
}

async fn index_downloaded_cloud_file(
    storage: &StorageEngine,
    space_id: SpaceId,
    relative_path: &str,
    local_path: &Path,
) -> Result<(), String> {
    let content = tokio::fs::read(local_path)
        .await
        .map_err(|e| format!("Failed to read downloaded cloud file: {e}"))?;
    let now = Utc::now();
    let meta = FileMetadata {
        space_id,
        relative_path: relative_path.to_string(),
        hash: hash_data(&content),
        size: content.len() as u64,
        modified_at: now,
        version_vector: VersionVector::new("baidu_cloud")
            .to_json()
            .map_err(|e| e.to_string())?,
        created_at: now,
    };
    storage
        .save_file_meta(&meta)
        .await
        .map_err(|e| format!("Failed to save downloaded cloud file metadata: {e}"))
}

async fn save_cloud_conflict(
    storage: &StorageEngine,
    provider: &dyn CloudProvider,
    root: &Path,
    space_id: SpaceId,
    relative_path: &str,
    existing: Option<&RemoteFileMetadata>,
    entry: &CloudRemoteEntry,
) -> Result<(), String> {
    let conflict = SyncConflict {
        id: 0,
        space_id,
        relative_path: relative_path.to_string(),
        local_version: existing
            .and_then(|metadata| metadata.remote_revision.clone())
            .unwrap_or_else(|| "local_changed".to_string()),
        remote_version: entry
            .remote_revision
            .clone()
            .unwrap_or_else(|| "remote_changed".to_string()),
        remote_device_id: BAIDU_PROVIDER.to_string(),
        detected_at: Utc::now(),
    };
    storage
        .save_conflict(&conflict)
        .await
        .map_err(|e| format!("Failed to save cloud conflict: {e}"))?;
    let Some(saved) = storage
        .find_matching_conflict(&conflict)
        .await
        .map_err(|e| format!("Failed to load saved cloud conflict: {e}"))?
    else {
        return Ok(());
    };
    save_cloud_conflict_snapshot(storage, provider, root, &saved, entry).await
}

async fn save_cloud_conflict_snapshot(
    storage: &StorageEngine,
    provider: &dyn CloudProvider,
    root: &Path,
    conflict: &SyncConflict,
    entry: &CloudRemoteEntry,
) -> Result<(), String> {
    if !is_text_relative_path(&conflict.relative_path) {
        return Ok(());
    }
    let temp_dir = root.join(SYNCFLOW_META_DIR).join("conflicts");
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| format!("Failed to create conflict temp directory: {e}"))?;
    let temp_path = temp_dir.join(format!("{}.tmp", Uuid::new_v4()));
    provider
        .download_file(&entry.remote_path, &temp_path)
        .await
        .map_err(|e| format!("Failed to download cloud conflict snapshot: {e}"))?;
    let bytes = tokio::fs::read(&temp_path)
        .await
        .map_err(|e| format!("Failed to read cloud conflict snapshot: {e}"))?;
    tokio::fs::remove_file(&temp_path).await.ok();
    let max_bytes = 100_000usize;
    let content_truncated = bytes.len() > max_bytes;
    let content_bytes = if content_truncated {
        &bytes[..max_bytes]
    } else {
        &bytes[..]
    };
    let snapshot = ConflictSnapshot {
        id: 0,
        conflict_id: conflict.id,
        space_id: conflict.space_id,
        relative_path: conflict.relative_path.clone(),
        snapshot_kind: "remote_text".to_string(),
        content_text: Some(String::from_utf8_lossy(content_bytes).to_string()),
        content_truncated,
        content_size: bytes.len() as u64,
        created_at: Utc::now(),
    };
    storage
        .save_conflict_snapshot(&snapshot)
        .await
        .map_err(|e| format!("Failed to save cloud conflict snapshot: {e}"))
}

async fn remove_cloud_conflicts_for_path(
    storage: &StorageEngine,
    space_id: SpaceId,
    relative_path: &str,
) -> Result<(), String> {
    let conflicts = storage
        .get_conflicts_for_space(&space_id)
        .await
        .map_err(|e| format!("Failed to load cloud conflicts: {e}"))?;
    for conflict in conflicts.into_iter().filter(|conflict| {
        conflict.remote_device_id == BAIDU_PROVIDER && conflict.relative_path == relative_path
    }) {
        storage
            .remove_conflict(conflict.id)
            .await
            .map_err(|e| format!("Failed to remove resolved cloud conflict: {e}"))?;
    }
    Ok(())
}

async fn cleanup_redundant_cloud_conflicts(
    storage: &StorageEngine,
    space_id: SpaceId,
) -> Result<(), String> {
    let conflicts = storage
        .get_conflicts_for_space(&space_id)
        .await
        .map_err(|e| format!("Failed to load cloud conflicts: {e}"))?;
    for conflict in conflicts.into_iter().filter(|conflict| {
        conflict.remote_device_id == BAIDU_PROVIDER
            && conflict.local_version == conflict.remote_version
    }) {
        storage
            .remove_conflict(conflict.id)
            .await
            .map_err(|e| format!("Failed to remove redundant cloud conflict: {e}"))?;
    }
    Ok(())
}

async fn apply_remote_deletions(
    storage: &StorageEngine,
    space_id: SpaceId,
    _root: &Path,
    binding: &CloudSpaceBinding,
    remote_paths: &std::collections::HashSet<String>,
) -> Result<(), String> {
    let space_root_name = cloud_remote_root_name(&binding.remote_root_path);
    let known = storage
        .list_remote_file_metadata(&space_id, &binding.provider)
        .await
        .map_err(|e| format!("Failed to load known cloud metadata: {e}"))?;
    let existing_remote_deletions: std::collections::HashSet<String> = storage
        .get_conflicts_for_space(&space_id)
        .await
        .map_err(|e| format!("Failed to load remote deletion notices: {e}"))?
        .into_iter()
        .filter(|conflict| {
            conflict.remote_device_id == CLOUD_REMOTE_DELETED_DEVICE_ID
                && conflict.remote_version == "remote_deleted"
        })
        .map(|conflict| conflict.relative_path)
        .collect();
    for metadata in known.into_iter().filter(|metadata| !metadata.tombstone) {
        if metadata.is_directory {
            continue;
        }
        if is_ignored_cloud_sync_relative_path(
            &metadata.local_relative_path,
            space_root_name.as_deref(),
        ) {
            continue;
        }
        if remote_paths.contains(&metadata.local_relative_path) {
            continue;
        }
        if existing_remote_deletions.contains(&metadata.local_relative_path) {
            continue;
        }
        tracing::warn!(
            "cloud remote deletion detected for {}, local deletion is disabled",
            metadata.local_relative_path
        );
        let conflict = SyncConflict {
            id: 0,
            space_id,
            relative_path: metadata.local_relative_path.clone(),
            local_version: metadata
                .last_local_hash
                .clone()
                .or_else(|| metadata.last_remote_revision.clone())
                .unwrap_or_else(|| "local_kept".to_string()),
            remote_version: "remote_deleted".to_string(),
            remote_device_id: CLOUD_REMOTE_DELETED_DEVICE_ID.to_string(),
            detected_at: Utc::now(),
        };
        storage
            .save_conflict(&conflict)
            .await
            .map_err(|e| format!("Failed to save remote deletion notice: {e}"))?;
    }
    Ok(())
}

async fn process_cloud_task(
    storage: &StorageEngine,
    provider: &dyn CloudProvider,
    root: &Path,
    task: &CloudSyncTask,
) -> Result<(), String> {
    match task.task_kind.as_str() {
        "mkdir" => {
            if storage
                .get_remote_file_metadata(&task.space_id, &task.provider, &task.local_relative_path)
                .await
                .map_err(|e| format!("Failed to load remote directory metadata: {e}"))?
                .as_ref()
                .map(|metadata| metadata.is_directory && !metadata.tombstone)
                .unwrap_or(false)
            {
                storage
                    .remove_cloud_sync_task(task.id)
                    .await
                    .map_err(|e| format!("Failed to remove completed cloud directory task: {e}"))?;
                return Ok(());
            }
            let entry = match provider.create_directory(&task.remote_path).await {
                Ok(entry) => entry,
                Err(error)
                    if error.to_string().contains("31061")
                        || error.to_string().contains("file already exists") =>
                {
                    provider
                        .get_metadata(&task.remote_path)
                        .await
                        .map_err(|e| format!("Failed to confirm existing cloud directory: {e}"))?
                        .ok_or_else(|| {
                            format!(
                                "Cloud directory already exists but metadata was not found: {}",
                                task.remote_path
                            )
                        })?
                }
                Err(error) => return Err(format!("Failed to create cloud directory: {error}")),
            };
            storage
                .save_remote_file_metadata(&remote_metadata_from_entry(
                    task,
                    &entry,
                    Some(Utc::now()),
                    false,
                    None,
                ))
                .await
                .map_err(|e| format!("Failed to save remote directory metadata: {e}"))?;
            write_local_sync_manifest(storage, task.space_id, root, None).await?;
        }
        "upload" => {
            let local_path = safe_local_task_path(root, &task.local_relative_path)?;
            let local_baseline = read_local_cloud_baseline(&local_path).await?;
            if cloud_upload_task_already_synced(storage, task, local_baseline.as_ref()).await? {
                storage
                    .remove_cloud_sync_task(task.id)
                    .await
                    .map_err(|e| format!("Failed to remove already synced cloud task: {e}"))?;
                return Ok(());
            }
            let upload_result = provider
                .upload_file(
                    &local_path,
                    &task.remote_path,
                    task.expected_remote_revision.as_deref(),
                )
                .await;
            let entry = match upload_result {
                Ok(result) => result.entry,
                Err(error)
                    if error.to_string().contains("31061")
                        || error.to_string().contains("file already exists") =>
                {
                    provider
                        .get_metadata(&task.remote_path)
                        .await
                        .map_err(|e| format!("Failed to confirm existing cloud file: {e}"))?
                        .ok_or_else(|| {
                            format!(
                                "Cloud file already exists but metadata was not found: {}",
                                task.remote_path
                            )
                        })?
                }
                Err(error) => return Err(format!("Failed to upload to cloud: {error}")),
            };
            storage
                .save_remote_file_metadata(&remote_metadata_from_entry(
                    task,
                    &entry,
                    Some(Utc::now()),
                    false,
                    local_baseline,
                ))
                .await
                .map_err(|e| format!("Failed to save remote metadata: {e}"))?;
            write_local_sync_manifest(storage, task.space_id, root, None).await?;
        }
        "delete" => {
            provider
                .delete_path(&task.remote_path, task.expected_remote_revision.as_deref())
                .await
                .map_err(|e| format!("Failed to delete cloud file: {e}"))?;
            let now = Utc::now();
            let metadata = syncflow_core::storage::RemoteFileMetadata {
                space_id: task.space_id,
                provider: task.provider.clone(),
                remote_path: task.remote_path.clone(),
                local_relative_path: task.local_relative_path.clone(),
                remote_file_id: None,
                is_directory: false,
                size: 0,
                md5: None,
                server_mtime: None,
                remote_revision: None,
                last_remote_file_id: None,
                last_remote_md5: None,
                last_remote_size: None,
                last_remote_server_mtime: None,
                last_remote_revision: None,
                last_seen_at: now,
                last_synced_at: Some(now),
                last_local_hash: None,
                last_local_modified_at: None,
                last_local_size: None,
                tombstone: true,
            };
            storage
                .save_remote_file_metadata(&metadata)
                .await
                .map_err(|e| format!("Failed to save remote deletion marker: {e}"))?;
            write_local_sync_manifest(storage, task.space_id, root, None).await?;
        }
        other => return Err(format!("Unknown cloud sync task type: {other}")),
    }
    storage
        .remove_cloud_sync_task(task.id)
        .await
        .map_err(|e| format!("Failed to remove completed cloud sync task: {e}"))?;
    Ok(())
}

async fn cloud_upload_task_already_synced(
    storage: &StorageEngine,
    task: &CloudSyncTask,
    local_baseline: Option<&LocalCloudBaseline>,
) -> Result<bool, String> {
    let Some(local_baseline) = local_baseline else {
        return Ok(false);
    };
    let Some(existing) = storage
        .get_remote_file_metadata(&task.space_id, &task.provider, &task.local_relative_path)
        .await
        .map_err(|e| format!("Failed to load cloud upload baseline: {e}"))?
    else {
        return Ok(false);
    };
    if existing.tombstone {
        return Ok(false);
    }
    Ok(
        existing.last_local_hash.as_deref() == Some(local_baseline.hash.as_str())
            && existing.last_local_size == Some(local_baseline.size),
    )
}

fn remote_metadata_from_entry(
    task: &CloudSyncTask,
    entry: &syncflow_core::cloud::CloudRemoteEntry,
    last_synced_at: Option<chrono::DateTime<Utc>>,
    tombstone: bool,
    local_baseline: Option<LocalCloudBaseline>,
) -> syncflow_core::storage::RemoteFileMetadata {
    syncflow_core::storage::RemoteFileMetadata {
        space_id: task.space_id,
        provider: task.provider.clone(),
        remote_path: entry.remote_path.clone(),
        local_relative_path: task.local_relative_path.clone(),
        remote_file_id: entry.remote_file_id.clone(),
        is_directory: entry.is_directory,
        size: entry.size,
        md5: entry.md5.clone(),
        server_mtime: entry.server_mtime,
        remote_revision: entry.remote_revision.clone(),
        last_remote_file_id: entry.remote_file_id.clone(),
        last_remote_md5: entry.md5.clone(),
        last_remote_size: Some(entry.size),
        last_remote_server_mtime: entry.server_mtime,
        last_remote_revision: entry.remote_revision.clone(),
        last_local_hash: local_baseline
            .as_ref()
            .map(|baseline| baseline.hash.clone()),
        last_local_modified_at: local_baseline
            .as_ref()
            .and_then(|baseline| baseline.modified_at),
        last_local_size: local_baseline.as_ref().map(|baseline| baseline.size),
        last_seen_at: Utc::now(),
        last_synced_at,
        tombstone,
    }
}

fn cloud_metadata_task(
    binding: &CloudSpaceBinding,
    space_id: SpaceId,
    relative_path: &str,
    remote_path: &str,
) -> CloudSyncTask {
    let now = Utc::now();
    CloudSyncTask {
        id: 0,
        space_id,
        provider: binding.provider.clone(),
        task_kind: "upload".to_string(),
        local_relative_path: relative_path.to_string(),
        remote_path: remote_path.to_string(),
        expected_remote_revision: None,
        payload_json: None,
        attempts: 0,
        last_error: None,
        created_at: now,
        updated_at: now,
        next_attempt_at: None,
    }
}

struct LocalCloudBaseline {
    hash: String,
    modified_at: Option<chrono::DateTime<Utc>>,
    size: u64,
}

async fn read_local_cloud_baseline(
    local_path: &Path,
) -> Result<Option<LocalCloudBaseline>, String> {
    if !local_path.exists() {
        return Ok(None);
    }
    let metadata = tokio::fs::metadata(local_path)
        .await
        .map_err(|e| format!("Failed to read local file metadata: {e}"))?;
    if !metadata.is_file() {
        return Ok(None);
    }
    let content = tokio::fs::read(local_path)
        .await
        .map_err(|e| format!("Failed to read local file: {e}"))?;
    Ok(Some(LocalCloudBaseline {
        hash: hash_data(&content),
        modified_at: metadata.modified().ok().map(chrono::DateTime::<Utc>::from),
        size: metadata.len(),
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncManifest {
    version: u32,
    #[serde(default)]
    manifest_id: Option<String>,
    #[serde(default)]
    sequence: u64,
    #[serde(default)]
    base_remote_revision: Option<String>,
    #[serde(default)]
    updated_by_device_id: Option<String>,
    space_id: String,
    provider: Option<String>,
    remote_root_path: Option<String>,
    updated_at: DateTime<Utc>,
    entries: Vec<SyncManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncManifestEntry {
    relative_path: String,
    is_directory: bool,
    local_hash: Option<String>,
    local_modified_at: Option<DateTime<Utc>>,
    local_size: Option<u64>,
    remote_path: String,
    remote_file_id: Option<String>,
    remote_md5: Option<String>,
    remote_size: Option<u64>,
    remote_server_mtime: Option<DateTime<Utc>>,
    remote_revision: Option<String>,
    last_synced_at: Option<DateTime<Utc>>,
    tombstone: bool,
}

async fn ensure_local_manifest_dir(root: &Path) -> Result<(), String> {
    tokio::fs::create_dir_all(root.join(SYNCFLOW_META_DIR))
        .await
        .map_err(|e| format!("Failed to create local SyncFlow metadata directory: {e}"))
}

async fn write_local_sync_manifest(
    storage: &StorageEngine,
    space_id: SpaceId,
    root: &Path,
    binding: Option<&CloudSpaceBinding>,
) -> Result<(), String> {
    ensure_local_manifest_dir(root).await?;
    let existing =
        read_manifest_file(&root.join(SYNCFLOW_META_DIR).join(SYNCFLOW_MANIFEST_FILE)).await?;
    let manifest = build_sync_manifest(storage, space_id, binding, existing.as_ref(), None).await?;
    write_manifest_file(root, &manifest).await
}

async fn build_sync_manifest(
    storage: &StorageEngine,
    space_id: SpaceId,
    binding: Option<&CloudSpaceBinding>,
    previous: Option<&SyncManifest>,
    current_remote_revision: Option<String>,
) -> Result<SyncManifest, String> {
    let records = storage
        .list_remote_file_metadata(&space_id, BAIDU_PROVIDER)
        .await
        .map_err(|e| format!("Failed to load cloud metadata for local manifest: {e}"))?;
    let space_root_name = binding
        .and_then(|binding| cloud_remote_root_name(&binding.remote_root_path))
        .or_else(|| {
            previous
                .and_then(|manifest| manifest.remote_root_path.as_deref())
                .and_then(cloud_remote_root_name)
        });
    Ok(SyncManifest {
        version: SYNCFLOW_MANIFEST_VERSION,
        manifest_id: previous
            .and_then(|manifest| manifest.manifest_id.clone())
            .or_else(|| Some(Uuid::new_v4().to_string())),
        sequence: previous.map(|manifest| manifest.sequence + 1).unwrap_or(1),
        base_remote_revision: current_remote_revision
            .or_else(|| previous.and_then(|manifest| manifest.base_remote_revision.clone())),
        updated_by_device_id: None,
        space_id: space_id.to_string(),
        provider: binding
            .map(|binding| binding.provider.clone())
            .or_else(|| Some(BAIDU_PROVIDER.to_string())),
        remote_root_path: binding.map(|binding| binding.remote_root_path.clone()),
        updated_at: Utc::now(),
        entries: records
            .into_iter()
            .filter(|record| {
                !is_ignored_cloud_sync_relative_path(
                    &record.local_relative_path,
                    space_root_name.as_deref(),
                )
            })
            .map(|record| SyncManifestEntry {
                relative_path: record.local_relative_path,
                is_directory: record.is_directory,
                local_hash: record.last_local_hash,
                local_modified_at: record.last_local_modified_at,
                local_size: record.last_local_size,
                remote_path: record.remote_path,
                remote_file_id: record.last_remote_file_id.or(record.remote_file_id),
                remote_md5: record.last_remote_md5.or(record.md5),
                remote_size: record.last_remote_size.or(Some(record.size)),
                remote_server_mtime: record.last_remote_server_mtime.or(record.server_mtime),
                remote_revision: record.last_remote_revision.or(record.remote_revision),
                last_synced_at: record.last_synced_at,
                tombstone: record.tombstone,
            })
            .collect(),
    })
}

async fn write_manifest_file(root: &Path, manifest: &SyncManifest) -> Result<(), String> {
    ensure_local_manifest_dir(root).await?;
    let payload = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| format!("Failed to encode local SyncFlow manifest: {e}"))?;
    tokio::fs::write(
        root.join(SYNCFLOW_META_DIR).join(SYNCFLOW_MANIFEST_FILE),
        payload,
    )
    .await
    .map_err(|e| format!("Failed to write local SyncFlow manifest: {e}"))
}

async fn read_manifest_file(path: &Path) -> Result<Option<SyncManifest>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| format!("Failed to read SyncFlow manifest: {e}"))?;
    serde_json::from_slice::<SyncManifest>(&bytes)
        .map(Some)
        .map_err(|e| format!("Failed to parse SyncFlow manifest: {e}"))
}

async fn import_local_sync_manifest(
    storage: &StorageEngine,
    space_id: SpaceId,
    root: &Path,
) -> Result<(), String> {
    let path = root.join(SYNCFLOW_META_DIR).join(SYNCFLOW_MANIFEST_FILE);
    if let Some(manifest) = read_manifest_file(&path).await? {
        import_sync_manifest(storage, space_id, &manifest).await?;
    }
    Ok(())
}

async fn import_cloud_sync_manifest(
    storage: &StorageEngine,
    provider: &dyn CloudProvider,
    space_id: SpaceId,
    root: &Path,
    binding: &CloudSpaceBinding,
) -> Result<(), String> {
    ensure_local_manifest_dir(root).await?;
    let remote_manifest_path = join_remote_path(
        &binding.remote_root_path,
        &format!("{SYNCFLOW_META_DIR}/{SYNCFLOW_MANIFEST_FILE}"),
    )?;
    let local_manifest_path = root
        .join(SYNCFLOW_META_DIR)
        .join("cloud-manifest-import.json");
    match provider
        .download_file(&remote_manifest_path, &local_manifest_path)
        .await
    {
        Ok(()) => {}
        Err(error)
            if error.to_string().contains("not found")
                || error.to_string().contains("FileNotFound")
                || error.to_string().contains("errno=-9") =>
        {
            return Ok(());
        }
        Err(error) => {
            return Err(format!(
                "Failed to download cloud SyncFlow manifest: {error}"
            ))
        }
    }
    if let Some(manifest) = read_manifest_file(&local_manifest_path).await? {
        import_sync_manifest(storage, space_id, &manifest).await?;
        let filtered_manifest =
            filtered_sync_manifest_entries(manifest, Some(&binding.remote_root_path));
        write_manifest_file(root, &filtered_manifest).await?;
    }
    tokio::fs::remove_file(local_manifest_path).await.ok();
    Ok(())
}

async fn import_sync_manifest(
    storage: &StorageEngine,
    space_id: SpaceId,
    manifest: &SyncManifest,
) -> Result<(), String> {
    if manifest.version != SYNCFLOW_MANIFEST_VERSION {
        return Err(format!(
            "Unsupported SyncFlow manifest version: {}",
            manifest.version
        ));
    }
    let provider = manifest
        .provider
        .clone()
        .unwrap_or_else(|| BAIDU_PROVIDER.to_string());
    let space_root_name = manifest
        .remote_root_path
        .as_deref()
        .and_then(cloud_remote_root_name);
    for entry in &manifest.entries {
        if is_ignored_cloud_sync_relative_path(&entry.relative_path, space_root_name.as_deref()) {
            continue;
        }
        let metadata = RemoteFileMetadata {
            space_id,
            provider: provider.clone(),
            remote_path: entry.remote_path.clone(),
            local_relative_path: entry.relative_path.clone(),
            remote_file_id: entry.remote_file_id.clone(),
            is_directory: entry.is_directory,
            size: entry.remote_size.unwrap_or(0),
            md5: entry.remote_md5.clone(),
            server_mtime: entry.remote_server_mtime,
            remote_revision: entry.remote_revision.clone(),
            last_remote_file_id: entry.remote_file_id.clone(),
            last_remote_md5: entry.remote_md5.clone(),
            last_remote_size: entry.remote_size,
            last_remote_server_mtime: entry.remote_server_mtime,
            last_remote_revision: entry.remote_revision.clone(),
            last_local_hash: entry.local_hash.clone(),
            last_local_modified_at: entry.local_modified_at,
            last_local_size: entry.local_size,
            last_seen_at: Utc::now(),
            last_synced_at: entry.last_synced_at,
            tombstone: entry.tombstone,
        };
        storage
            .save_remote_file_metadata(&metadata)
            .await
            .map_err(|e| format!("Failed to import SyncFlow manifest entry: {e}"))?;
    }
    Ok(())
}

async fn sync_manifest_to_cloud(
    storage: &StorageEngine,
    provider: &dyn CloudProvider,
    space_id: SpaceId,
    root: &Path,
    binding: &CloudSpaceBinding,
    device_id: &str,
) -> Result<(), String> {
    let local_manifest_path = root.join(SYNCFLOW_META_DIR).join(SYNCFLOW_MANIFEST_FILE);
    let remote_manifest_dir = join_remote_path(&binding.remote_root_path, SYNCFLOW_META_DIR)?;
    match provider.create_directory(&remote_manifest_dir).await {
        Ok(_) => {}
        Err(error)
            if error.to_string().contains("31061")
                || error.to_string().contains("file already exists")
                || error.to_string().contains("errno=-8") => {}
        Err(error) => {
            return Err(format!(
                "Failed to create cloud manifest directory: {error}"
            ))
        }
    }
    let remote_manifest_path = join_remote_path(
        &binding.remote_root_path,
        &format!("{SYNCFLOW_META_DIR}/{SYNCFLOW_MANIFEST_FILE}"),
    )?;
    let local_manifest = read_manifest_file(&local_manifest_path).await?;
    let remote_metadata = provider
        .get_metadata(&remote_manifest_path)
        .await
        .map_err(|e| format!("Failed to inspect cloud SyncFlow manifest: {e}"))?;
    if let (Some(local_manifest), Some(remote_metadata)) =
        (local_manifest.as_ref(), remote_metadata.as_ref())
    {
        let remote_revision = remote_metadata.remote_revision.clone();
        if local_manifest.base_remote_revision.is_some()
            && local_manifest.base_remote_revision != remote_revision
        {
            import_cloud_sync_manifest(storage, provider, space_id, root, binding).await?;
        }
    }
    let latest_local_manifest = read_manifest_file(&local_manifest_path).await?;
    let latest_remote_revision = provider
        .get_metadata(&remote_manifest_path)
        .await
        .map_err(|e| format!("Failed to inspect latest cloud SyncFlow manifest: {e}"))?
        .and_then(|entry| entry.remote_revision);
    let mut manifest = build_sync_manifest(
        storage,
        space_id,
        Some(binding),
        latest_local_manifest.as_ref(),
        latest_remote_revision,
    )
    .await?;
    manifest.updated_by_device_id = Some(device_id.to_string());
    write_manifest_file(root, &manifest).await?;
    provider
        .upload_file(&local_manifest_path, &remote_manifest_path, None)
        .await
        .map_err(|e| format!("Failed to upload cloud SyncFlow manifest: {e}"))?;
    if let Ok(Some(updated)) = provider.get_metadata(&remote_manifest_path).await {
        manifest.base_remote_revision = updated.remote_revision;
        write_manifest_file(root, &manifest).await?;
    }
    Ok(())
}

fn safe_local_task_path(root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let path = root.join(relative_path);
    if !path.starts_with(root) || relative_path.split('/').any(|part| part == "..") {
        return Err("Cloud sync task local path is unsafe".to_string());
    }
    Ok(path)
}

fn cloud_retry_delay_seconds(attempts: u32) -> i64 {
    match attempts {
        0 => 5,
        1 => 15,
        2 => 60,
        _ => 300,
    }
}

fn join_remote_path(remote_root_path: &str, relative_path: &str) -> Result<String, String> {
    let normalized_relative = relative_path.replace('\\', "/");
    if normalized_relative.starts_with('/')
        || normalized_relative
            .split('/')
            .any(|part| part == ".." || part.is_empty())
    {
        return Err("Cloud sync relative path is unsafe".to_string());
    }
    Ok(format!(
        "{}/{}",
        remote_root_path.trim_end_matches('/'),
        normalized_relative
    ))
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_files_inner(root, &mut files)?;
    Ok(files)
}

fn collect_directories(root: &Path) -> Result<Vec<String>, String> {
    let mut directories = Vec::new();
    collect_directories_inner(root, root, &mut directories)?;
    directories.sort();
    Ok(directories)
}

fn collect_directories_inner(
    root: &Path,
    path: &Path,
    directories: &mut Vec<String>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(path).map_err(|e| format!("Failed to read directory: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();
        let relative_path = strip_root_prefix(root, &path)?;
        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to read directory metadata: {e}"))?;
        if is_syncflow_metadata_path(&relative_path)
            || is_ignored_local_sync_relative_path(&relative_path)
        {
            continue;
        }
        if metadata.is_dir() {
            directories.push(relative_path);
            collect_directories_inner(root, &path, directories)?;
        }
    }
    Ok(())
}

fn collect_files_inner(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(path).map_err(|e| format!("Failed to read directory: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to read file metadata: {e}"))?;
        let file_name = path.file_name().and_then(|name| name.to_str());
        if file_name == Some(SYNCFLOW_META_DIR) || file_name == Some(".DS_Store") {
            continue;
        }
        if metadata.is_dir() {
            collect_files_inner(&path, files)?;
        } else if metadata.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn strip_root_prefix(root: &Path, child: &Path) -> Result<String, String> {
    let relative = child
        .strip_prefix(root)
        .map_err(|_| "File event is outside the sync space root".to_string())?;
    if relative
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("File event contains an unsafe path".to_string());
    }
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn is_syncflow_metadata_path(relative_path: &str) -> bool {
    relative_path == SYNCFLOW_META_DIR
        || relative_path.starts_with(&format!("{SYNCFLOW_META_DIR}/"))
}

fn is_ignored_local_sync_relative_path(relative_path: &str) -> bool {
    let normalized = relative_path.replace('\\', "/");
    normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .any(|part| part == ".DS_Store")
}

fn cloud_remote_root_name(remote_root_path: &str) -> Option<String> {
    remote_root_path
        .trim_end_matches('/')
        .rsplit('/')
        .find(|part| !part.is_empty())
        .map(ToOwned::to_owned)
}

fn is_ignored_cloud_sync_relative_path(relative_path: &str, space_root_name: Option<&str>) -> bool {
    let normalized = relative_path.replace('\\', "/");
    let parts = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return true;
    }
    if parts.iter().any(|part| *part == "..") {
        return true;
    }
    if parts.iter().any(|part| *part == SYNCFLOW_META_DIR) {
        return true;
    }
    if parts.last() == Some(&".DS_Store") {
        return true;
    }
    if let Some(space_root_name) = space_root_name {
        if parts.first() == Some(&space_root_name) {
            return true;
        }
    }
    false
}

fn filtered_sync_manifest_entries(
    mut manifest: SyncManifest,
    remote_root_path: Option<&str>,
) -> SyncManifest {
    let space_root_name = remote_root_path
        .and_then(cloud_remote_root_name)
        .or_else(|| manifest.remote_root_path.as_deref().and_then(cloud_remote_root_name));
    manifest.entries.retain(|entry| {
        !is_ignored_cloud_sync_relative_path(&entry.relative_path, space_root_name.as_deref())
    });
    manifest
}

fn is_text_relative_path(relative_path: &str) -> bool {
    let extension = Path::new(relative_path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    matches!(
        extension.as_deref(),
        Some("txt")
            | Some("md")
            | Some("json")
            | Some("xml")
            | Some("yml")
            | Some("yaml")
            | Some("toml")
            | Some("csv")
            | Some("html")
            | Some("htm")
            | Some("css")
            | Some("scss")
            | Some("less")
            | Some("js")
            | Some("ts")
            | Some("jsx")
            | Some("tsx")
            | Some("rs")
            | Some("py")
            | Some("go")
            | Some("java")
            | Some("c")
            | Some("cpp")
            | Some("h")
            | Some("hpp")
            | Some("rb")
            | Some("php")
            | Some("log")
            | Some("ini")
            | Some("cfg")
            | Some("conf")
            | Some("env")
            | Some("sh")
            | Some("bat")
            | Some("ps1")
    )
}

fn status_to_dto(runtime: &SpaceRuntime) -> SyncRuntimeStatusDto {
    SyncRuntimeStatusDto {
        space_id: runtime.space_id.to_string(),
        status: runtime.status.as_str().to_string(),
        file_count: runtime.file_count,
        pending_count: runtime.pending_count,
        conflict_count: runtime.conflict_count,
        cloud_conflict_count: runtime.cloud_conflict_count,
        connected_peer_count: runtime.connected_peer_count,
        discovered_peer_count: runtime.discovered_peer_count,
        cloud_provider: runtime.cloud_provider.clone(),
        cloud_remote_path: runtime.cloud_remote_path.clone(),
        last_cloud_scan_at: runtime.last_cloud_scan_at.map(|value| value.to_rfc3339()),
        last_indexed_at: runtime.last_indexed_at.map(|value| value.to_rfc3339()),
        last_transport_event: runtime.last_transport_event.clone(),
        last_transport_event_at: runtime
            .last_transport_event_at
            .map(|value| value.to_rfc3339()),
        last_error: runtime.last_error.clone(),
    }
}

fn refresh_runtime_liveness(runtime: &mut SpaceRuntime) {
    if !matches!(
        runtime.status,
        RuntimeStatus::Watching | RuntimeStatus::Syncing
    ) {
        return;
    }

    let watcher_stopped = runtime
        .watcher_task
        .as_ref()
        .map(|task| task.is_finished())
        .unwrap_or(true);
    let queue_stopped = runtime
        .queue_task
        .as_ref()
        .map(|task| task.is_finished())
        .unwrap_or(true);

    if !watcher_stopped && !queue_stopped {
        return;
    }

    runtime.status = RuntimeStatus::Error;
    runtime.last_error = Some(match (watcher_stopped, queue_stopped) {
        (true, true) => "后台同步任务已停止，请重新启动同步。".to_string(),
        (true, false) => "文件监听任务已停止，请重新启动同步。".to_string(),
        (false, true) => "同步队列任务已停止，请重新启动同步。".to_string(),
        (false, false) => unreachable!(),
    });
}

fn extract_sync_key(data: &[u8]) -> Option<String> {
    let null_pos = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    let meta_json = &data[..null_pos];
    let meta: serde_json::Value = serde_json::from_slice(meta_json).ok()?;
    meta.get("sync_key")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

fn is_control_message(data: &[u8], message_type: &str) -> bool {
    let null_pos = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    let meta_json = &data[..null_pos];
    serde_json::from_slice::<serde_json::Value>(meta_json)
        .ok()
        .and_then(|meta| {
            meta.get("type")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
        })
        .as_deref()
        == Some(message_type)
}

#[cfg(test)]
mod tests {
    use super::scan_remote_cloud_changes;
    use super::{
        enqueue_existing_files_for_cloud, join_remote_path, process_due_cloud_tasks,
        remote_entry_relative_path,
    };
    use chrono::Utc;
    use syncflow_core::cloud::{
        CloudProvider, CloudRemoteEntry, FakeCloudProvider, BAIDU_PROVIDER,
    };
    use syncflow_core::storage::{
        initialize_schema, CloudSpaceBinding, CloudSyncTask, StorageEngine,
    };
    use uuid::Uuid;

    #[test]
    fn joins_remote_path_safely() {
        assert_eq!(
            join_remote_path("/apps/SyncFlow/Notes", "docs/readme.md").unwrap(),
            "/apps/SyncFlow/Notes/docs/readme.md"
        );
        assert_eq!(
            join_remote_path("/apps/SyncFlow/Notes/", "docs\\readme.md").unwrap(),
            "/apps/SyncFlow/Notes/docs/readme.md"
        );
        assert!(join_remote_path("/apps/SyncFlow/Notes", "../secret.txt").is_err());
        assert!(join_remote_path("/apps/SyncFlow/Notes", "/secret.txt").is_err());
    }

    #[tokio::test]
    async fn process_due_cloud_tasks_uploads_and_clears_task() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = StorageEngine::new(pool);
        let provider = FakeCloudProvider::new();
        let space_id = Uuid::new_v4();
        let root = std::env::temp_dir().join(format!("syncflow-cloud-task-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(root.join("docs")).await.unwrap();
        tokio::fs::write(root.join("docs/readme.md"), b"cloud body")
            .await
            .unwrap();
        let now = Utc::now();
        let task = CloudSyncTask {
            id: 0,
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            task_kind: "upload".to_string(),
            local_relative_path: "docs/readme.md".to_string(),
            remote_path: "/apps/SyncFlow/Notes/docs/readme.md".to_string(),
            expected_remote_revision: None,
            payload_json: None,
            attempts: 0,
            last_error: None,
            created_at: now,
            updated_at: now,
            next_attempt_at: Some(now),
        };
        storage.enqueue_cloud_sync_task(&task).await.unwrap();

        process_due_cloud_tasks(&storage, &provider, space_id, &root)
            .await
            .unwrap();

        assert!(storage
            .get_due_cloud_sync_tasks(BAIDU_PROVIDER, Utc::now(), 10)
            .await
            .unwrap()
            .is_empty());
        assert!(provider
            .get_metadata("/apps/SyncFlow/Notes/docs/readme.md")
            .await
            .unwrap()
            .is_some());
        assert!(storage
            .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, "docs/readme.md")
            .await
            .unwrap()
            .is_some());

        tokio::fs::remove_dir_all(root).await.ok();
    }

    #[tokio::test]
    async fn process_due_cloud_tasks_creates_directory_and_clears_task() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = StorageEngine::new(pool);
        let provider = FakeCloudProvider::new();
        let space_id = Uuid::new_v4();
        let root = std::env::temp_dir().join(format!("syncflow-cloud-mkdir-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(root.join("empty-dir"))
            .await
            .unwrap();
        let now = Utc::now();
        let task = CloudSyncTask {
            id: 0,
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            task_kind: "mkdir".to_string(),
            local_relative_path: "empty-dir".to_string(),
            remote_path: "/apps/SyncFlow/Notes/empty-dir".to_string(),
            expected_remote_revision: None,
            payload_json: None,
            attempts: 0,
            last_error: None,
            created_at: now,
            updated_at: now,
            next_attempt_at: Some(now),
        };
        storage.enqueue_cloud_sync_task(&task).await.unwrap();

        process_due_cloud_tasks(&storage, &provider, space_id, &root)
            .await
            .unwrap();

        assert!(storage
            .get_due_cloud_sync_tasks(BAIDU_PROVIDER, Utc::now(), 10)
            .await
            .unwrap()
            .is_empty());
        assert!(
            provider
                .get_metadata("/apps/SyncFlow/Notes/empty-dir")
                .await
                .unwrap()
                .unwrap()
                .is_directory
        );
        assert!(
            storage
                .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, "empty-dir")
                .await
                .unwrap()
                .unwrap()
                .is_directory
        );

        tokio::fs::remove_dir_all(root).await.ok();
    }

    #[tokio::test]
    async fn enqueue_existing_files_for_cloud_uses_local_baseline_for_incremental_uploads() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = StorageEngine::new(pool);
        let space_id = Uuid::new_v4();
        let now = Utc::now();
        let binding = CloudSpaceBinding {
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            remote_root_path: "/apps/SyncFlow/Notes".to_string(),
            remote_root_id: None,
            sync_mode: "bidirectional".to_string(),
            plaintext: true,
            created_at: now,
            updated_at: now,
        };
        let root =
            std::env::temp_dir().join(format!("syncflow-cloud-incremental-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(root.join("docs")).await.unwrap();
        let file = root.join("docs/readme.md");
        tokio::fs::write(&file, b"v1").await.unwrap();
        let baseline = super::read_local_cloud_baseline(&file)
            .await
            .unwrap()
            .unwrap();
        let task = CloudSyncTask {
            id: 0,
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            task_kind: "upload".to_string(),
            local_relative_path: "docs/readme.md".to_string(),
            remote_path: "/apps/SyncFlow/Notes/docs/readme.md".to_string(),
            expected_remote_revision: None,
            payload_json: None,
            attempts: 0,
            last_error: None,
            created_at: now,
            updated_at: now,
            next_attempt_at: Some(now),
        };
        let entry = CloudRemoteEntry {
            remote_path: task.remote_path.clone(),
            remote_file_id: Some("fs-1".to_string()),
            is_directory: false,
            size: 2,
            md5: None,
            server_mtime: Some(now),
            remote_revision: Some("rev-1".to_string()),
        };
        storage
            .save_remote_file_metadata(&super::remote_metadata_from_entry(
                &task,
                &entry,
                Some(now),
                false,
                Some(baseline),
            ))
            .await
            .unwrap();

        enqueue_existing_files_for_cloud(
            &storage,
            &binding,
            space_id,
            &[("docs/readme.md".to_string(), file.clone())],
        )
        .await
        .unwrap();
        assert!(storage
            .get_due_cloud_sync_tasks(BAIDU_PROVIDER, Utc::now(), 10)
            .await
            .unwrap()
            .is_empty());

        tokio::fs::write(&file, b"v2").await.unwrap();
        enqueue_existing_files_for_cloud(
            &storage,
            &binding,
            space_id,
            &[("docs/readme.md".to_string(), file.clone())],
        )
        .await
        .unwrap();
        assert_eq!(
            storage
                .get_due_cloud_sync_tasks(BAIDU_PROVIDER, Utc::now(), 10)
                .await
                .unwrap()
                .len(),
            1
        );

        tokio::fs::remove_dir_all(root).await.ok();
    }

    #[tokio::test]
    async fn scan_remote_cloud_changes_downloads_new_file() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = StorageEngine::new(pool);
        let provider = FakeCloudProvider::new();
        let space_id = Uuid::new_v4();
        let now = Utc::now();
        let binding = CloudSpaceBinding {
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            remote_root_path: "/apps/SyncFlow/Notes".to_string(),
            remote_root_id: None,
            sync_mode: "bidirectional".to_string(),
            plaintext: true,
            created_at: now,
            updated_at: now,
        };
        let root = std::env::temp_dir().join(format!("syncflow-cloud-scan-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await.unwrap();
        provider
            .seed_file(
                "/apps/SyncFlow/Notes/docs/remote.md",
                b"remote body".to_vec(),
            )
            .unwrap();

        scan_remote_cloud_changes(&storage, &provider, space_id, &root, &binding)
            .await
            .unwrap();

        assert_eq!(
            tokio::fs::read(root.join("docs/remote.md")).await.unwrap(),
            b"remote body"
        );
        assert!(storage
            .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, "docs/remote.md")
            .await
            .unwrap()
            .is_some());

        tokio::fs::remove_dir_all(root).await.ok();
    }

    #[tokio::test]
    async fn scan_remote_cloud_changes_downloads_imported_manifest_file_missing_locally() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = StorageEngine::new(pool);
        let provider = FakeCloudProvider::new();
        let space_id = Uuid::new_v4();
        let now = Utc::now();
        let binding = CloudSpaceBinding {
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            remote_root_path: "/apps/SyncFlow/Notes".to_string(),
            remote_root_id: None,
            sync_mode: "bidirectional".to_string(),
            plaintext: true,
            created_at: now,
            updated_at: now,
        };
        let root = std::env::temp_dir().join(format!("syncflow-cloud-import-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await.unwrap();
        provider
            .seed_file(
                "/apps/SyncFlow/Notes/docs/remote.md",
                b"remote body".to_vec(),
            )
            .unwrap();
        let entry = provider
            .get_metadata("/apps/SyncFlow/Notes/docs/remote.md")
            .await
            .unwrap()
            .unwrap();
        let task = CloudSyncTask {
            id: 0,
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            task_kind: "download".to_string(),
            local_relative_path: "docs/remote.md".to_string(),
            remote_path: entry.remote_path.clone(),
            expected_remote_revision: None,
            payload_json: None,
            attempts: 0,
            last_error: None,
            created_at: now,
            updated_at: now,
            next_attempt_at: None,
        };
        storage
            .save_remote_file_metadata(&super::remote_metadata_from_entry(
                &task,
                &entry,
                Some(now),
                false,
                None,
            ))
            .await
            .unwrap();
        super::save_cloud_conflict(
            &storage,
            &provider,
            &root,
            space_id,
            "docs/remote.md",
            storage
                .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, "docs/remote.md")
                .await
                .unwrap()
                .as_ref(),
            &entry,
        )
        .await
        .unwrap();

        scan_remote_cloud_changes(&storage, &provider, space_id, &root, &binding)
            .await
            .unwrap();

        assert_eq!(
            tokio::fs::read(root.join("docs/remote.md")).await.unwrap(),
            b"remote body"
        );
        assert!(storage
            .get_file_meta(&space_id, "docs/remote.md")
            .await
            .unwrap()
            .is_some());
        assert!(storage
            .get_conflicts_for_space(&space_id)
            .await
            .unwrap()
            .is_empty());

        tokio::fs::remove_dir_all(root).await.ok();
    }

    #[tokio::test]
    async fn bidirectional_cloud_sync_uploads_local_and_downloads_remote() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = StorageEngine::new(pool);
        let provider = FakeCloudProvider::new();
        let space_id = Uuid::new_v4();
        let now = Utc::now();
        let binding = CloudSpaceBinding {
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            remote_root_path: "/apps/SyncFlow/Notes".to_string(),
            remote_root_id: None,
            sync_mode: "bidirectional".to_string(),
            plaintext: true,
            created_at: now,
            updated_at: now,
        };
        let root = std::env::temp_dir().join(format!("syncflow-bidirectional-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(root.join("local")).await.unwrap();
        tokio::fs::write(root.join("local/local.md"), b"local body")
            .await
            .unwrap();

        enqueue_existing_files_for_cloud(
            &storage,
            &binding,
            space_id,
            &[("local/local.md".to_string(), root.join("local/local.md"))],
        )
        .await
        .unwrap();
        process_due_cloud_tasks(&storage, &provider, space_id, &root)
            .await
            .unwrap();

        let uploaded_copy = root.join("uploaded-copy.md");
        provider
            .download_file("/apps/SyncFlow/Notes/local/local.md", &uploaded_copy)
            .await
            .unwrap();
        assert_eq!(tokio::fs::read(uploaded_copy).await.unwrap(), b"local body");
        assert!(storage
            .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, "local/local.md")
            .await
            .unwrap()
            .unwrap()
            .last_synced_at
            .is_some());

        provider
            .seed_file(
                "/apps/SyncFlow/Notes/remote/remote.md",
                b"remote body".to_vec(),
            )
            .unwrap();
        scan_remote_cloud_changes(&storage, &provider, space_id, &root, &binding)
            .await
            .unwrap();

        assert_eq!(
            tokio::fs::read(root.join("remote/remote.md"))
                .await
                .unwrap(),
            b"remote body"
        );
        assert!(storage
            .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, "remote/remote.md")
            .await
            .unwrap()
            .unwrap()
            .last_synced_at
            .is_some());
        assert!(storage
            .get_due_cloud_sync_tasks(BAIDU_PROVIDER, Utc::now(), 10)
            .await
            .unwrap()
            .is_empty());

        tokio::fs::remove_dir_all(root).await.ok();
    }

    #[tokio::test]
    async fn scan_remote_cloud_changes_applies_modify_and_delete() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = StorageEngine::new(pool);
        let provider = FakeCloudProvider::new();
        let space_id = Uuid::new_v4();
        let now = Utc::now();
        let binding = CloudSpaceBinding {
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            remote_root_path: "/apps/SyncFlow/Notes".to_string(),
            remote_root_id: None,
            sync_mode: "bidirectional".to_string(),
            plaintext: true,
            created_at: now,
            updated_at: now,
        };
        let root = std::env::temp_dir().join(format!("syncflow-cloud-diff-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await.unwrap();
        provider
            .seed_file("/apps/SyncFlow/Notes/remote.md", b"v1".to_vec())
            .unwrap();
        scan_remote_cloud_changes(&storage, &provider, space_id, &root, &binding)
            .await
            .unwrap();

        provider
            .seed_file("/apps/SyncFlow/Notes/remote.md", b"v2".to_vec())
            .unwrap();
        scan_remote_cloud_changes(&storage, &provider, space_id, &root, &binding)
            .await
            .unwrap();
        assert_eq!(
            tokio::fs::read(root.join("remote.md")).await.unwrap(),
            b"v2"
        );

        provider
            .delete_path("/apps/SyncFlow/Notes/remote.md", None)
            .await
            .unwrap();
        scan_remote_cloud_changes(&storage, &provider, space_id, &root, &binding)
            .await
            .unwrap();
        assert!(root.join("remote.md").exists());
        let metadata = storage
            .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, "remote.md")
            .await
            .unwrap()
            .unwrap();
        assert!(!metadata.tombstone);

        tokio::fs::remove_dir_all(root).await.ok();
    }

    #[tokio::test]
    async fn scan_remote_cloud_changes_does_not_rewrite_identical_local_file() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = StorageEngine::new(pool);
        let provider = FakeCloudProvider::new();
        let space_id = Uuid::new_v4();
        let now = Utc::now();
        let root = std::env::temp_dir().join(format!("syncflow-cloud-mtime-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await.unwrap();
        let local_path = root.join("remote.md");
        tokio::fs::write(&local_path, b"same body").await.unwrap();
        provider
            .seed_file("/apps/SyncFlow/Notes/remote.md", b"same body".to_vec())
            .unwrap();
        let baseline = super::read_local_cloud_baseline(&local_path).await.unwrap();
        let mut entry = provider
            .get_metadata("/apps/SyncFlow/Notes/remote.md")
            .await
            .unwrap()
            .unwrap();
        let task = CloudSyncTask {
            id: 0,
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            task_kind: "download".to_string(),
            local_relative_path: "remote.md".to_string(),
            remote_path: entry.remote_path.clone(),
            expected_remote_revision: None,
            payload_json: None,
            attempts: 0,
            last_error: None,
            created_at: now,
            updated_at: now,
            next_attempt_at: None,
        };
        storage
            .save_remote_file_metadata(&super::remote_metadata_from_entry(
                &task,
                &entry,
                Some(now),
                false,
                baseline,
            ))
            .await
            .unwrap();
        let before_modified = tokio::fs::metadata(&local_path)
            .await
            .unwrap()
            .modified()
            .unwrap();
        entry.remote_revision = Some("new-remote-revision-same-body".to_string());

        let downloaded =
            super::download_cloud_file_if_changed(&provider, &entry, &root, &local_path)
                .await
                .unwrap();

        assert!(!downloaded);
        assert_eq!(
            tokio::fs::metadata(&local_path)
                .await
                .unwrap()
                .modified()
                .unwrap(),
            before_modified
        );
        assert!(!super::local_file_changed_since_cloud_sync(
            storage
                .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, "remote.md")
                .await
                .unwrap()
                .as_ref(),
            &local_path
        )
        .await
        .unwrap());

        tokio::fs::remove_dir_all(root).await.ok();
    }

    #[tokio::test]
    async fn scan_remote_cloud_changes_keeps_nested_files_visible_for_deletion_check() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = StorageEngine::new(pool);
        let provider = FakeCloudProvider::new();
        let space_id = Uuid::new_v4();
        let now = Utc::now();
        let binding = CloudSpaceBinding {
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            remote_root_path: "/apps/SyncFlow/Notes".to_string(),
            remote_root_id: None,
            sync_mode: "bidirectional".to_string(),
            plaintext: true,
            created_at: now,
            updated_at: now,
        };
        let root = std::env::temp_dir().join(format!("syncflow-cloud-nested-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(root.join("docs")).await.unwrap();
        tokio::fs::write(root.join("docs/nested.md"), b"nested")
            .await
            .unwrap();
        provider
            .create_directory("/apps/SyncFlow/Notes/docs")
            .await
            .unwrap();
        provider
            .seed_file("/apps/SyncFlow/Notes/docs/nested.md", b"nested".to_vec())
            .unwrap();
        let baseline = super::read_local_cloud_baseline(&root.join("docs/nested.md"))
            .await
            .unwrap();
        let task = CloudSyncTask {
            id: 0,
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            task_kind: "upload".to_string(),
            local_relative_path: "docs/nested.md".to_string(),
            remote_path: "/apps/SyncFlow/Notes/docs/nested.md".to_string(),
            expected_remote_revision: None,
            payload_json: None,
            attempts: 0,
            last_error: None,
            created_at: now,
            updated_at: now,
            next_attempt_at: None,
        };
        let entry = provider
            .get_metadata("/apps/SyncFlow/Notes/docs/nested.md")
            .await
            .unwrap()
            .unwrap();
        storage
            .save_remote_file_metadata(&super::remote_metadata_from_entry(
                &task,
                &entry,
                Some(now),
                false,
                baseline,
            ))
            .await
            .unwrap();

        scan_remote_cloud_changes(&storage, &provider, space_id, &root, &binding)
            .await
            .unwrap();

        assert!(storage
            .get_conflicts_for_space(&space_id)
            .await
            .unwrap()
            .is_empty());

        tokio::fs::remove_dir_all(root).await.ok();
    }

    #[tokio::test]
    async fn scan_remote_cloud_changes_ignores_nested_space_copy_and_macos_metadata() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = StorageEngine::new(pool);
        let provider = FakeCloudProvider::new();
        let space_id = Uuid::new_v4();
        let now = Utc::now();
        let binding = CloudSpaceBinding {
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            remote_root_path: "/apps/SyncFlow/Notes".to_string(),
            remote_root_id: None,
            sync_mode: "bidirectional".to_string(),
            plaintext: true,
            created_at: now,
            updated_at: now,
        };
        let root = std::env::temp_dir().join(format!("syncflow-cloud-junk-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await.unwrap();
        provider
            .seed_file(
                "/apps/SyncFlow/Notes/Notes/.syncflow/manifest.json",
                b"junk manifest".to_vec(),
            )
            .unwrap();
        provider
            .seed_file("/apps/SyncFlow/Notes/.DS_Store", b"junk".to_vec())
            .unwrap();
        provider
            .seed_file("/apps/SyncFlow/Notes/docs/remote.md", b"remote".to_vec())
            .unwrap();

        scan_remote_cloud_changes(&storage, &provider, space_id, &root, &binding)
            .await
            .unwrap();

        assert!(root.join("docs/remote.md").exists());
        assert!(!root.join("Notes/.syncflow/manifest.json").exists());
        assert!(!root.join(".DS_Store").exists());
        assert!(storage
            .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, "docs/remote.md")
            .await
            .unwrap()
            .is_some());
        assert!(storage
            .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, "Notes/.syncflow/manifest.json")
            .await
            .unwrap()
            .is_none());
        assert!(storage
            .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, ".DS_Store")
            .await
            .unwrap()
            .is_none());
        assert!(storage
            .get_conflicts_for_space(&space_id)
            .await
            .unwrap()
            .is_empty());

        tokio::fs::remove_dir_all(root).await.ok();
    }

    #[tokio::test]
    async fn scan_remote_cloud_changes_records_conflict_when_local_changed() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = StorageEngine::new(pool);
        let provider = FakeCloudProvider::new();
        let space_id = Uuid::new_v4();
        let now = Utc::now();
        let binding = CloudSpaceBinding {
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            remote_root_path: "/apps/SyncFlow/Notes".to_string(),
            remote_root_id: None,
            sync_mode: "bidirectional".to_string(),
            plaintext: true,
            created_at: now,
            updated_at: now,
        };
        let root = std::env::temp_dir().join(format!("syncflow-cloud-conflict-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await.unwrap();
        provider
            .seed_file("/apps/SyncFlow/Notes/remote.md", b"v1".to_vec())
            .unwrap();
        scan_remote_cloud_changes(&storage, &provider, space_id, &root, &binding)
            .await
            .unwrap();
        tokio::fs::write(root.join("remote.md"), b"local edit")
            .await
            .unwrap();
        provider
            .seed_file("/apps/SyncFlow/Notes/remote.md", b"remote edit".to_vec())
            .unwrap();

        scan_remote_cloud_changes(&storage, &provider, space_id, &root, &binding)
            .await
            .unwrap();

        assert_eq!(
            tokio::fs::read(root.join("remote.md")).await.unwrap(),
            b"local edit"
        );
        assert_eq!(
            storage
                .get_conflicts_for_space(&space_id)
                .await
                .unwrap()
                .len(),
            1
        );

        tokio::fs::remove_dir_all(root).await.ok();
    }

    #[test]
    fn derives_relative_path_only_under_remote_root() {
        let entry = CloudRemoteEntry {
            remote_path: "/apps/SyncFlow/Notes/docs/readme.md".to_string(),
            remote_file_id: None,
            is_directory: false,
            size: 0,
            md5: None,
            server_mtime: None,
            remote_revision: None,
        };
        assert_eq!(
            remote_entry_relative_path("/apps/SyncFlow/Notes", &entry).as_deref(),
            Some("docs/readme.md")
        );
        let outside = CloudRemoteEntry {
            remote_path: "/apps/SyncFlow/Other/readme.md".to_string(),
            ..entry
        };
        assert!(remote_entry_relative_path("/apps/SyncFlow/Notes", &outside).is_none());
    }

    #[test]
    fn detects_remote_changes_from_explicit_remote_baseline() {
        let now = Utc::now();
        let task = CloudSyncTask {
            id: 0,
            space_id: Uuid::new_v4(),
            provider: BAIDU_PROVIDER.to_string(),
            task_kind: "upload".to_string(),
            local_relative_path: "docs/readme.md".to_string(),
            remote_path: "/apps/SyncFlow/Notes/docs/readme.md".to_string(),
            expected_remote_revision: None,
            payload_json: None,
            attempts: 0,
            last_error: None,
            created_at: now,
            updated_at: now,
            next_attempt_at: None,
        };
        let entry = CloudRemoteEntry {
            remote_path: task.remote_path.clone(),
            remote_file_id: Some("fs-1".to_string()),
            is_directory: false,
            size: 2,
            md5: Some("md5-v1".to_string()),
            server_mtime: Some(now),
            remote_revision: Some("rev-1".to_string()),
        };
        let metadata = super::remote_metadata_from_entry(&task, &entry, Some(now), false, None);

        assert!(!super::remote_changed_since_cloud_sync(
            Some(&metadata),
            &entry
        ));

        let mut changed_size = entry.clone();
        changed_size.size = 3;
        assert!(super::remote_changed_since_cloud_sync(
            Some(&metadata),
            &changed_size
        ));

        let mut changed_mtime = entry.clone();
        changed_mtime.server_mtime = Some(now + chrono::Duration::seconds(1));
        assert!(super::remote_changed_since_cloud_sync(
            Some(&metadata),
            &changed_mtime
        ));
    }

    #[tokio::test]
    async fn local_manifest_is_written_and_excluded_from_file_scan() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = StorageEngine::new(pool);
        let space_id = Uuid::new_v4();
        let root = std::env::temp_dir().join(format!("syncflow-manifest-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await.unwrap();
        tokio::fs::write(root.join("note.md"), b"note")
            .await
            .unwrap();

        super::write_local_sync_manifest(&storage, space_id, &root, None)
            .await
            .unwrap();

        assert!(root
            .join(super::SYNCFLOW_META_DIR)
            .join(super::SYNCFLOW_MANIFEST_FILE)
            .exists());
        let files = super::collect_indexed_files(&root).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "note.md");

        tokio::fs::remove_dir_all(root).await.ok();
    }

    #[test]
    fn collect_indexed_files_ignores_macos_metadata() {
        let root = std::env::temp_dir().join(format!("syncflow-ds-store-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join(".DS_Store"), b"junk").unwrap();
        std::fs::write(root.join("docs").join(".DS_Store"), b"junk").unwrap();
        std::fs::write(root.join("docs").join("readme.md"), b"readme").unwrap();

        let files = super::collect_indexed_files(&root).unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "docs/readme.md");

        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn imports_local_manifest_into_empty_metadata_cache() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = StorageEngine::new(pool);
        let space_id = Uuid::new_v4();
        let root =
            std::env::temp_dir().join(format!("syncflow-manifest-import-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(root.join(super::SYNCFLOW_META_DIR))
            .await
            .unwrap();
        let now = Utc::now();
        let manifest = super::SyncManifest {
            version: super::SYNCFLOW_MANIFEST_VERSION,
            manifest_id: Some("manifest-1".to_string()),
            sequence: 1,
            base_remote_revision: Some("base-1".to_string()),
            updated_by_device_id: Some("test-device".to_string()),
            space_id: space_id.to_string(),
            provider: Some(BAIDU_PROVIDER.to_string()),
            remote_root_path: Some("/apps/SyncFlow/Notes".to_string()),
            updated_at: now,
            entries: vec![super::SyncManifestEntry {
                relative_path: "docs/readme.md".to_string(),
                is_directory: false,
                local_hash: Some("local-hash".to_string()),
                local_modified_at: Some(now),
                local_size: Some(12),
                remote_path: "/apps/SyncFlow/Notes/docs/readme.md".to_string(),
                remote_file_id: Some("fs-1".to_string()),
                remote_md5: Some("remote-md5".to_string()),
                remote_size: Some(12),
                remote_server_mtime: Some(now),
                remote_revision: Some("rev-1".to_string()),
                last_synced_at: Some(now),
                tombstone: false,
            }],
        };
        super::write_manifest_file(&root, &manifest).await.unwrap();

        super::import_local_sync_manifest(&storage, space_id, &root)
            .await
            .unwrap();

        let imported = storage
            .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, "docs/readme.md")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(imported.last_local_hash.as_deref(), Some("local-hash"));
        assert_eq!(imported.last_remote_revision.as_deref(), Some("rev-1"));
        assert_eq!(imported.last_remote_size, Some(12));

        tokio::fs::remove_dir_all(root).await.ok();
    }

    #[tokio::test]
    async fn manifest_import_and_export_filter_cloud_junk_paths() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = StorageEngine::new(pool);
        let space_id = Uuid::new_v4();
        let now = Utc::now();
        let binding = CloudSpaceBinding {
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            remote_root_path: "/apps/SyncFlow/Notes".to_string(),
            remote_root_id: None,
            sync_mode: "bidirectional".to_string(),
            plaintext: true,
            created_at: now,
            updated_at: now,
        };
        let manifest = super::SyncManifest {
            version: super::SYNCFLOW_MANIFEST_VERSION,
            manifest_id: Some("manifest-1".to_string()),
            sequence: 1,
            base_remote_revision: Some("base-1".to_string()),
            updated_by_device_id: Some("test-device".to_string()),
            space_id: space_id.to_string(),
            provider: Some(BAIDU_PROVIDER.to_string()),
            remote_root_path: Some(binding.remote_root_path.clone()),
            updated_at: now,
            entries: vec![
                super::SyncManifestEntry {
                    relative_path: "docs/readme.md".to_string(),
                    is_directory: false,
                    local_hash: Some("local-hash".to_string()),
                    local_modified_at: Some(now),
                    local_size: Some(12),
                    remote_path: "/apps/SyncFlow/Notes/docs/readme.md".to_string(),
                    remote_file_id: Some("fs-1".to_string()),
                    remote_md5: Some("remote-md5".to_string()),
                    remote_size: Some(12),
                    remote_server_mtime: Some(now),
                    remote_revision: Some("rev-1".to_string()),
                    last_synced_at: Some(now),
                    tombstone: false,
                },
                super::SyncManifestEntry {
                    relative_path: "Notes/.syncflow/manifest.json".to_string(),
                    is_directory: false,
                    local_hash: None,
                    local_modified_at: None,
                    local_size: None,
                    remote_path: "/apps/SyncFlow/Notes/Notes/.syncflow/manifest.json".to_string(),
                    remote_file_id: Some("fs-junk".to_string()),
                    remote_md5: None,
                    remote_size: Some(1),
                    remote_server_mtime: Some(now),
                    remote_revision: Some("rev-junk".to_string()),
                    last_synced_at: Some(now),
                    tombstone: false,
                },
                super::SyncManifestEntry {
                    relative_path: ".DS_Store".to_string(),
                    is_directory: false,
                    local_hash: None,
                    local_modified_at: None,
                    local_size: None,
                    remote_path: "/apps/SyncFlow/Notes/.DS_Store".to_string(),
                    remote_file_id: Some("fs-ds-store".to_string()),
                    remote_md5: None,
                    remote_size: Some(1),
                    remote_server_mtime: Some(now),
                    remote_revision: Some("rev-ds-store".to_string()),
                    last_synced_at: Some(now),
                    tombstone: false,
                },
            ],
        };

        super::import_sync_manifest(&storage, space_id, &manifest)
            .await
            .unwrap();

        assert!(storage
            .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, "docs/readme.md")
            .await
            .unwrap()
            .is_some());
        assert!(storage
            .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, "Notes/.syncflow/manifest.json")
            .await
            .unwrap()
            .is_none());
        assert!(storage
            .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, ".DS_Store")
            .await
            .unwrap()
            .is_none());

        let task = CloudSyncTask {
            id: 0,
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            task_kind: "download".to_string(),
            local_relative_path: "Notes/.syncflow/manifest.json".to_string(),
            remote_path: "/apps/SyncFlow/Notes/Notes/.syncflow/manifest.json".to_string(),
            expected_remote_revision: None,
            payload_json: None,
            attempts: 0,
            last_error: None,
            created_at: now,
            updated_at: now,
            next_attempt_at: None,
        };
        let entry = CloudRemoteEntry {
            remote_path: task.remote_path.clone(),
            remote_file_id: Some("fs-junk".to_string()),
            is_directory: false,
            size: 1,
            md5: None,
            server_mtime: Some(now),
            remote_revision: Some("rev-junk".to_string()),
        };
        storage
            .save_remote_file_metadata(&super::remote_metadata_from_entry(
                &task,
                &entry,
                Some(now),
                false,
                None,
            ))
            .await
            .unwrap();
        let exported =
            super::build_sync_manifest(&storage, space_id, Some(&binding), Some(&manifest), None)
                .await
                .unwrap();

        assert_eq!(exported.entries.len(), 1);
        assert_eq!(exported.entries[0].relative_path, "docs/readme.md");
    }

    #[tokio::test]
    async fn cloud_manifest_upload_imports_remote_manifest_when_base_changed() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = StorageEngine::new(pool);
        let provider = FakeCloudProvider::new();
        let space_id = Uuid::new_v4();
        let root = std::env::temp_dir().join(format!("syncflow-manifest-merge-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(root.join(super::SYNCFLOW_META_DIR))
            .await
            .unwrap();
        let now = Utc::now();
        let binding = CloudSpaceBinding {
            space_id,
            provider: BAIDU_PROVIDER.to_string(),
            remote_root_path: "/apps/SyncFlow/Notes".to_string(),
            remote_root_id: None,
            sync_mode: "bidirectional".to_string(),
            plaintext: true,
            created_at: now,
            updated_at: now,
        };
        let local_manifest = super::SyncManifest {
            version: super::SYNCFLOW_MANIFEST_VERSION,
            manifest_id: Some("manifest-1".to_string()),
            sequence: 1,
            base_remote_revision: Some("old-rev".to_string()),
            updated_by_device_id: Some("device-a".to_string()),
            space_id: space_id.to_string(),
            provider: Some(BAIDU_PROVIDER.to_string()),
            remote_root_path: Some(binding.remote_root_path.clone()),
            updated_at: now,
            entries: vec![],
        };
        super::write_manifest_file(&root, &local_manifest)
            .await
            .unwrap();
        let remote_manifest = super::SyncManifest {
            version: super::SYNCFLOW_MANIFEST_VERSION,
            manifest_id: Some("manifest-1".to_string()),
            sequence: 2,
            base_remote_revision: Some("remote-rev".to_string()),
            updated_by_device_id: Some("device-b".to_string()),
            space_id: space_id.to_string(),
            provider: Some(BAIDU_PROVIDER.to_string()),
            remote_root_path: Some(binding.remote_root_path.clone()),
            updated_at: now,
            entries: vec![super::SyncManifestEntry {
                relative_path: "remote-only.md".to_string(),
                is_directory: false,
                local_hash: Some("hash".to_string()),
                local_modified_at: Some(now),
                local_size: Some(4),
                remote_path: "/apps/SyncFlow/Notes/remote-only.md".to_string(),
                remote_file_id: Some("fs-remote".to_string()),
                remote_md5: Some("md5".to_string()),
                remote_size: Some(4),
                remote_server_mtime: Some(now),
                remote_revision: Some("rev-remote-file".to_string()),
                last_synced_at: Some(now),
                tombstone: false,
            }],
        };
        let remote_manifest_path = root.join("remote-manifest.json");
        tokio::fs::write(
            &remote_manifest_path,
            serde_json::to_vec_pretty(&remote_manifest).unwrap(),
        )
        .await
        .unwrap();
        provider
            .upload_file(
                &remote_manifest_path,
                "/apps/SyncFlow/Notes/.syncflow/manifest.json",
                None,
            )
            .await
            .unwrap();

        super::sync_manifest_to_cloud(
            &storage,
            &provider,
            space_id,
            &root,
            &binding,
            "test-device",
        )
        .await
        .unwrap();

        assert!(storage
            .get_remote_file_metadata(&space_id, BAIDU_PROVIDER, "remote-only.md")
            .await
            .unwrap()
            .is_some());

        tokio::fs::remove_dir_all(root).await.ok();
        tokio::fs::remove_file(remote_manifest_path).await.ok();
    }
}
