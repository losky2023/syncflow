pub mod queue;
pub mod version_vector;
pub mod watcher;

#[cfg(test)]
mod tests;

pub use queue::SyncQueue;
pub use version_vector::{VersionVector, ConflictStatus};
pub use watcher::{FileEvent, start_watcher};

use std::sync::Arc;
use crate::error::Result;
use crate::storage::StorageEngine;
use crate::transport::TransportLayer;
use crate::crypto::{encrypt_data, decrypt_data, hash_data};
use queue::SyncTask;

pub struct SyncEngine {
    storage: Arc<StorageEngine>,
    transport: Arc<TransportLayer>,
    queue: Arc<SyncQueue>,
    version_vectors: std::sync::RwLock<std::collections::HashMap<String, VersionVector>>,
    device_id: String,
    root_key: [u8; 32],
}

impl SyncEngine {
    pub fn new(
        storage: Arc<StorageEngine>,
        transport: Arc<TransportLayer>,
        device_id: String,
        root_key: [u8; 32],
    ) -> Self {
        Self {
            storage,
            transport,
            queue: Arc::new(SyncQueue::new()),
            version_vectors: std::sync::RwLock::new(std::collections::HashMap::new()),
            device_id,
            root_key,
        }
    }

    pub async fn handle_file_event(&self, event: &FileEvent) -> Result<()> {
        match event {
            FileEvent::Created(path) | FileEvent::Modified(path) => {
                let content = tokio::fs::read(path).await?;
                let hash = hash_data(&content);
                let path_str = path.to_str().unwrap_or("").to_string();

                let vv = {
                    let mut vv_map = self.version_vectors.write().unwrap();
                    let vv = vv_map
                        .entry(path_str.clone())
                        .or_insert_with(|| VersionVector::new(&self.device_id));
                    vv.increment(&self.device_id);
                    vv.clone()
                };

                let meta = crate::storage::FileMetadata {
                    path: path_str,
                    hash,
                    size: content.len() as u64,
                    modified_at: chrono::Utc::now(),
                    version_vector: vv.to_json()?,
                    created_at: chrono::Utc::now(),
                };
                self.storage.save_file_meta(&meta).await?;

                let connected = self.transport.connected_peers().await;
                self.queue.enqueue(event, connected).await;
            }
            FileEvent::Deleted(_path) => {
                let connected = self.transport.connected_peers().await;
                self.queue.enqueue(event, connected).await;
            }
        }
        Ok(())
    }

    pub async fn process_queue(&self) -> Result<()> {
        while let Some(task) = self.queue.dequeue().await {
            match task {
                SyncTask::Upload { peer_id, path } => {
                    let content = tokio::fs::read(&path).await?;
                    let vv_map = self.version_vectors.read().unwrap();
                    let vv = vv_map.get(path.to_str().unwrap_or("")).cloned();
                    drop(vv_map);

                    if let Some(vv) = vv {
                        let meta_json = serde_json::json!({
                            "type": "metadata",
                            "path": path.to_str().unwrap_or(""),
                            "hash": hash_data(&content),
                            "size": content.len(),
                            "version_vector": vv.to_json()?,
                        });

                        let encrypted = encrypt_data(&content, &self.root_key)?;

                        let mut message = meta_json.to_string().into_bytes();
                        message.push(0);
                        message.extend(encrypted);

                        self.transport.send_data(&peer_id, &message).await?;
                        tracing::info!("Sent file {} to {}", path.display(), peer_id);
                    }
                }
                SyncTask::Delete { peer_id, path } => {
                    let msg = serde_json::json!({
                        "type": "delete",
                        "path": path.to_str().unwrap_or(""),
                    });
                    let data = msg.to_string().into_bytes();
                    self.transport.send_data(&peer_id, &data).await?;
                    tracing::info!("Sent delete for {} to {}", path.display(), peer_id);
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn receive_file(&self, from: &str, data: &[u8]) -> Result<()> {
        let null_pos = data.iter().position(|&b| b == 0)
            .ok_or_else(|| crate::error::SyncFlowError::WebRtc("Invalid message format".into()))?;

        let meta_json = &data[..null_pos];
        let encrypted = &data[null_pos + 1..];

        let meta: serde_json::Value = serde_json::from_slice(meta_json)
            .map_err(|e| crate::error::SyncFlowError::WebRtc(format!("Invalid metadata: {}", e)))?;

        if meta.get("type").and_then(|v| v.as_str()) == Some("metadata") {
            let path = meta["path"].as_str().unwrap_or("");
            let incoming_vv_json = meta["version_vector"].as_str().unwrap_or("");
            let incoming_vv = VersionVector::from_json(incoming_vv_json)?;

            let vv_map = self.version_vectors.read().unwrap();
            let local_vv = vv_map.get(path).cloned();
            drop(vv_map);

            if let Some(local_vv) = local_vv {
                if local_vv.is_conflicting(&incoming_vv) {
                    tracing::warn!("Conflict detected for file {}", path);
                    return Ok(());
                }
            }

            let decrypted = decrypt_data(encrypted, &self.root_key)?;
            tokio::fs::write(path, &decrypted).await?;

            let mut vv_map = self.version_vectors.write().unwrap();
            let vv = vv_map
                .entry(path.to_string())
                .or_insert_with(|| VersionVector::new(&self.device_id));
            vv.merge(&incoming_vv);

            let file_meta = crate::storage::FileMetadata {
                path: path.to_string(),
                hash: meta["hash"].as_str().unwrap_or("").to_string(),
                size: meta["size"].as_u64().unwrap_or(0),
                modified_at: chrono::Utc::now(),
                version_vector: vv.to_json()?,
                created_at: chrono::Utc::now(),
            };
            self.storage.save_file_meta(&file_meta).await?;

            tracing::info!("Received file {} from {}", path, from);
        }

        Ok(())
    }
}
