use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudProviderKind {
    BaiduNetdisk,
}

impl CloudProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BaiduNetdisk => "baidu_netdisk",
        }
    }
}

impl TryFrom<&str> for CloudProviderKind {
    type Error = String;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "baidu_netdisk" => Ok(Self::BaiduNetdisk),
            other => Err(format!("unknown cloud provider: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudRemoteEntry {
    pub remote_path: String,
    pub remote_file_id: Option<String>,
    pub is_directory: bool,
    pub size: u64,
    pub md5: Option<String>,
    pub server_mtime: Option<DateTime<Utc>>,
    pub remote_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudUploadResult {
    pub entry: CloudRemoteEntry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudProviderErrorKind {
    AuthExpired,
    PermissionDenied,
    QuotaExceeded,
    RateLimited,
    NetworkUnavailable,
    NotFound,
    Conflict,
    PathInvalid,
    ProviderUnavailable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudProviderError {
    pub kind: CloudProviderErrorKind,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudAccountState {
    pub provider: CloudProviderKind,
    pub account_id: Option<String>,
    pub display_name: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub scopes: Vec<String>,
}

#[async_trait]
pub trait CloudProvider: Send + Sync {
    fn provider_kind(&self) -> CloudProviderKind;

    async fn account_state(&self) -> Result<CloudAccountState>;

    async fn list_directory(&self, remote_path: &str) -> Result<Vec<CloudRemoteEntry>>;

    async fn get_metadata(&self, remote_path: &str) -> Result<Option<CloudRemoteEntry>>;

    async fn create_directory(&self, remote_path: &str) -> Result<CloudRemoteEntry>;

    async fn upload_file(
        &self,
        local_path: &Path,
        remote_path: &str,
        expected_remote_revision: Option<&str>,
    ) -> Result<CloudUploadResult>;

    async fn download_file(&self, remote_path: &str, local_path: &Path) -> Result<()>;

    async fn delete_path(
        &self,
        remote_path: &str,
        expected_remote_revision: Option<&str>,
    ) -> Result<()>;

    async fn move_path(
        &self,
        from_remote_path: &str,
        to_remote_path: &str,
        expected_remote_revision: Option<&str>,
    ) -> Result<CloudRemoteEntry>;
}
