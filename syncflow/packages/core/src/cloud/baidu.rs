use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::cloud::provider::{
    CloudAccountState, CloudProvider, CloudProviderKind, CloudRemoteEntry, CloudUploadResult,
};
use crate::crypto::{decrypt_data, derive_space_key};
use crate::error::{Result, SyncFlowError};
use crate::storage::CloudAccount;

pub const BAIDU_OAUTH_AUTHORIZE_URL: &str = "https://openapi.baidu.com/oauth/2.0/authorize";
pub const BAIDU_OAUTH_TOKEN_URL: &str = "https://openapi.baidu.com/oauth/2.0/token";
pub const BAIDU_XPAN_FILE_URL: &str = "https://pan.baidu.com/rest/2.0/xpan/file";
pub const BAIDU_XPAN_MULTIMEDIA_URL: &str = "https://pan.baidu.com/rest/2.0/xpan/multimedia";
pub const BAIDU_PCS_SUPERFILE2_URL: &str = "https://d.pcs.baidu.com/rest/2.0/pcs/superfile2";
pub const BAIDU_PROVIDER: &str = "baidu_netdisk";
pub const DEFAULT_BAIDU_REDIRECT_URI: &str = "oob";
const BAIDU_UPLOAD_BLOCK_SIZE: usize = 4 * 1024 * 1024;
const BAIDU_REQUEST_TIMEOUT_SECONDS: u64 = 30;

#[derive(Debug, Clone)]
pub struct BaiduNetdiskProvider {
    client: reqwest::Client,
    access_token: String,
    account_state: CloudAccountState,
    file_api_base: String,
    multimedia_api_base: String,
    upload_api_base: String,
}

impl BaiduNetdiskProvider {
    pub fn new(access_token: String, account_state: CloudAccountState) -> Self {
        Self {
            client: baidu_http_client(),
            access_token,
            account_state,
            file_api_base: BAIDU_XPAN_FILE_URL.to_string(),
            multimedia_api_base: BAIDU_XPAN_MULTIMEDIA_URL.to_string(),
            upload_api_base: BAIDU_PCS_SUPERFILE2_URL.to_string(),
        }
    }

    pub fn with_file_api_base(
        access_token: String,
        account_state: CloudAccountState,
        file_api_base: String,
    ) -> Self {
        Self {
            client: baidu_http_client(),
            access_token,
            account_state,
            file_api_base,
            multimedia_api_base: BAIDU_XPAN_MULTIMEDIA_URL.to_string(),
            upload_api_base: BAIDU_PCS_SUPERFILE2_URL.to_string(),
        }
    }

    pub fn with_api_bases(
        access_token: String,
        account_state: CloudAccountState,
        file_api_base: String,
        multimedia_api_base: String,
        upload_api_base: String,
    ) -> Self {
        Self {
            client: baidu_http_client(),
            access_token,
            account_state,
            file_api_base,
            multimedia_api_base,
            upload_api_base,
        }
    }

    pub fn from_cloud_account(account: &CloudAccount, client_id: &str) -> Result<Self> {
        if account.provider != BAIDU_PROVIDER {
            return Err(SyncFlowError::Cloud(format!(
                "cloud account provider is not Baidu Netdisk: {}",
                account.provider
            )));
        }
        let access_token = decrypt_baidu_token(&account.access_token_encrypted, client_id)?;
        Ok(Self::new(
            access_token,
            CloudAccountState {
                provider: CloudProviderKind::BaiduNetdisk,
                account_id: account.account_id.clone(),
                display_name: account.display_name.clone(),
                expires_at: account.expires_at,
                scopes: account.scopes.clone(),
            },
        ))
    }

    fn list_url(&self, remote_path: &str) -> String {
        format!(
            "{}?method=list&access_token={}&dir={}",
            self.file_api_base,
            url_encode(&self.access_token),
            url_encode(remote_path)
        )
    }

    fn create_dir_url(&self) -> String {
        format!(
            "{}?method=create&access_token={}",
            self.file_api_base,
            url_encode(&self.access_token)
        )
    }

    fn precreate_url(&self) -> String {
        format!(
            "{}?method=precreate&access_token={}",
            self.file_api_base,
            url_encode(&self.access_token)
        )
    }

    fn upload_block_url(&self, remote_path: &str, upload_id: &str, partseq: usize) -> String {
        format!(
            "{}?method=upload&type=tmpfile&access_token={}&path={}&uploadid={}&partseq={}",
            self.upload_api_base,
            url_encode(&self.access_token),
            url_encode(remote_path),
            url_encode(upload_id),
            partseq
        )
    }

    fn create_file_url(&self) -> String {
        format!(
            "{}?method=create&access_token={}",
            self.file_api_base,
            url_encode(&self.access_token)
        )
    }

    fn filemetas_url(&self, fs_id: &str) -> String {
        format!(
            "{}?method=filemetas&access_token={}&fsids={}&dlink=1",
            self.multimedia_api_base,
            url_encode(&self.access_token),
            url_encode(&format!("[{fs_id}]"))
        )
    }

    fn filemanager_url(&self, opera: &str) -> String {
        format!(
            "{}?method=filemanager&access_token={}&opera={}",
            self.file_api_base,
            url_encode(&self.access_token),
            url_encode(opera)
        )
    }

    async fn precreate_file(
        &self,
        remote_path: &str,
        size: u64,
        block_md5s: &[String],
    ) -> Result<BaiduPrecreateResponse> {
        let block_list = serde_json::to_string(block_md5s)
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu block list encode failed: {e}")))?;
        let response = self
            .client
            .post(self.precreate_url())
            .form(&[
                ("path", remote_path.to_string()),
                ("size", size.to_string()),
                ("isdir", "0".to_string()),
                ("autoinit", "1".to_string()),
                ("rtype", "3".to_string()),
                ("block_list", block_list),
            ])
            .send()
            .await
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu precreate request failed: {e}")))?;
        let status = response.status();
        let body = response.text().await.map_err(|e| {
            SyncFlowError::Cloud(format!("Baidu precreate response read failed: {e}"))
        })?;
        if !status.is_success() {
            return Err(SyncFlowError::Cloud(format!(
                "Baidu precreate failed with HTTP {status}: {}",
                sanitize_api_body(&body)
            )));
        }
        let payload: BaiduPrecreateResponse = serde_json::from_str(&body).map_err(|e| {
            SyncFlowError::Cloud(format!("Baidu precreate response parse failed: {e}"))
        })?;
        payload.ensure_success("precreate")?;
        Ok(payload)
    }

    async fn upload_file_blocks(
        &self,
        local_path: &Path,
        remote_path: &str,
        upload_id: &str,
        partseqs: &[usize],
    ) -> Result<Vec<String>> {
        let content = tokio::fs::read(local_path).await?;
        let mut uploaded_blocks = Vec::new();
        for partseq in partseqs {
            let start = partseq.saturating_mul(BAIDU_UPLOAD_BLOCK_SIZE);
            if start >= content.len() && !content.is_empty() {
                return Err(SyncFlowError::Cloud(format!(
                    "Baidu upload partseq {partseq} is outside file content"
                )));
            }
            let end = std::cmp::min(start + BAIDU_UPLOAD_BLOCK_SIZE, content.len());
            let chunk = if content.is_empty() {
                Vec::new()
            } else {
                content[start..end].to_vec()
            };
            let part = reqwest::multipart::Part::bytes(chunk).file_name("block");
            let form = reqwest::multipart::Form::new().part("file", part);
            let response = self
                .client
                .post(self.upload_block_url(remote_path, upload_id, *partseq))
                .multipart(form)
                .send()
                .await
                .map_err(|e| SyncFlowError::Cloud(format!("Baidu block upload failed: {e}")))?;
            let status = response.status();
            let body = response.text().await.map_err(|e| {
                SyncFlowError::Cloud(format!("Baidu block upload response read failed: {e}"))
            })?;
            if !status.is_success() {
                if status.as_u16() == 400 && body.contains("\"error_code\":31061") {
                    continue;
                }
                return Err(SyncFlowError::Cloud(format!(
                    "Baidu block upload failed with HTTP {status}: {}",
                    sanitize_api_body(&body)
                )));
            }
            let payload: BaiduUploadBlockResponse = serde_json::from_str(&body).map_err(|e| {
                SyncFlowError::Cloud(format!("Baidu block upload response parse failed: {e}"))
            })?;
            if let Some(errno) = payload.errno {
                ensure_baidu_errno_success(Some(errno), payload.errmsg.as_deref(), "upload")?;
            }
            if let Some(md5) = payload.md5 {
                uploaded_blocks.push(md5);
            }
        }
        Ok(uploaded_blocks)
    }

    async fn create_file(
        &self,
        remote_path: &str,
        size: u64,
        upload_id: &str,
        block_list: &[String],
    ) -> Result<CloudRemoteEntry> {
        let block_list_json = serde_json::to_string(block_list)
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu create block list failed: {e}")))?;
        let response = self
            .client
            .post(self.create_file_url())
            .form(&[
                ("path", remote_path.to_string()),
                ("size", size.to_string()),
                ("isdir", "0".to_string()),
                ("rtype", "3".to_string()),
                ("uploadid", upload_id.to_string()),
                ("block_list", block_list_json),
            ])
            .send()
            .await
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu create file request failed: {e}")))?;
        let status = response.status();
        let body = response.text().await.map_err(|e| {
            SyncFlowError::Cloud(format!("Baidu create file response read failed: {e}"))
        })?;
        if !status.is_success() {
            if status.as_u16() == 400 && body.contains("\"error_code\":31061") {
                if let Some(existing) = self.get_metadata(remote_path).await? {
                    return Ok(existing);
                }
            }
            return Err(SyncFlowError::Cloud(format!(
                "Baidu create file failed with HTTP {status}: {}",
                sanitize_api_body(&body)
            )));
        }
        let payload: BaiduCreateResponse = serde_json::from_str(&body).map_err(|e| {
            SyncFlowError::Cloud(format!("Baidu create file response parse failed: {e}"))
        })?;
        payload.ensure_success("create")?;
        Ok(CloudRemoteEntry {
            remote_path: payload.path.unwrap_or_else(|| remote_path.to_string()),
            remote_file_id: payload.fs_id.map(|value| value.to_string()),
            is_directory: false,
            size,
            md5: payload.md5,
            server_mtime: payload
                .server_mtime
                .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single()),
            remote_revision: payload.fs_id.map(|value| value.to_string()),
        })
    }

    async fn get_download_link(&self, fs_id: &str) -> Result<String> {
        let response = self
            .client
            .get(self.filemetas_url(fs_id))
            .send()
            .await
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu filemetas request failed: {e}")))?;
        let status = response.status();
        let body = response.text().await.map_err(|e| {
            SyncFlowError::Cloud(format!("Baidu filemetas response read failed: {e}"))
        })?;
        if !status.is_success() {
            return Err(SyncFlowError::Cloud(format!(
                "Baidu filemetas failed with HTTP {status}: {}",
                sanitize_api_body(&body)
            )));
        }
        let payload: BaiduFileMetasResponse = serde_json::from_str(&body).map_err(|e| {
            SyncFlowError::Cloud(format!("Baidu filemetas response parse failed: {e}"))
        })?;
        payload.ensure_success("filemetas")?;
        payload
            .list
            .and_then(|mut list| list.pop())
            .and_then(|item| item.dlink)
            .ok_or_else(|| {
                SyncFlowError::Cloud("Baidu filemetas response missing dlink".to_string())
            })
    }
}

fn baidu_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(
            BAIDU_REQUEST_TIMEOUT_SECONDS,
        ))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

pub fn encrypt_baidu_token_for_storage(token: &str, client_id: &str) -> Result<Vec<u8>> {
    let key = baidu_token_encryption_key(client_id);
    crate::crypto::encrypt_data(token.as_bytes(), &key)
}

pub fn decrypt_baidu_token(ciphertext: &[u8], client_id: &str) -> Result<String> {
    let key = baidu_token_encryption_key(client_id);
    let plaintext = decrypt_data(ciphertext, &key)?;
    String::from_utf8(plaintext)
        .map_err(|e| SyncFlowError::Cloud(format!("stored Baidu token is not UTF-8: {e}")))
}

fn baidu_token_encryption_key(client_id: &str) -> [u8; 32] {
    derive_space_key(&format!(
        "syncflow-cloud-token:{BAIDU_PROVIDER}:{client_id}"
    ))
}

#[async_trait]
impl CloudProvider for BaiduNetdiskProvider {
    fn provider_kind(&self) -> CloudProviderKind {
        CloudProviderKind::BaiduNetdisk
    }

    async fn account_state(&self) -> Result<CloudAccountState> {
        Ok(self.account_state.clone())
    }

    async fn list_directory(&self, remote_path: &str) -> Result<Vec<CloudRemoteEntry>> {
        let response = self
            .client
            .get(self.list_url(remote_path))
            .send()
            .await
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu list request failed: {e}")))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu list response read failed: {e}")))?;
        if !status.is_success() {
            return Err(SyncFlowError::Cloud(format!(
                "Baidu list failed with HTTP {status}: {}",
                sanitize_api_body(&body)
            )));
        }
        let payload: BaiduListResponse = serde_json::from_str(&body)
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu list response parse failed: {e}")))?;
        payload.ensure_success("list")?;
        Ok(payload
            .list
            .unwrap_or_default()
            .into_iter()
            .map(BaiduFileEntry::into_cloud_entry)
            .collect())
    }

    async fn get_metadata(&self, remote_path: &str) -> Result<Option<CloudRemoteEntry>> {
        let parent = parent_remote_path(remote_path);
        let name = remote_file_name(remote_path);
        Ok(self
            .list_directory(&parent)
            .await?
            .into_iter()
            .find(|entry| remote_file_name(&entry.remote_path) == name))
    }

    async fn create_directory(&self, remote_path: &str) -> Result<CloudRemoteEntry> {
        let response = self
            .client
            .post(self.create_dir_url())
            .form(&[
                ("path", remote_path.to_string()),
                ("isdir", "1".to_string()),
                ("rtype", "0".to_string()),
            ])
            .send()
            .await
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu mkdir request failed: {e}")))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu mkdir response read failed: {e}")))?;
        if !status.is_success() {
            return Err(SyncFlowError::Cloud(format!(
                "Baidu mkdir failed with HTTP {status}: {}",
                sanitize_api_body(&body)
            )));
        }
        let payload: BaiduCreateResponse = serde_json::from_str(&body)
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu mkdir response parse failed: {e}")))?;
        payload.ensure_success("mkdir")?;
        Ok(CloudRemoteEntry {
            remote_path: payload.path.unwrap_or_else(|| remote_path.to_string()),
            remote_file_id: payload.fs_id.map(|value| value.to_string()),
            is_directory: true,
            size: 0,
            md5: None,
            server_mtime: None,
            remote_revision: payload.fs_id.map(|value| value.to_string()),
        })
    }

    async fn upload_file(
        &self,
        local_path: &Path,
        remote_path: &str,
        _expected_remote_revision: Option<&str>,
    ) -> Result<CloudUploadResult> {
        let metadata = tokio::fs::metadata(local_path).await?;
        if !metadata.is_file() {
            return Err(SyncFlowError::Cloud(format!(
                "Baidu upload source is not a file: {}",
                local_path.display()
            )));
        }
        let block_md5s = compute_file_block_md5s(local_path, BAIDU_UPLOAD_BLOCK_SIZE).await?;
        let precreate = self
            .precreate_file(remote_path, metadata.len(), &block_md5s)
            .await?;
        let upload_id = precreate.uploadid.ok_or_else(|| {
            SyncFlowError::Cloud("Baidu precreate response did not include uploadid".to_string())
        })?;
        let partseqs = precreate
            .block_list
            .unwrap_or_else(|| (0..block_md5s.len()).collect());
        let block_list = self
            .upload_file_blocks(local_path, remote_path, &upload_id, &partseqs)
            .await?;
        let block_list = if block_list.is_empty() {
            block_md5s.clone()
        } else {
            block_list
        };
        let created = self
            .create_file(remote_path, metadata.len(), &upload_id, &block_list)
            .await?;
        Ok(CloudUploadResult { entry: created })
    }

    async fn download_file(&self, remote_path: &str, local_path: &Path) -> Result<()> {
        let metadata = self
            .get_metadata(remote_path)
            .await?
            .ok_or_else(|| SyncFlowError::FileNotFound(remote_path.to_string()))?;
        let fs_id = metadata.remote_file_id.ok_or_else(|| {
            SyncFlowError::Cloud(format!("Baidu remote file has no fs_id: {remote_path}"))
        })?;
        let dlink = self.get_download_link(&fs_id).await?;
        let download_url = append_access_token(&dlink, &self.access_token);
        let response = self
            .client
            .get(download_url)
            .send()
            .await
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu download request failed: {e}")))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(SyncFlowError::Cloud(format!(
                "Baidu download failed with HTTP {status}: {}",
                sanitize_api_body(&body)
            )));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu download body failed: {e}")))?;
        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(local_path, bytes).await?;
        Ok(())
    }

    async fn delete_path(
        &self,
        remote_path: &str,
        _expected_remote_revision: Option<&str>,
    ) -> Result<()> {
        let filelist = serde_json::to_string(&vec![remote_path])
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu delete payload failed: {e}")))?;
        let response = self
            .client
            .post(self.filemanager_url("delete"))
            .form(&[("filelist", filelist)])
            .send()
            .await
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu delete request failed: {e}")))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu delete response read failed: {e}")))?;
        if !status.is_success() {
            return Err(SyncFlowError::Cloud(format!(
                "Baidu delete failed with HTTP {status}: {}",
                sanitize_api_body(&body)
            )));
        }
        let payload: BaiduFileManagerResponse = serde_json::from_str(&body).map_err(|e| {
            SyncFlowError::Cloud(format!("Baidu delete response parse failed: {e}"))
        })?;
        payload.ensure_success("delete")
    }

    async fn move_path(
        &self,
        from_remote_path: &str,
        to_remote_path: &str,
        _expected_remote_revision: Option<&str>,
    ) -> Result<CloudRemoteEntry> {
        let parent = parent_remote_path(to_remote_path);
        let name = remote_file_name(to_remote_path);
        let filelist = serde_json::to_string(&vec![serde_json::json!({
            "path": from_remote_path,
            "dest": parent,
            "newname": name,
        })])
        .map_err(|e| SyncFlowError::Cloud(format!("Baidu move payload failed: {e}")))?;
        let response = self
            .client
            .post(self.filemanager_url("move"))
            .form(&[("filelist", filelist)])
            .send()
            .await
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu move request failed: {e}")))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu move response read failed: {e}")))?;
        if !status.is_success() {
            return Err(SyncFlowError::Cloud(format!(
                "Baidu move failed with HTTP {status}: {}",
                sanitize_api_body(&body)
            )));
        }
        let payload: BaiduFileManagerResponse = serde_json::from_str(&body)
            .map_err(|e| SyncFlowError::Cloud(format!("Baidu move response parse failed: {e}")))?;
        payload.ensure_success("move")?;
        Ok(CloudRemoteEntry {
            remote_path: to_remote_path.to_string(),
            remote_file_id: None,
            is_directory: false,
            size: 0,
            md5: None,
            server_mtime: None,
            remote_revision: None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaiduOAuthConfig {
    pub device_id: Option<String>,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
}

impl BaiduOAuthConfig {
    pub fn from_env() -> Result<Self> {
        let client_id = std::env::var("SYNCFLOW_BAIDU_CLIENT_ID")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                SyncFlowError::Cloud("SYNCFLOW_BAIDU_CLIENT_ID is not configured".to_string())
            })?;
        let client_secret = std::env::var("SYNCFLOW_BAIDU_CLIENT_SECRET")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let device_id = std::env::var("SYNCFLOW_BAIDU_DEVICE_ID")
            .ok()
            .or_else(|| std::env::var("SYNCFLOW_BAIDU_APP_ID").ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let redirect_uri = std::env::var("SYNCFLOW_BAIDU_REDIRECT_URI")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_BAIDU_REDIRECT_URI.to_string());
        let scopes = std::env::var("SYNCFLOW_BAIDU_SCOPES")
            .ok()
            .map(|value| parse_scopes(&value))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| vec!["basic".to_string(), "netdisk".to_string()]);

        Ok(Self {
            device_id,
            client_id,
            client_secret,
            redirect_uri,
            scopes,
        })
    }

    pub fn authorization_url(&self, state: &str) -> String {
        let mut url = build_authorization_url(
            &self.client_id,
            &self.redirect_uri,
            &self.scopes,
            state,
            "code",
        );
        if let Some(device_id) = &self.device_id {
            url.push_str("&device_id=");
            url.push_str(&url_encode(device_id));
        }
        url
    }

    pub fn implicit_authorization_url(&self, state: &str) -> String {
        let mut url = build_authorization_url(
            &self.client_id,
            &self.redirect_uri,
            &self.scopes,
            state,
            "token",
        );
        if let Some(device_id) = &self.device_id {
            url.push_str("&device_id=");
            url.push_str(&url_encode(device_id));
        }
        url
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaiduTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: Option<i64>,
    pub scope: Option<String>,
    pub session_key: Option<String>,
    pub session_secret: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BaiduListResponse {
    errno: Option<i64>,
    errmsg: Option<String>,
    list: Option<Vec<BaiduFileEntry>>,
}

#[derive(Debug, Deserialize)]
struct BaiduCreateResponse {
    errno: Option<i64>,
    errmsg: Option<String>,
    path: Option<String>,
    fs_id: Option<i64>,
    md5: Option<String>,
    server_mtime: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct BaiduPrecreateResponse {
    errno: Option<i64>,
    errmsg: Option<String>,
    uploadid: Option<String>,
    block_list: Option<Vec<usize>>,
}

#[derive(Debug, Deserialize)]
struct BaiduUploadBlockResponse {
    errno: Option<i64>,
    errmsg: Option<String>,
    md5: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BaiduFileManagerResponse {
    errno: Option<i64>,
    errmsg: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BaiduFileMetasResponse {
    errno: Option<i64>,
    errmsg: Option<String>,
    list: Option<Vec<BaiduFileMetaItem>>,
}

#[derive(Debug, Deserialize)]
struct BaiduFileMetaItem {
    dlink: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BaiduFileEntry {
    path: String,
    fs_id: Option<i64>,
    isdir: Option<i64>,
    size: Option<u64>,
    md5: Option<String>,
    server_mtime: Option<i64>,
}

impl BaiduListResponse {
    fn ensure_success(&self, operation: &str) -> Result<()> {
        ensure_baidu_errno_success(self.errno, self.errmsg.as_deref(), operation)
    }
}

impl BaiduCreateResponse {
    fn ensure_success(&self, operation: &str) -> Result<()> {
        ensure_baidu_errno_success(self.errno, self.errmsg.as_deref(), operation)
    }
}

impl BaiduPrecreateResponse {
    fn ensure_success(&self, operation: &str) -> Result<()> {
        ensure_baidu_errno_success(self.errno, self.errmsg.as_deref(), operation)
    }
}

impl BaiduFileManagerResponse {
    fn ensure_success(&self, operation: &str) -> Result<()> {
        ensure_baidu_errno_success(self.errno, self.errmsg.as_deref(), operation)
    }
}

impl BaiduFileMetasResponse {
    fn ensure_success(&self, operation: &str) -> Result<()> {
        ensure_baidu_errno_success(self.errno, self.errmsg.as_deref(), operation)
    }
}

impl BaiduFileEntry {
    fn into_cloud_entry(self) -> CloudRemoteEntry {
        let server_mtime = self
            .server_mtime
            .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single());
        let remote_revision = self
            .fs_id
            .map(|value| value.to_string())
            .or_else(|| self.md5.clone());
        CloudRemoteEntry {
            remote_path: self.path,
            remote_file_id: self.fs_id.map(|value| value.to_string()),
            is_directory: self.isdir.unwrap_or(0) == 1,
            size: self.size.unwrap_or(0),
            md5: self.md5,
            server_mtime,
            remote_revision,
        }
    }
}

fn ensure_baidu_errno_success(
    errno: Option<i64>,
    errmsg: Option<&str>,
    operation: &str,
) -> Result<()> {
    match errno.unwrap_or(0) {
        0 => Ok(()),
        errno => Err(SyncFlowError::Cloud(format!(
            "Baidu {operation} failed errno={errno}: {}",
            errmsg.unwrap_or("unknown error")
        ))),
    }
}

pub fn build_authorization_url(
    client_id: &str,
    redirect_uri: &str,
    scopes: &[String],
    state: &str,
    response_type: &str,
) -> String {
    let scope = scopes.join(",");
    format!(
        "{}?response_type={}&client_id={}&redirect_uri={}&scope={}&state={}",
        BAIDU_OAUTH_AUTHORIZE_URL,
        url_encode(response_type),
        url_encode(client_id),
        url_encode(redirect_uri),
        url_encode(&scope),
        url_encode(state)
    )
}

pub fn parse_scope_string(scope: Option<&str>, fallback: &[String]) -> Vec<String> {
    scope
        .map(parse_scopes)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_vec())
}

fn parse_scopes(value: &str) -> Vec<String> {
    value
        .split(|ch: char| ch == ',' || ch == ' ' || ch == ';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

async fn compute_file_block_md5s(local_path: &Path, block_size: usize) -> Result<Vec<String>> {
    if block_size == 0 {
        return Err(SyncFlowError::Cloud(
            "Baidu upload block size must be greater than zero".to_string(),
        ));
    }
    let content = tokio::fs::read(local_path).await?;
    if content.is_empty() {
        return Ok(vec![hex_md5(&[])]);
    }
    Ok(content.chunks(block_size).map(hex_md5).collect())
}

fn hex_md5(bytes: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn parent_remote_path(remote_path: &str) -> String {
    let trimmed = remote_path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(index) => trimmed[..index].to_string(),
    }
}

fn remote_file_name(remote_path: &str) -> String {
    remote_path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(remote_path)
        .to_string()
}

fn sanitize_api_body(body: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
    if let serde_json::Value::Object(map) = value {
        let mut sanitized = serde_json::Map::new();
        for (key, value) in map {
            if matches!(key.as_str(), "access_token" | "refresh_token" | "dlink") {
                sanitized.insert(key, serde_json::Value::String("<redacted>".to_string()));
            } else {
                sanitized.insert(key, value);
            }
        }
        serde_json::Value::Object(sanitized).to_string()
    } else {
        body.chars().take(400).collect()
    }
}

fn append_access_token(url: &str, access_token: &str) -> String {
    let separator = if url.contains('?') { "&" } else { "?" };
    format!("{url}{separator}access_token={}", url_encode(access_token))
}

fn url_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_authorization_url_with_encoded_redirect_scope_and_state() {
        let url = build_authorization_url(
            "client id",
            "http://127.0.0.1:18082/callback",
            &["basic".to_string(), "netdisk".to_string()],
            "state value",
            "code",
        );

        assert!(url.starts_with(BAIDU_OAUTH_AUTHORIZE_URL));
        assert!(url.contains("client_id=client%20id"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A18082%2Fcallback"));
        assert!(url.contains("scope=basic%2Cnetdisk"));
        assert!(url.contains("state=state%20value"));
    }

    #[test]
    fn builds_implicit_authorization_url() {
        let config = BaiduOAuthConfig {
            device_id: Some("app-id".to_string()),
            client_id: "client-id".to_string(),
            client_secret: None,
            redirect_uri: "oob".to_string(),
            scopes: vec!["basic".to_string(), "netdisk".to_string()],
        };

        let url = config.implicit_authorization_url("state value");

        assert!(url.contains("response_type=token"));
        assert!(url.contains("device_id=app-id"));
        assert!(url.contains("redirect_uri=oob"));
        assert!(url.contains("scope=basic%2Cnetdisk"));
        assert!(url.contains("state=state%20value"));
    }

    #[test]
    fn parses_scope_response_or_uses_fallback() {
        let fallback = vec!["basic".to_string()];
        assert_eq!(
            parse_scope_string(Some("basic netdisk"), &fallback),
            vec!["basic".to_string(), "netdisk".to_string()]
        );
        assert_eq!(
            parse_scope_string(Some("basic,netdisk"), &fallback),
            vec!["basic".to_string(), "netdisk".to_string()]
        );
        assert_eq!(parse_scope_string(None, &fallback), fallback);
    }

    #[test]
    fn converts_baidu_file_entry_to_cloud_entry() {
        let entry = BaiduFileEntry {
            path: "/apps/SyncFlow/Notes/readme.md".to_string(),
            fs_id: Some(123),
            isdir: Some(0),
            size: Some(42),
            md5: Some("md5".to_string()),
            server_mtime: Some(1_700_000_000),
        }
        .into_cloud_entry();

        assert_eq!(entry.remote_file_id.as_deref(), Some("123"));
        assert_eq!(entry.remote_revision.as_deref(), Some("123"));
        assert!(!entry.is_directory);
        assert_eq!(entry.size, 42);
        assert!(entry.server_mtime.is_some());
    }

    #[test]
    fn computes_parent_and_file_name_for_remote_paths() {
        assert_eq!(
            parent_remote_path("/apps/SyncFlow/Notes/readme.md"),
            "/apps/SyncFlow/Notes"
        );
        assert_eq!(
            remote_file_name("/apps/SyncFlow/Notes/readme.md"),
            "readme.md"
        );
        assert_eq!(parent_remote_path("/readme.md"), "/");
    }

    #[test]
    fn builds_baidu_provider_urls_without_raw_tokens() {
        let provider = BaiduNetdiskProvider::with_file_api_base(
            "token value".to_string(),
            CloudAccountState {
                provider: CloudProviderKind::BaiduNetdisk,
                account_id: None,
                display_name: None,
                expires_at: None,
                scopes: vec![],
            },
            "https://example.test/xpan/file".to_string(),
        );
        let url = provider.list_url("/apps/SyncFlow/Notes");
        assert!(url.contains("access_token=token%20value"));
        assert!(url.contains("dir=%2Fapps%2FSyncFlow%2FNotes"));
        assert!(provider
            .precreate_url()
            .contains("method=precreate&access_token=token%20value"));
        assert!(provider
            .upload_block_url("/apps/SyncFlow/Notes/readme.md", "upload id", 2)
            .contains("/pcs/superfile2?method=upload&type=tmpfile"));
        assert!(provider
            .upload_block_url("/apps/SyncFlow/Notes/readme.md", "upload id", 2)
            .contains("uploadid=upload%20id&partseq=2"));
        assert!(provider
            .create_file_url()
            .contains("method=create&access_token=token%20value"));
        assert!(provider
            .filemetas_url("123")
            .contains("fsids=%5B123%5D&dlink=1"));
    }

    #[test]
    fn baidu_token_storage_encryption_roundtrips() {
        let encrypted = encrypt_baidu_token_for_storage("access-token", "client-id").unwrap();
        assert_ne!(encrypted, b"access-token");
        let decrypted = decrypt_baidu_token(&encrypted, "client-id").unwrap();
        assert_eq!(decrypted, "access-token");
        assert!(decrypt_baidu_token(&encrypted, "other-client").is_err());
    }

    #[tokio::test]
    async fn computes_file_block_md5s() {
        let root = std::env::temp_dir().join(format!("syncflow-md5-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await.unwrap();
        let path = root.join("data.txt");
        tokio::fs::write(&path, b"abcdef").await.unwrap();

        let hashes = compute_file_block_md5s(&path, 3).await.unwrap();
        assert_eq!(hashes, vec![hex_md5(b"abc"), hex_md5(b"def")]);

        tokio::fs::remove_dir_all(root).await.ok();
    }

    #[test]
    fn appends_access_token_to_download_link() {
        assert_eq!(
            append_access_token("https://example.test/file", "token value"),
            "https://example.test/file?access_token=token%20value"
        );
        assert_eq!(
            append_access_token("https://example.test/file?x=1", "token value"),
            "https://example.test/file?x=1&access_token=token%20value"
        );
    }
}
