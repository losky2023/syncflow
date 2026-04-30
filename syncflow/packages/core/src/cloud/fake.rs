use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::cloud::provider::{
    CloudAccountState, CloudProvider, CloudProviderKind, CloudRemoteEntry, CloudUploadResult,
};
use crate::crypto::hash_data;
use crate::error::{Result, SyncFlowError};

#[derive(Debug, Clone, Default)]
pub struct FakeCloudProvider {
    entries: Arc<RwLock<HashMap<String, FakeEntry>>>,
}

#[derive(Debug, Clone)]
struct FakeEntry {
    entry: CloudRemoteEntry,
    content: Vec<u8>,
}

impl FakeCloudProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seed_file(&self, remote_path: &str, content: Vec<u8>) -> Result<CloudRemoteEntry> {
        let entry = file_entry(remote_path, &content);
        self.entries
            .write()
            .map_err(|_| SyncFlowError::Cloud("fake provider lock poisoned".to_string()))?
            .insert(
                normalize_remote_path(remote_path),
                FakeEntry {
                    entry: entry.clone(),
                    content,
                },
            );
        Ok(entry)
    }
}

#[async_trait]
impl CloudProvider for FakeCloudProvider {
    fn provider_kind(&self) -> CloudProviderKind {
        CloudProviderKind::BaiduNetdisk
    }

    async fn account_state(&self) -> Result<CloudAccountState> {
        Ok(CloudAccountState {
            provider: CloudProviderKind::BaiduNetdisk,
            account_id: Some("fake-account".to_string()),
            display_name: Some("Fake Baidu Account".to_string()),
            expires_at: None,
            scopes: vec!["basic".to_string(), "netdisk".to_string()],
        })
    }

    async fn list_directory(&self, remote_path: &str) -> Result<Vec<CloudRemoteEntry>> {
        let prefix = normalize_directory_prefix(remote_path);
        let entries = self
            .entries
            .read()
            .map_err(|_| SyncFlowError::Cloud("fake provider lock poisoned".to_string()))?;
        Ok(entries
            .values()
            .filter(|entry| entry.entry.remote_path.starts_with(&prefix))
            .map(|entry| entry.entry.clone())
            .collect())
    }

    async fn get_metadata(&self, remote_path: &str) -> Result<Option<CloudRemoteEntry>> {
        Ok(self
            .entries
            .read()
            .map_err(|_| SyncFlowError::Cloud("fake provider lock poisoned".to_string()))?
            .get(&normalize_remote_path(remote_path))
            .map(|entry| entry.entry.clone()))
    }

    async fn create_directory(&self, remote_path: &str) -> Result<CloudRemoteEntry> {
        let normalized = normalize_remote_path(remote_path);
        let entry = CloudRemoteEntry {
            remote_file_id: Some(format!("fake-dir:{normalized}")),
            remote_path: normalized.clone(),
            is_directory: true,
            size: 0,
            md5: None,
            server_mtime: Some(Utc::now()),
            remote_revision: Some(format!(
                "dir:{}",
                Utc::now().timestamp_nanos_opt().unwrap_or_default()
            )),
        };
        self.entries
            .write()
            .map_err(|_| SyncFlowError::Cloud("fake provider lock poisoned".to_string()))?
            .insert(
                normalized,
                FakeEntry {
                    entry: entry.clone(),
                    content: Vec::new(),
                },
            );
        Ok(entry)
    }

    async fn upload_file(
        &self,
        local_path: &Path,
        remote_path: &str,
        expected_remote_revision: Option<&str>,
    ) -> Result<CloudUploadResult> {
        let normalized = normalize_remote_path(remote_path);
        let content = tokio::fs::read(local_path).await?;
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SyncFlowError::Cloud("fake provider lock poisoned".to_string()))?;
        if let Some(expected) = expected_remote_revision {
            let current = entries
                .get(&normalized)
                .and_then(|entry| entry.entry.remote_revision.as_deref());
            if current != Some(expected) {
                return Err(SyncFlowError::Cloud("remote revision conflict".to_string()));
            }
        }
        let entry = file_entry(&normalized, &content);
        entries.insert(
            normalized,
            FakeEntry {
                entry: entry.clone(),
                content,
            },
        );
        Ok(CloudUploadResult { entry })
    }

    async fn download_file(&self, remote_path: &str, local_path: &Path) -> Result<()> {
        let content = self
            .entries
            .read()
            .map_err(|_| SyncFlowError::Cloud("fake provider lock poisoned".to_string()))?
            .get(&normalize_remote_path(remote_path))
            .map(|entry| entry.content.clone())
            .ok_or_else(|| SyncFlowError::FileNotFound(remote_path.to_string()))?;
        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(local_path, content).await?;
        Ok(())
    }

    async fn delete_path(
        &self,
        remote_path: &str,
        expected_remote_revision: Option<&str>,
    ) -> Result<()> {
        let normalized = normalize_remote_path(remote_path);
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SyncFlowError::Cloud("fake provider lock poisoned".to_string()))?;
        if let Some(expected) = expected_remote_revision {
            let current = entries
                .get(&normalized)
                .and_then(|entry| entry.entry.remote_revision.as_deref());
            if current != Some(expected) {
                return Err(SyncFlowError::Cloud("remote revision conflict".to_string()));
            }
        }
        entries.remove(&normalized);
        Ok(())
    }

    async fn move_path(
        &self,
        from_remote_path: &str,
        to_remote_path: &str,
        expected_remote_revision: Option<&str>,
    ) -> Result<CloudRemoteEntry> {
        let from = normalize_remote_path(from_remote_path);
        let to = normalize_remote_path(to_remote_path);
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SyncFlowError::Cloud("fake provider lock poisoned".to_string()))?;
        let mut fake_entry = entries
            .remove(&from)
            .ok_or_else(|| SyncFlowError::FileNotFound(from_remote_path.to_string()))?;
        if let Some(expected) = expected_remote_revision {
            if fake_entry.entry.remote_revision.as_deref() != Some(expected) {
                entries.insert(from, fake_entry);
                return Err(SyncFlowError::Cloud("remote revision conflict".to_string()));
            }
        }
        fake_entry.entry.remote_path = to.clone();
        fake_entry.entry.remote_file_id = Some(format!("fake:{}", to));
        fake_entry.entry.remote_revision = Some(format!(
            "rev:{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let entry = fake_entry.entry.clone();
        entries.insert(to, fake_entry);
        Ok(entry)
    }
}

fn file_entry(remote_path: &str, content: &[u8]) -> CloudRemoteEntry {
    let normalized = normalize_remote_path(remote_path);
    let hash = hash_data(content);
    CloudRemoteEntry {
        remote_file_id: Some(format!("fake:{}", normalized)),
        remote_path: normalized,
        is_directory: false,
        size: content.len() as u64,
        md5: Some(hash.clone()),
        server_mtime: Some(Utc::now()),
        remote_revision: Some(hash),
    }
}

fn normalize_directory_prefix(remote_path: &str) -> String {
    let normalized = normalize_remote_path(remote_path);
    if normalized.ends_with('/') {
        normalized
    } else {
        format!("{normalized}/")
    }
}

fn normalize_remote_path(remote_path: &str) -> String {
    let normalized = remote_path.replace('\\', "/");
    if normalized.starts_with('/') {
        normalized
    } else {
        format!("/{normalized}")
    }
}
