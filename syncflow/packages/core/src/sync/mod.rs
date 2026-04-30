pub mod queue;
pub mod version_vector;
pub mod watcher;

#[cfg(test)]
mod tests;

pub use queue::SyncQueue;
pub use version_vector::{ConflictStatus, VersionVector};
pub use watcher::{start_watcher, FileEvent};

use crate::crypto::{decrypt_data, derive_space_key, encrypt_data, hash_data};
use crate::error::Result;
use crate::storage::{ConflictSnapshot, FileMetadata, StorageEngine, SyncConflict};
use crate::transport::TransportLayer;
use chrono::Utc;
use queue::SyncTask;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

const MAX_CONFLICT_TEXT_BYTES: usize = 100_000;

pub struct SyncEngine {
    storage: Arc<StorageEngine>,
    transport: Arc<TransportLayer>,
    queue: Arc<SyncQueue>,
    version_vectors: std::sync::RwLock<std::collections::HashMap<String, VersionVector>>,
    suppressed_remote_writes: std::sync::RwLock<HashMap<String, String>>,
    device_id: String,
}

impl SyncEngine {
    pub fn new(
        storage: Arc<StorageEngine>,
        transport: Arc<TransportLayer>,
        device_id: String,
    ) -> Self {
        Self {
            storage,
            transport,
            queue: Arc::new(SyncQueue::new()),
            version_vectors: std::sync::RwLock::new(std::collections::HashMap::new()),
            suppressed_remote_writes: std::sync::RwLock::new(HashMap::new()),
            device_id,
        }
    }

    pub async fn index_local_file(
        &self,
        space_id: Uuid,
        relative_path: &str,
        resolved_path: &Path,
    ) -> Result<FileMetadata> {
        let content = tokio::fs::read(resolved_path).await?;
        let hash = hash_data(&content);
        let key = file_key(&space_id, relative_path);

        let vv = {
            let mut vv_map = self.version_vectors.write().unwrap();
            let vv = vv_map
                .entry(key)
                .or_insert_with(|| VersionVector::new(&self.device_id));
            vv.increment(&self.device_id);
            vv.clone()
        };

        let meta = FileMetadata {
            space_id,
            relative_path: relative_path.to_string(),
            hash,
            size: content.len() as u64,
            modified_at: Utc::now(),
            version_vector: vv.to_json()?,
            created_at: Utc::now(),
        };
        self.storage.save_file_meta(&meta).await?;
        Ok(meta)
    }

    pub async fn handle_space_file_event(
        &self,
        space_id: Uuid,
        relative_path: &str,
        resolved_path: &Path,
        event: &FileEvent,
    ) -> Result<()> {
        match event {
            FileEvent::Created(_) | FileEvent::Modified(_) => {
                let content = tokio::fs::read(resolved_path).await?;
                let hash = hash_data(&content);
                let file_key = file_key(&space_id, relative_path);

                if self.consume_suppressed_remote_write(&file_key, &hash) {
                    tracing::info!(
                        "Skipped echo for remotely applied file {} in space {}",
                        relative_path,
                        space_id
                    );
                    return Ok(());
                }

                if let Some(existing) = self.storage.get_file_meta(&space_id, relative_path).await?
                {
                    if existing.hash == hash {
                        return Ok(());
                    }
                }

                self.index_local_file(space_id, relative_path, resolved_path)
                    .await?;
                let connected = self.transport.connected_peers().await;
                self.queue
                    .enqueue(space_id, relative_path, resolved_path, event, connected)
                    .await;
            }
            FileEvent::Deleted(_) => {
                self.storage
                    .remove_file_meta(&space_id, relative_path)
                    .await?;
                let connected = self.transport.connected_peers().await;
                self.queue
                    .enqueue(space_id, relative_path, resolved_path, event, connected)
                    .await;
            }
        }
        Ok(())
    }

    pub async fn enqueue_existing_file_for_peer(
        &self,
        space_id: Uuid,
        relative_path: &str,
        resolved_path: &Path,
        peer_id: String,
    ) {
        self.queue
            .enqueue(
                space_id,
                relative_path,
                resolved_path,
                &FileEvent::Modified(resolved_path.to_path_buf()),
                vec![peer_id],
            )
            .await;
    }

    pub async fn handle_file_event(&self, event: &FileEvent) -> Result<()> {
        match event {
            FileEvent::Created(path) | FileEvent::Modified(path) => {
                let fallback_space = Uuid::nil();
                let relative_path = path.to_string_lossy().replace('\\', "/");
                self.index_local_file(fallback_space, &relative_path, path)
                    .await?;
                let connected = self.transport.connected_peers().await;
                self.queue
                    .enqueue(fallback_space, &relative_path, path, event, connected)
                    .await;
            }
            FileEvent::Deleted(path) => {
                let connected = self.transport.connected_peers().await;
                let relative_path = path.to_string_lossy().replace('\\', "/");
                self.queue
                    .enqueue(fallback_space(), &relative_path, path, event, connected)
                    .await;
            }
        }
        Ok(())
    }

    pub async fn process_queue(&self) -> Result<()> {
        while let Some(task) = self.queue.dequeue().await {
            let retry_task = task.clone();
            let outcome: Result<()> = async {
                match task {
                    SyncTask::Upload {
                        peer_id,
                        space_id,
                        relative_path,
                        resolved_path,
                    } => {
                        let content = tokio::fs::read(&resolved_path).await?;
                        let key = file_key(&space_id, &relative_path);
                        let vv = {
                            let vv_map = self.version_vectors.read().unwrap();
                            vv_map.get(&key).cloned()
                        };

                        if let Some(vv) = vv {
                            let Some(space) = self.storage.get_synced_space(&space_id).await?
                            else {
                                return Ok(());
                            };
                            let meta_json = serde_json::json!({
                                "type": "metadata",
                                "sync_key": space.sync_key,
                                "relative_path": relative_path,
                                "hash": hash_data(&content),
                                "size": content.len(),
                                "version_vector": vv.to_json()?,
                            });

                            let space_key = derive_space_key(&space.sync_key);
                            let encrypted = encrypt_data(&content, &space_key)?;

                            let mut message = meta_json.to_string().into_bytes();
                            message.push(0);
                            message.extend(encrypted);

                            self.transport.send_data(&peer_id, &message).await?;
                            tracing::info!(
                                "Sent file {} for space {} to {}",
                                resolved_path.display(),
                                space_id,
                                peer_id
                            );
                        }
                    }
                    SyncTask::Delete {
                        peer_id,
                        space_id,
                        relative_path,
                    } => {
                        let Some(space) = self.storage.get_synced_space(&space_id).await? else {
                            return Ok(());
                        };
                        let msg = serde_json::json!({
                            "type": "delete",
                            "sync_key": space.sync_key,
                            "relative_path": relative_path,
                        });
                        let data = msg.to_string().into_bytes();
                        self.transport.send_data(&peer_id, &data).await?;
                        tracing::info!("Sent delete for {} to {}", relative_path, peer_id);
                    }
                    _ => {}
                }
                Ok(())
            }
            .await;

            if let Err(error) = outcome {
                let is_retryable = matches!(
                    &error,
                    crate::error::SyncFlowError::WebRtc(message)
                        if message.contains("No connection to peer")
                            || message.contains("Failed to send data")
                );
                if is_retryable {
                    self.queue.requeue_front(retry_task).await;
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                }
                return Err(error);
            }
        }
        Ok(())
    }

    pub async fn receive_file(&self, from: &str, data: &[u8]) -> Result<()> {
        self.receive_space_file(from, None, None, data).await
    }

    pub async fn receive_space_file(
        &self,
        from: &str,
        space_root: Option<&Path>,
        expected_local_space_id: Option<Uuid>,
        data: &[u8],
    ) -> Result<()> {
        if let Ok(meta) = serde_json::from_slice::<serde_json::Value>(data) {
            if meta.get("type").and_then(|v| v.as_str()) == Some("delete") {
                let space_id = expected_local_space_id.unwrap_or_else(Uuid::nil);
                let relative_path = meta["relative_path"]
                    .as_str()
                    .or_else(|| meta["path"].as_str())
                    .unwrap_or("");

                if relative_path.is_empty() {
                    return Err(crate::error::SyncFlowError::WebRtc(
                        "Delete message missing relative_path".into(),
                    ));
                }

                if let Some(root) = space_root {
                    let target = safe_join(root, relative_path)?;
                    if target.exists() {
                        if target.is_dir() {
                            tokio::fs::remove_dir_all(&target).await?;
                        } else {
                            tokio::fs::remove_file(&target).await?;
                        }
                    }
                }

                self.storage
                    .remove_file_meta(&space_id, relative_path)
                    .await?;
                tracing::info!("Received delete {} from {}", relative_path, from);
                return Ok(());
            }
        }

        let null_pos = data
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| crate::error::SyncFlowError::WebRtc("Invalid message format".into()))?;

        let meta_json = &data[..null_pos];
        let encrypted = &data[null_pos + 1..];

        let meta: serde_json::Value = serde_json::from_slice(meta_json)
            .map_err(|e| crate::error::SyncFlowError::WebRtc(format!("Invalid metadata: {}", e)))?;

        if meta.get("type").and_then(|v| v.as_str()) == Some("metadata") {
            let space_id = expected_local_space_id.unwrap_or_else(Uuid::nil);
            let sync_key = meta["sync_key"].as_str().unwrap_or("");
            let relative_path = meta["relative_path"]
                .as_str()
                .or_else(|| meta["path"].as_str())
                .unwrap_or("");
            let incoming_hash = meta["hash"].as_str().unwrap_or("").to_string();
            let existing_meta = self.storage.get_file_meta(&space_id, relative_path).await?;
            let incoming_vv_json = meta["version_vector"].as_str().unwrap_or("");
            let incoming_vv = VersionVector::from_json(incoming_vv_json)?;
            let key = file_key(&space_id, relative_path);

            if sync_key.is_empty() {
                return Err(crate::error::SyncFlowError::WebRtc(
                    "File metadata missing sync_key".into(),
                ));
            }

            let space_key = derive_space_key(sync_key);
            let decrypted = decrypt_data(encrypted, &space_key)?;

            if let Some(existing) = existing_meta.as_ref() {
                if existing.hash == incoming_hash {
                    let vv_json = {
                        let mut vv_map = self.version_vectors.write().unwrap();
                        let vv = vv_map
                            .entry(key)
                            .or_insert_with(|| VersionVector::new(&self.device_id));
                        vv.merge(&incoming_vv);
                        vv.to_json()?
                    };

                    if existing.version_vector != vv_json {
                        let mut updated_meta = existing.clone();
                        updated_meta.version_vector = vv_json;
                        updated_meta.modified_at = Utc::now();
                        self.storage.save_file_meta(&updated_meta).await?;
                    }

                    tracing::info!(
                        "Skipped unchanged remote file {} from {}",
                        relative_path,
                        from
                    );
                    return Ok(());
                }
            }

            let local_vv = {
                let vv_map = self.version_vectors.read().unwrap();
                vv_map.get(&key).cloned()
            };

            if let Some(local_vv) = local_vv {
                if local_vv.is_conflicting(&incoming_vv) {
                    let conflict = SyncConflict {
                        id: 0,
                        space_id,
                        relative_path: relative_path.to_string(),
                        local_version: local_vv.to_json()?,
                        remote_version: incoming_vv_json.to_string(),
                        remote_device_id: from.to_string(),
                        detected_at: Utc::now(),
                    };
                    self.storage.save_conflict(&conflict).await?;

                    if let Some(saved_conflict) =
                        self.storage.find_matching_conflict(&conflict).await?
                    {
                        if let Some(snapshot) = build_remote_text_snapshot(
                            saved_conflict.id,
                            space_id,
                            relative_path,
                            &decrypted,
                        ) {
                            self.storage.save_conflict_snapshot(&snapshot).await?;
                        }
                    }
                    tracing::warn!("Conflict detected for file {}", relative_path);
                    return Ok(());
                }
            }
            self.mark_suppressed_remote_write(&key, &incoming_hash);
            if let Some(root) = space_root {
                let target = safe_join(root, relative_path)?;
                if let Some(parent) = target.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::write(&target, &decrypted).await?;
            }

            let vv_json = {
                let mut vv_map = self.version_vectors.write().unwrap();
                let vv = vv_map
                    .entry(key)
                    .or_insert_with(|| VersionVector::new(&self.device_id));
                vv.merge(&incoming_vv);
                vv.to_json()?
            };

            let file_meta = FileMetadata {
                space_id,
                relative_path: relative_path.to_string(),
                hash: incoming_hash,
                size: meta["size"].as_u64().unwrap_or(0),
                modified_at: Utc::now(),
                version_vector: vv_json,
                created_at: Utc::now(),
            };
            self.storage.save_file_meta(&file_meta).await?;

            tracing::info!("Received file {} from {}", relative_path, from);
        }

        Ok(())
    }

    pub fn merge_known_version(
        &self,
        space_id: Uuid,
        relative_path: &str,
        version_vector_json: &str,
    ) -> Result<String> {
        let incoming_vv = VersionVector::from_json(version_vector_json)?;
        let key = file_key(&space_id, relative_path);
        let vv_json = {
            let mut vv_map = self.version_vectors.write().unwrap();
            let vv = vv_map
                .entry(key)
                .or_insert_with(|| VersionVector::new(&self.device_id));
            vv.merge(&incoming_vv);
            vv.to_json()?
        };
        Ok(vv_json)
    }

    fn mark_suppressed_remote_write(&self, file_key: &str, hash: &str) {
        self.suppressed_remote_writes
            .write()
            .unwrap()
            .insert(file_key.to_string(), hash.to_string());
    }

    fn consume_suppressed_remote_write(&self, file_key: &str, hash: &str) -> bool {
        let mut suppressed = self.suppressed_remote_writes.write().unwrap();
        match suppressed.get(file_key) {
            Some(expected_hash) if expected_hash == hash => {
                suppressed.remove(file_key);
                true
            }
            Some(_) => {
                suppressed.remove(file_key);
                false
            }
            None => false,
        }
    }
}

fn fallback_space() -> Uuid {
    Uuid::nil()
}

fn file_key(space_id: &Uuid, relative_path: &str) -> String {
    format!("{}:{}", space_id, relative_path.replace('\\', "/"))
}

fn safe_join(root: &Path, relative_path: &str) -> Result<PathBuf> {
    let path = Path::new(relative_path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(crate::error::SyncFlowError::Conflict(
            "remote path escapes sync space".to_string(),
        ));
    }
    Ok(root.join(path))
}

fn build_remote_text_snapshot(
    conflict_id: i64,
    space_id: Uuid,
    relative_path: &str,
    decrypted: &[u8],
) -> Option<ConflictSnapshot> {
    if !is_text_path(relative_path) {
        return None;
    }

    let truncated = decrypted.len() > MAX_CONFLICT_TEXT_BYTES;
    let content_bytes = if truncated {
        &decrypted[..MAX_CONFLICT_TEXT_BYTES]
    } else {
        decrypted
    };

    Some(ConflictSnapshot {
        id: 0,
        conflict_id,
        space_id,
        relative_path: relative_path.to_string(),
        snapshot_kind: "remote_text".to_string(),
        content_text: Some(String::from_utf8_lossy(content_bytes).to_string()),
        content_truncated: truncated,
        content_size: decrypted.len() as u64,
        created_at: Utc::now(),
    })
}

fn is_text_path(relative_path: &str) -> bool {
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
