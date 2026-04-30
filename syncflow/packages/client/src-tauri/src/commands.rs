use base64::Engine;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use syncflow_core::cloud::{
    encrypt_baidu_token_for_storage, parse_scope_string, BaiduOAuthConfig, BaiduTokenResponse,
    BAIDU_OAUTH_TOKEN_URL, BAIDU_PROVIDER, DEFAULT_BAIDU_REDIRECT_URI,
};
use syncflow_core::storage::{CloudAccount, CloudApiConfig, CloudSpaceBinding, StorageEngine};
use tauri::State;
use uuid::Uuid;

use crate::fs_safety::{parse_space_id, resolve_space_path, strip_root_prefix};
use crate::runtime::{DeviceStateDto, SyncRuntimeStatusDto};
use crate::TauriState;

const CLOUD_REMOTE_DELETED_DEVICE_ID: &str = "baidu_netdisk:remote_deleted";

#[derive(Serialize)]
pub struct AuthResult {
    pub success: bool,
    pub error: Option<String>,
    pub account_id: String,
    pub device_id: String,
    pub device_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncedSpaceDto {
    pub id: String,
    pub sync_key: String,
    pub name: String,
    pub root_path: String,
    pub status: String,
    pub created_at: String,
    pub last_scanned_at: Option<String>,
    pub cloud_binding: Option<CloudSpaceBindingDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudSpaceBindingDto {
    pub space_id: String,
    pub provider: String,
    pub remote_root_path: String,
    pub remote_root_id: Option<String>,
    pub sync_mode: String,
    pub plaintext: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    pub name: String,
    pub relative_path: String,
    pub node_type: String,
    pub has_children: bool,
    pub extension: Option<String>,
    pub size: Option<u64>,
    pub modified_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDetails {
    pub name: String,
    pub node_type: String,
    pub extension: Option<String>,
    pub size: u64,
    pub modified_at: Option<String>,
    pub space_name: String,
    pub relative_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPreviewResult {
    pub content: String,
    pub truncated: bool,
    pub size: u64,
    pub max_bytes: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveTextFileRequest {
    pub space_id: String,
    pub relative_path: String,
    pub content: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveTextFileResult {
    pub details: FileDetails,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTreeItemRequest {
    pub space_id: String,
    pub parent_relative_path: Option<String>,
    pub name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePreviewResult {
    pub data_url: String,
    pub mime_type: String,
    pub size: u64,
    pub truncated: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictInfo {
    pub id: i64,
    pub space_id: String,
    pub relative_path: String,
    pub local_version: String,
    pub remote_version: String,
    pub remote_device: String,
    pub detected_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictDetail {
    pub id: i64,
    pub space_id: String,
    pub space_name: String,
    pub relative_path: String,
    pub remote_device: String,
    pub detected_at: String,
    pub local_version: String,
    pub remote_version: String,
    pub local_file_exists: bool,
    pub is_text: bool,
    pub local_text_content: Option<String>,
    pub local_text_truncated: Option<bool>,
    pub remote_text_content: Option<String>,
    pub remote_text_truncated: Option<bool>,
    pub can_keep_local: bool,
    pub can_keep_remote: bool,
    pub can_compare_text: bool,
    pub missing_remote_snapshot_reason: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncDiagnosticsDto {
    pub space_id: String,
    pub space_name: String,
    pub root_path: String,
    pub cloud_provider: Option<String>,
    pub cloud_remote_path: Option<String>,
    pub summary: SyncSummaryDto,
    pub manifest: Option<SyncManifestSummaryDto>,
    pub queue: Vec<CloudSyncTaskDto>,
    pub conflicts: Vec<ConflictInfo>,
    pub remote_deletions: Vec<RemoteDeletionNoticeDto>,
    pub safety_notes: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSummaryDto {
    pub last_successful_sync_at: Option<String>,
    pub last_sync_activity_at: Option<String>,
    pub last_sync_error_at: Option<String>,
    pub last_sync_error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncManifestSummaryDto {
    pub path: String,
    pub version: u32,
    pub manifest_id: Option<String>,
    pub sequence: u64,
    pub base_remote_revision: Option<String>,
    pub updated_by_device_id: Option<String>,
    pub updated_at: Option<String>,
    pub entry_count: usize,
    pub file_count: usize,
    pub directory_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudSyncTaskDto {
    pub id: i64,
    pub task_kind: String,
    pub local_relative_path: String,
    pub remote_path: String,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub next_attempt_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDeletionNoticeDto {
    pub id: i64,
    pub relative_path: String,
    pub detected_at: String,
    pub local_version: String,
}

#[derive(Serialize)]
pub struct DiscoveredDevice {
    pub device_id: String,
    pub device_name: String,
    pub ip: String,
    pub platform: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpaceInvitePayload {
    version: u8,
    account_id: String,
    account_secret: String,
    space_name: String,
    sync_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaiduApiConfigDto {
    pub configured: bool,
    pub provider: String,
    pub device_id: Option<String>,
    pub client_id: String,
    pub has_client_secret: bool,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub source: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBaiduApiConfigRequest {
    pub device_id: Option<String>,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: Option<String>,
    pub scopes: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaiduOAuthStartResult {
    pub authorization_url: String,
    pub state: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaiduAccountStatusDto {
    pub connected: bool,
    pub provider: String,
    pub account_id: Option<String>,
    pub display_name: Option<String>,
    pub expires_at: Option<String>,
    pub scopes: Vec<String>,
    pub reconnect_required: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaiduOAuthCompleteResult {
    pub success: bool,
    pub status: BaiduAccountStatusDto,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaiduOAuthCompleteRequest {
    pub code: String,
    pub state: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaiduImplicitOAuthCompleteRequest {
    pub access_token: String,
    pub expires_in: Option<i64>,
    pub scope: Option<String>,
    pub state: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudSyncTaskActionRequest {
    pub space_id: String,
    pub task_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDeletionActionRequest {
    pub space_id: String,
    pub notice_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindBaiduSpaceRequest {
    pub space_id: String,
    pub remote_root_path: Option<String>,
}

#[tauri::command]
pub async fn get_baidu_api_config(
    state: State<'_, TauriState>,
) -> Result<BaiduApiConfigDto, String> {
    let storage = state.storage.lock().await;
    load_baidu_api_config_dto(&storage).await
}

#[tauri::command]
pub async fn save_baidu_api_config(
    request: SaveBaiduApiConfigRequest,
    state: State<'_, TauriState>,
) -> Result<BaiduApiConfigDto, String> {
    let client_id = request.client_id.trim().to_string();
    if client_id.is_empty() {
        return Err("Baidu API Key / Client ID is required".to_string());
    }
    let device_id = request
        .device_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let redirect_uri = request
        .redirect_uri
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_BAIDU_REDIRECT_URI)
        .to_string();
    let scopes = sanitize_baidu_scopes(request.scopes).unwrap_or_else(default_baidu_scopes);
    let now = Utc::now();
    let config = CloudApiConfig {
        provider: BAIDU_PROVIDER.to_string(),
        device_id,
        client_id,
        client_secret: request
            .client_secret
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        redirect_uri,
        scopes,
        created_at: now,
        updated_at: now,
    };

    let storage = state.storage.lock().await;
    storage
        .save_cloud_api_config(&config)
        .await
        .map_err(|e| format!("Failed to save Baidu API config: {e}"))?;
    Ok(map_baidu_api_config(Some(config), "local"))
}

#[tauri::command]
pub async fn clear_baidu_api_config(
    state: State<'_, TauriState>,
) -> Result<BaiduApiConfigDto, String> {
    let storage = state.storage.lock().await;
    storage
        .remove_cloud_api_config(BAIDU_PROVIDER)
        .await
        .map_err(|e| format!("Failed to clear Baidu API config: {e}"))?;
    load_baidu_api_config_dto(&storage).await
}

#[tauri::command]
pub async fn get_baidu_account_status(
    state: State<'_, TauriState>,
) -> Result<BaiduAccountStatusDto, String> {
    let storage = state.storage.lock().await;
    let account = storage
        .get_cloud_account(BAIDU_PROVIDER)
        .await
        .map_err(|e| format!("Failed to load Baidu account: {e}"))?;
    Ok(map_baidu_account_status(account))
}

#[tauri::command]
pub async fn start_baidu_oauth(
    state: State<'_, TauriState>,
) -> Result<BaiduOAuthStartResult, String> {
    let storage = state.storage.lock().await;
    let config = load_baidu_oauth_config(&storage).await?;
    if config
        .device_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        return Err("请在百度网盘配置中填写 AppID / Device ID。".to_string());
    }
    let state = Uuid::new_v4().to_string();
    Ok(BaiduOAuthStartResult {
        authorization_url: config.implicit_authorization_url(&state),
        state,
        redirect_uri: config.redirect_uri,
        scopes: config.scopes,
    })
}

#[tauri::command]
pub async fn open_url(url: String) -> Result<bool, String> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://127.0.0.1")) {
        return Err("Only https URLs and local callback URLs can be opened".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Start-Process -LiteralPath $args[0]",
                url,
            ])
            .spawn()
            .map_err(|e| format!("Failed to open URL: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {e}"))?;
    }

    Ok(true)
}

#[tauri::command]
pub async fn complete_baidu_oauth(
    request: BaiduOAuthCompleteRequest,
    state: State<'_, TauriState>,
) -> Result<BaiduOAuthCompleteResult, String> {
    let storage = state.storage.lock().await;
    let config = load_baidu_oauth_config(&storage).await?;
    drop(storage);
    if request.code.trim().is_empty() {
        return Err("Baidu OAuth code is required".to_string());
    }

    let token = exchange_baidu_oauth_code(&config, &request.code).await?;
    let now = Utc::now();
    let expires_at = token
        .expires_in
        .filter(|value| *value > 0)
        .map(|seconds| now + Duration::seconds(seconds));
    let scopes = parse_scope_string(token.scope.as_deref(), &config.scopes);
    let account_id = request
        .state
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("baidu:{value}"));
    let account = CloudAccount {
        provider: BAIDU_PROVIDER.to_string(),
        account_id,
        display_name: None,
        access_token_encrypted: encrypt_baidu_token_for_storage(
            &token.access_token,
            &config.client_id,
        )
        .map_err(|e| format!("Failed to encrypt Baidu access token: {e}"))?,
        refresh_token_encrypted: encrypt_baidu_token_for_storage(
            &token.refresh_token,
            &config.client_id,
        )
        .map_err(|e| format!("Failed to encrypt Baidu refresh token: {e}"))?,
        expires_at,
        scopes,
        created_at: now,
        updated_at: now,
    };

    let storage = state.storage.lock().await;
    storage
        .save_cloud_account(&account)
        .await
        .map_err(|e| format!("Failed to save Baidu account: {e}"))?;

    Ok(BaiduOAuthCompleteResult {
        success: true,
        status: map_baidu_account_status(Some(account)),
    })
}

#[tauri::command]
pub async fn complete_baidu_implicit_oauth(
    request: BaiduImplicitOAuthCompleteRequest,
    state: State<'_, TauriState>,
) -> Result<BaiduOAuthCompleteResult, String> {
    let storage = state.storage.lock().await;
    let config = load_baidu_oauth_config(&storage).await?;
    drop(storage);

    let access_token = request.access_token.trim();
    if access_token.is_empty() {
        return Err("Baidu OAuth access_token is required".to_string());
    }

    let now = Utc::now();
    let expires_at = Some(
        request
            .expires_in
            .filter(|value| *value > 0)
            .map(|seconds| now + Duration::seconds(seconds))
            .unwrap_or_else(|| now + Duration::days(30)),
    );
    let scopes = parse_scope_string(request.scope.as_deref(), &config.scopes);
    let account_id = request
        .state
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("baidu:{value}"));
    let account = CloudAccount {
        provider: BAIDU_PROVIDER.to_string(),
        account_id,
        display_name: None,
        access_token_encrypted: encrypt_baidu_token_for_storage(access_token, &config.client_id)
            .map_err(|e| format!("Failed to encrypt Baidu access token: {e}"))?,
        refresh_token_encrypted: Vec::new(),
        expires_at,
        scopes,
        created_at: now,
        updated_at: now,
    };

    let storage = state.storage.lock().await;
    storage
        .save_cloud_account(&account)
        .await
        .map_err(|e| format!("Failed to save Baidu account: {e}"))?;

    Ok(BaiduOAuthCompleteResult {
        success: true,
        status: map_baidu_account_status(Some(account)),
    })
}

#[tauri::command]
pub async fn disconnect_baidu_account(state: State<'_, TauriState>) -> Result<bool, String> {
    let storage = state.storage.lock().await;
    storage
        .remove_cloud_account(BAIDU_PROVIDER)
        .await
        .map_err(|e| format!("Failed to disconnect Baidu account: {e}"))
}

#[tauri::command]
pub async fn login(
    username: String,
    password: String,
    state: State<'_, TauriState>,
) -> Result<AuthResult, String> {
    let storage = state.storage.lock().await;
    let (account, account_secret) = ensure_local_account(&storage, &username, &password).await?;
    drop(storage);

    state.session_sync_context.lock().await.initialize(
        account.account_id,
        account_secret,
        account_secret,
        state.device_name.clone(),
    );

    tracing::info!(
        "Login successful for account {} on device {} ({})",
        account.account_id,
        state.device_name,
        state.device_id
    );

    let storage = state.storage.lock().await;
    let spaces = storage
        .get_synced_spaces()
        .await
        .map_err(|e| format!("Failed to load synced spaces: {e}"))?;
    drop(storage);

    for space in spaces {
        if let Err(error) = state.runtime_manager.start_space(space.id).await {
            tracing::warn!(
                "failed to auto-start space after login {}: {}",
                space.id,
                error
            );
        }
    }

    Ok(AuthResult {
        success: true,
        error: None,
        account_id: account.account_id.to_string(),
        device_id: state.device_id.to_string(),
        device_name: state.device_name.clone(),
    })
}

#[tauri::command]
pub async fn pick_folder() -> Result<Option<String>, String> {
    let task = rfd::AsyncFileDialog::new()
        .set_title("Select folder to sync")
        .pick_folder();

    if let Some(handle) = task.await {
        Ok(Some(handle.path().to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn get_synced_folders(
    state: State<'_, TauriState>,
) -> Result<Vec<SyncedSpaceDto>, String> {
    let storage = state.storage.lock().await;
    let spaces = storage
        .get_synced_spaces()
        .await
        .map_err(|e| format!("Failed to load synced spaces: {e}"))?;

    let mut result = Vec::new();
    for space in spaces {
        result.push(map_space_dto_with_storage(&storage, space).await?);
    }
    Ok(result)
}

#[tauri::command]
pub async fn add_synced_folder(
    path: String,
    sync_key: Option<String>,
    state: State<'_, TauriState>,
) -> Result<SyncedSpaceDto, String> {
    let canonical_path =
        std::fs::canonicalize(&path).map_err(|e| format!("Path is not accessible: {e}"))?;
    let meta = tokio::fs::metadata(&canonical_path)
        .await
        .map_err(|e| format!("Path is not accessible: {e}"))?;
    if !meta.is_dir() {
        return Err("Path is not a directory".to_string());
    }

    let root_path = canonical_path.to_string_lossy().to_string();
    let name = canonical_path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| root_path.clone());

    let space = syncflow_core::storage::SyncedSpace {
        id: Uuid::new_v4(),
        sync_key: sync_key
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
        name,
        root_path,
        status: "Monitoring".to_string(),
        created_at: chrono::Utc::now(),
        last_scanned_at: None,
    };

    let storage = state.storage.lock().await;
    let created = storage
        .add_synced_space(&space)
        .await
        .map_err(|e| format!("Failed to add synced space: {e}"))?;

    drop(storage);

    if state.session_sync_context.lock().await.root_key().is_some() {
        if let Err(error) = state.runtime_manager.start_space(created.id).await {
            tracing::warn!("failed to auto-start space {}: {}", created.id, error);
        }
    }

    let storage = state.storage.lock().await;
    map_space_dto_with_storage(&storage, created).await
}

#[tauri::command]
pub async fn bind_baidu_space(
    request: BindBaiduSpaceRequest,
    state: State<'_, TauriState>,
) -> Result<SyncedSpaceDto, String> {
    let space_id = parse_space_id(&request.space_id)?;
    let storage = state.storage.lock().await;
    if storage
        .get_cloud_account(BAIDU_PROVIDER)
        .await
        .map_err(|e| format!("Failed to load Baidu account: {e}"))?
        .is_none()
    {
        return Err("Connect a Baidu Netdisk account before binding a space".to_string());
    }
    let space = storage
        .get_synced_space(&space_id)
        .await
        .map_err(|e| format!("Failed to load synced space: {e}"))?
        .ok_or_else(|| "Synced space not found".to_string())?;
    let now = Utc::now();
    let binding = CloudSpaceBinding {
        space_id,
        provider: BAIDU_PROVIDER.to_string(),
        remote_root_path: request
            .remote_root_path
            .as_deref()
            .map(normalize_baidu_app_path)
            .transpose()?
            .unwrap_or_else(|| default_baidu_remote_root(&space.name, &space.id)),
        remote_root_id: None,
        sync_mode: "bidirectional".to_string(),
        plaintext: true,
        created_at: now,
        updated_at: now,
    };
    storage
        .save_cloud_space_binding(&binding)
        .await
        .map_err(|e| format!("Failed to bind Baidu space: {e}"))?;

    map_space_dto_with_storage(&storage, space).await
}

#[tauri::command]
pub async fn create_baidu_synced_space(
    path: String,
    remote_root_path: Option<String>,
    state: State<'_, TauriState>,
) -> Result<SyncedSpaceDto, String> {
    let space = add_synced_folder(path, None, state.clone()).await?;
    bind_baidu_space(
        BindBaiduSpaceRequest {
            space_id: space.id,
            remote_root_path,
        },
        state,
    )
    .await
}

#[tauri::command]
pub async fn remove_synced_folder(
    space_id: String,
    state: State<'_, TauriState>,
) -> Result<bool, String> {
    let parsed = parse_space_id(&space_id)?;
    let _ = state.runtime_manager.stop_space(parsed).await;
    let storage = state.storage.lock().await;
    storage
        .remove_synced_space(&parsed)
        .await
        .map_err(|e| format!("Failed to remove synced space: {e}"))
}

#[tauri::command]
pub async fn get_tree_children(
    space_id: String,
    parent_relative_path: Option<String>,
    state: State<'_, TauriState>,
) -> Result<Vec<TreeNode>, String> {
    let (_space, parent_path) =
        resolve_space_path(&state, &space_id, parent_relative_path.as_deref()).await?;
    let root = resolve_space_path(&state, &space_id, None).await?.1;

    let entries =
        std::fs::read_dir(&parent_path).map_err(|e| format!("Failed to read directory: {e}"))?;

    let mut nodes = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to read file metadata: {e}"))?;
        let is_dir = metadata.is_dir();
        let relative_path = strip_root_prefix(&root, &path)?;
        let modified_at = metadata.modified().ok().map(format_system_time);

        nodes.push(TreeNode {
            name: entry.file_name().to_string_lossy().to_string(),
            relative_path,
            node_type: if is_dir {
                "directory".to_string()
            } else {
                "file".to_string()
            },
            has_children: if is_dir {
                directory_has_children(&path)
            } else {
                false
            },
            extension: path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_lowercase()),
            size: if is_dir { None } else { Some(metadata.len()) },
            modified_at,
        });
    }

    nodes.sort_by(
        |left, right| match (left.node_type.as_str(), right.node_type.as_str()) {
            ("directory", "file") => std::cmp::Ordering::Less,
            ("file", "directory") => std::cmp::Ordering::Greater,
            _ => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
        },
    );

    Ok(nodes)
}

#[tauri::command]
pub async fn get_file_details(
    space_id: String,
    relative_path: String,
    state: State<'_, TauriState>,
) -> Result<FileDetails, String> {
    let (space, resolved_path) =
        resolve_space_path(&state, &space_id, Some(&relative_path)).await?;
    let metadata = std::fs::metadata(&resolved_path)
        .map_err(|e| format!("Failed to read file details: {e}"))?;

    Ok(FileDetails {
        name: resolved_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_string(),
        node_type: if metadata.is_dir() {
            "directory".to_string()
        } else {
            "file".to_string()
        },
        extension: resolved_path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_lowercase()),
        size: if metadata.is_dir() { 0 } else { metadata.len() },
        modified_at: metadata.modified().ok().map(format_system_time),
        space_name: space.name,
        relative_path,
    })
}

#[tauri::command]
pub async fn create_tree_file(
    request: CreateTreeItemRequest,
    state: State<'_, TauriState>,
) -> Result<TreeNode, String> {
    let (space, root) = resolve_space_path(&state, &request.space_id, None).await?;
    let (relative_path, target) = resolve_new_child_path(
        &root,
        request.parent_relative_path.as_deref(),
        &request.name,
    )?;
    if target.exists() {
        return Err("同名文件或文件夹已存在".to_string());
    }
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("创建父目录失败: {e}"))?;
    }
    tokio::fs::write(&target, [])
        .await
        .map_err(|e| format!("创建文件失败: {e}"))?;

    let metadata = tokio::fs::metadata(&target)
        .await
        .map_err(|e| format!("读取新文件信息失败: {e}"))?;
    let hash = syncflow_core::crypto::hash_data(&[]);
    let version_vector = syncflow_core::sync::VersionVector::new("local_editor")
        .to_json()
        .map_err(|e| format!("编码文件版本失败: {e}"))?;
    let storage = state.storage.lock().await;
    storage
        .save_file_meta(&syncflow_core::storage::FileMetadata {
            space_id: space.id,
            relative_path: relative_path.clone(),
            hash,
            size: metadata.len(),
            modified_at: chrono::Utc::now(),
            version_vector,
            created_at: chrono::Utc::now(),
        })
        .await
        .map_err(|e| format!("保存文件元数据失败: {e}"))?;
    drop(storage);
    state.runtime_manager.refresh_space_counts(space.id).await;

    tree_node_from_path(&root, &target)
}

#[tauri::command]
pub async fn create_tree_folder(
    request: CreateTreeItemRequest,
    state: State<'_, TauriState>,
) -> Result<TreeNode, String> {
    let (space, root) = resolve_space_path(&state, &request.space_id, None).await?;
    let (_relative_path, target) = resolve_new_child_path(
        &root,
        request.parent_relative_path.as_deref(),
        &request.name,
    )?;
    if target.exists() {
        return Err("同名文件或文件夹已存在".to_string());
    }
    tokio::fs::create_dir(&target)
        .await
        .map_err(|e| format!("创建文件夹失败: {e}"))?;
    state.runtime_manager.refresh_space_counts(space.id).await;
    tree_node_from_path(&root, &target)
}

#[tauri::command]
pub async fn preview_file_text(
    space_id: String,
    relative_path: String,
    max_bytes: Option<usize>,
    state: State<'_, TauriState>,
) -> Result<TextPreviewResult, String> {
    let (_, resolved_path) = resolve_space_path(&state, &space_id, Some(&relative_path)).await?;
    let metadata = std::fs::metadata(&resolved_path)
        .map_err(|e| format!("Failed to read file metadata: {e}"))?;
    if metadata.is_dir() {
        return Err("Directory text preview is not supported".to_string());
    }

    let max_bytes = max_bytes.unwrap_or(100_000);
    let bytes = tokio::fs::read(&resolved_path)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;
    let truncated = bytes.len() > max_bytes;
    let content_bytes = if truncated {
        &bytes[..max_bytes]
    } else {
        &bytes[..]
    };
    let content = String::from_utf8_lossy(content_bytes).to_string();

    Ok(TextPreviewResult {
        content,
        truncated,
        size: metadata.len(),
        max_bytes,
    })
}

#[tauri::command]
pub async fn save_text_file(
    request: SaveTextFileRequest,
    state: State<'_, TauriState>,
) -> Result<SaveTextFileResult, String> {
    if request.content.len() > 2 * 1024 * 1024 {
        return Err("Text file is too large to edit in SyncFlow".to_string());
    }
    if !is_text_relative_path(&request.relative_path) {
        return Err("Only text files can be edited".to_string());
    }
    let (space, root) = resolve_space_path(&state, &request.space_id, None).await?;
    let target = safe_join_for_write(&root, &request.relative_path)?;
    let metadata =
        std::fs::metadata(&target).map_err(|e| format!("Failed to read file metadata: {e}"))?;
    if metadata.is_dir() {
        return Err("Directory cannot be edited as text".to_string());
    }
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create parent directories: {e}"))?;
    }
    tokio::fs::write(&target, request.content.as_bytes())
        .await
        .map_err(|e| format!("Failed to save text file: {e}"))?;

    let saved_metadata = tokio::fs::metadata(&target)
        .await
        .map_err(|e| format!("Failed to read saved file metadata: {e}"))?;
    let hash = syncflow_core::crypto::hash_data(request.content.as_bytes());
    let version_vector = syncflow_core::sync::VersionVector::new("local_editor")
        .to_json()
        .map_err(|e| format!("Failed to encode file version: {e}"))?;
    let storage = state.storage.lock().await;
    storage
        .save_file_meta(&syncflow_core::storage::FileMetadata {
            space_id: space.id,
            relative_path: request.relative_path.clone(),
            hash,
            size: saved_metadata.len(),
            modified_at: chrono::Utc::now(),
            version_vector,
            created_at: chrono::Utc::now(),
        })
        .await
        .map_err(|e| format!("Failed to save file metadata: {e}"))?;
    drop(storage);
    state.runtime_manager.refresh_space_counts(space.id).await;

    Ok(SaveTextFileResult {
        details: FileDetails {
            name: target
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_string(),
            node_type: "file".to_string(),
            extension: target
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_lowercase()),
            size: saved_metadata.len(),
            modified_at: saved_metadata.modified().ok().map(format_system_time),
            space_name: space.name,
            relative_path: request.relative_path,
        },
    })
}

#[tauri::command]
pub async fn preview_file_image(
    space_id: String,
    relative_path: String,
    max_bytes: Option<usize>,
    state: State<'_, TauriState>,
) -> Result<ImagePreviewResult, String> {
    let (_, resolved_path) = resolve_space_path(&state, &space_id, Some(&relative_path)).await?;
    let metadata = std::fs::metadata(&resolved_path)
        .map_err(|e| format!("Failed to read file metadata: {e}"))?;
    if metadata.is_dir() {
        return Err("Directory image preview is not supported".to_string());
    }

    let mime_type = detect_image_mime(&resolved_path)?;
    let max_bytes = max_bytes.unwrap_or(5 * 1024 * 1024).min(5 * 1024 * 1024);
    let size = metadata.len() as usize;
    if size > max_bytes {
        return Err("Image file is too large to preview".to_string());
    }

    let bytes = tokio::fs::read(&resolved_path)
        .await
        .map_err(|e| format!("Failed to read image: {e}"))?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);

    Ok(ImagePreviewResult {
        data_url: format!("data:{mime_type};base64,{encoded}"),
        mime_type: mime_type.to_string(),
        size: metadata.len(),
        truncated: false,
    })
}

#[tauri::command]
pub async fn open_file(
    space_id: String,
    relative_path: String,
    state: State<'_, TauriState>,
) -> Result<bool, String> {
    let (_, resolved_path) = resolve_space_path(&state, &space_id, Some(&relative_path)).await?;
    let file_path = resolved_path.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", &file_path])
            .spawn()
            .map_err(|e| format!("Failed to open file: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {e}"))?;
    }
    Ok(true)
}

#[tauri::command]
pub async fn get_device_info(state: State<'_, TauriState>) -> Result<Vec<DeviceStateDto>, String> {
    state
        .runtime_manager
        .aggregate_devices(&state.device_id)
        .await
}

#[tauri::command]
pub async fn get_conflicts(
    space_id: Option<String>,
    state: State<'_, TauriState>,
) -> Result<Vec<ConflictInfo>, String> {
    let storage = state.storage.lock().await;
    let conflicts = match space_id {
        Some(value) => {
            let parsed = parse_space_id(&value)?;
            storage
                .get_conflicts_for_space(&parsed)
                .await
                .map_err(|e| format!("Failed to load conflicts: {e}"))?
        }
        None => storage
            .get_all_conflicts()
            .await
            .map_err(|e| format!("Failed to load conflicts: {e}"))?,
    };

    let devices = storage
        .get_known_devices()
        .await
        .map_err(|e| format!("Failed to load device list: {e}"))?;

    Ok(conflicts
        .into_iter()
        .filter(|conflict| conflict.remote_device_id == BAIDU_PROVIDER)
        .map(|conflict| {
            let remote_device = devices
                .iter()
                .find(|device| device.device_id.to_string() == conflict.remote_device_id)
                .map(|device| device.device_name.clone())
                .unwrap_or_else(|| conflict.remote_device_id.clone());
            ConflictInfo {
                id: conflict.id,
                space_id: conflict.space_id.to_string(),
                relative_path: conflict.relative_path,
                local_version: conflict.local_version,
                remote_version: conflict.remote_version,
                remote_device,
                detected_at: conflict.detected_at.to_rfc3339(),
            }
        })
        .collect())
}

#[tauri::command]
pub async fn get_conflict_detail(
    conflict_id: i64,
    state: State<'_, TauriState>,
) -> Result<ConflictDetail, String> {
    let storage = state.storage.lock().await;
    let conflict = storage
        .get_conflict_by_id(conflict_id)
        .await
        .map_err(|e| format!("Failed to load conflict: {e}"))?
        .ok_or_else(|| "Conflict not found".to_string())?;
    let space = storage
        .get_synced_space(&conflict.space_id)
        .await
        .map_err(|e| format!("Failed to load synced space: {e}"))?
        .ok_or_else(|| "Synced space not found".to_string())?;
    let devices = storage
        .get_known_devices()
        .await
        .map_err(|e| format!("Failed to load known devices: {e}"))?;
    let snapshots = storage
        .get_conflict_snapshots(conflict_id)
        .await
        .map_err(|e| format!("Failed to load conflict snapshots: {e}"))?;
    drop(storage);

    let remote_device = devices
        .iter()
        .find(|device| device.device_id.to_string() == conflict.remote_device_id)
        .map(|device| device.device_name.clone())
        .unwrap_or_else(|| conflict.remote_device_id.clone());

    let root = PathBuf::from(&space.root_path);
    let root_canonical = std::fs::canonicalize(&root)
        .map_err(|e| format!("Failed to resolve space root path: {e}"))?;
    let target = safe_join_for_write(&root_canonical, &conflict.relative_path)?;
    let local_file_exists = target.exists() && target.is_file();
    let is_text = is_text_relative_path(&conflict.relative_path);
    let (local_text_content, local_text_truncated) = if is_text && local_file_exists {
        let metadata = std::fs::metadata(&target)
            .map_err(|e| format!("Failed to read local file metadata: {e}"))?;
        let max_bytes = 100_000usize;
        let bytes = tokio::fs::read(&target)
            .await
            .map_err(|e| format!("Failed to read local file: {e}"))?;
        let truncated = metadata.len() as usize > max_bytes;
        let content_bytes = if truncated {
            &bytes[..max_bytes]
        } else {
            &bytes[..]
        };
        (
            Some(String::from_utf8_lossy(content_bytes).to_string()),
            Some(truncated),
        )
    } else {
        (None, None)
    };

    let remote_snapshot = snapshots
        .iter()
        .find(|snapshot| snapshot.snapshot_kind == "remote_text");
    let remote_text_content = remote_snapshot.and_then(|snapshot| snapshot.content_text.clone());
    let remote_text_truncated = remote_snapshot.map(|snapshot| snapshot.content_truncated);
    let can_keep_remote = remote_text_content.is_some();
    let can_compare_text = is_text && local_text_content.is_some() && remote_text_content.is_some();
    let missing_remote_snapshot_reason = if is_text && remote_text_content.is_none() {
        Some("This conflict was created before remote text snapshots were stored.".to_string())
    } else if !is_text {
        Some("Remote compare is only available for supported text files.".to_string())
    } else {
        None
    };

    Ok(ConflictDetail {
        id: conflict.id,
        space_id: conflict.space_id.to_string(),
        space_name: space.name,
        relative_path: conflict.relative_path,
        remote_device,
        detected_at: conflict.detected_at.to_rfc3339(),
        local_version: conflict.local_version,
        remote_version: conflict.remote_version,
        local_file_exists,
        is_text,
        local_text_content,
        local_text_truncated,
        remote_text_content,
        remote_text_truncated,
        can_keep_local: true,
        can_keep_remote,
        can_compare_text,
        missing_remote_snapshot_reason,
    })
}

#[tauri::command]
pub async fn resolve_conflict_keep_local(
    conflict_id: i64,
    state: State<'_, TauriState>,
) -> Result<bool, String> {
    state
        .runtime_manager
        .resolve_cloud_conflict_keep_local(conflict_id)
        .await
}

#[tauri::command]
pub async fn dismiss_conflict(
    conflict_id: i64,
    state: State<'_, TauriState>,
) -> Result<bool, String> {
    let storage = state.storage.lock().await;
    let removed = storage
        .remove_conflict(conflict_id)
        .await
        .map_err(|e| format!("Failed to dismiss conflict: {e}"))?;
    if !removed {
        return Err("Conflict not found".to_string());
    }
    Ok(true)
}

#[tauri::command]
pub async fn resolve_conflict_keep_remote(
    conflict_id: i64,
    state: State<'_, TauriState>,
) -> Result<bool, String> {
    state
        .runtime_manager
        .resolve_cloud_conflict_keep_remote(conflict_id)
        .await
}

#[tauri::command]
pub async fn start_sync(
    _password: String,
    device_name: String,
    state: State<'_, TauriState>,
) -> Result<bool, String> {
    if state.session_sync_context.lock().await.root_key().is_none() {
        return Err("Please log in and unlock the local account first".to_string());
    }

    if !device_name.is_empty() {
        tracing::info!("Sync session initialized for device: {}", device_name);
    }
    let storage = state.storage.lock().await;
    let spaces = storage
        .get_synced_spaces()
        .await
        .map_err(|e| format!("Failed to load synced spaces: {e}"))?;
    drop(storage);

    for space in spaces {
        if let Err(error) = state.runtime_manager.start_space(space.id).await {
            tracing::warn!("failed to auto-start space {}: {}", space.id, error);
        }
    }
    Ok(true)
}

#[tauri::command]
pub async fn stop_sync(state: State<'_, TauriState>) -> Result<bool, String> {
    state.runtime_manager.stop_all().await;
    state.session_sync_context.lock().await.clear();
    tracing::info!("Sync session stopped");
    Ok(true)
}

#[tauri::command]
pub async fn start_space_sync(
    space_id: String,
    state: State<'_, TauriState>,
) -> Result<SyncRuntimeStatusDto, String> {
    let parsed = parse_space_id(&space_id)?;
    state.runtime_manager.start_space(parsed).await
}

#[tauri::command]
pub async fn stop_space_sync(
    space_id: String,
    state: State<'_, TauriState>,
) -> Result<SyncRuntimeStatusDto, String> {
    let parsed = parse_space_id(&space_id)?;
    state.runtime_manager.stop_space(parsed).await
}

#[tauri::command]
pub async fn get_sync_status(
    space_id: String,
    state: State<'_, TauriState>,
) -> Result<SyncRuntimeStatusDto, String> {
    let parsed = parse_space_id(&space_id)?;
    state.runtime_manager.get_status(parsed).await
}

#[tauri::command]
pub async fn get_all_sync_statuses(
    state: State<'_, TauriState>,
) -> Result<Vec<SyncRuntimeStatusDto>, String> {
    state.runtime_manager.get_all_statuses().await
}

#[tauri::command]
pub async fn get_sync_diagnostics(
    space_id: String,
    state: State<'_, TauriState>,
) -> Result<SyncDiagnosticsDto, String> {
    let parsed = parse_space_id(&space_id)?;
    let storage = state.storage.lock().await;
    let space = storage
        .get_synced_space(&parsed)
        .await
        .map_err(|e| format!("Failed to load synced space: {e}"))?
        .ok_or_else(|| "Synced space not found".to_string())?;
    let binding = storage
        .get_cloud_space_binding(&parsed, BAIDU_PROVIDER)
        .await
        .map_err(|e| format!("Failed to load cloud binding: {e}"))?;
    let queue = storage
        .list_cloud_sync_tasks_for_space(&parsed, BAIDU_PROVIDER, 50)
        .await
        .map_err(|e| format!("Failed to load cloud sync queue: {e}"))?;
    let cloud_metadata = storage
        .list_remote_file_metadata(&parsed, BAIDU_PROVIDER)
        .await
        .map_err(|e| format!("Failed to load cloud sync metadata: {e}"))?;
    let conflicts = storage
        .get_conflicts_for_space(&parsed)
        .await
        .map_err(|e| format!("Failed to load conflicts: {e}"))?;
    let devices = storage
        .get_known_devices()
        .await
        .map_err(|e| format!("Failed to load devices: {e}"))?;
    drop(storage);

    let root = std::fs::canonicalize(&space.root_path)
        .map_err(|e| format!("Sync space root is not accessible: {e}"))?;
    let manifest_path = root.join(".syncflow").join("manifest.json");
    let manifest = read_sync_manifest_summary(&manifest_path).await?;
    let summary = build_sync_summary(&space, &queue, &cloud_metadata, manifest.as_ref());
    let mut conflict_dtos = Vec::new();
    let mut remote_deletions = Vec::new();
    for conflict in conflicts {
        if conflict.remote_device_id == CLOUD_REMOTE_DELETED_DEVICE_ID {
            remote_deletions.push(RemoteDeletionNoticeDto {
                id: conflict.id,
                relative_path: conflict.relative_path,
                detected_at: conflict.detected_at.to_rfc3339(),
                local_version: conflict.local_version,
            });
        } else {
            let remote_device = devices
                .iter()
                .find(|device| device.device_id.to_string() == conflict.remote_device_id)
                .map(|device| device.device_name.clone())
                .unwrap_or_else(|| conflict.remote_device_id.clone());
            conflict_dtos.push(ConflictInfo {
                id: conflict.id,
                space_id: conflict.space_id.to_string(),
                relative_path: conflict.relative_path,
                local_version: conflict.local_version,
                remote_version: conflict.remote_version,
                remote_device,
                detected_at: conflict.detected_at.to_rfc3339(),
            });
        }
    }

    Ok(SyncDiagnosticsDto {
        space_id,
        space_name: space.name,
        root_path: space.root_path,
        cloud_provider: binding.as_ref().map(|value| value.provider.clone()),
        cloud_remote_path: binding.as_ref().map(|value| value.remote_root_path.clone()),
        summary,
        manifest,
        queue: queue.into_iter().map(map_cloud_sync_task_dto).collect(),
        conflicts: conflict_dtos,
        remote_deletions,
        safety_notes: vec![
            "云端删除不会自动删除本地文件。".to_string(),
            "同步基线保存在本地 .syncflow/manifest.json，并会同步一份到云端 .syncflow/manifest.json。".to_string(),
        ],
    })
}

#[tauri::command]
pub async fn retry_cloud_sync_task(
    request: CloudSyncTaskActionRequest,
    state: State<'_, TauriState>,
) -> Result<bool, String> {
    let parsed = parse_space_id(&request.space_id)?;
    let storage = state.storage.lock().await;
    let updated = storage
        .retry_cloud_sync_task(request.task_id, &parsed, BAIDU_PROVIDER, Utc::now())
        .await
        .map_err(|e| format!("Failed to retry cloud sync task: {e}"))?;
    Ok(updated)
}

#[tauri::command]
pub async fn ignore_cloud_sync_task(
    request: CloudSyncTaskActionRequest,
    state: State<'_, TauriState>,
) -> Result<bool, String> {
    let parsed = parse_space_id(&request.space_id)?;
    let storage = state.storage.lock().await;
    let removed = storage
        .remove_cloud_sync_task_for_space(request.task_id, &parsed, BAIDU_PROVIDER)
        .await
        .map_err(|e| format!("Failed to ignore cloud sync task: {e}"))?;
    Ok(removed)
}

#[tauri::command]
pub async fn restore_remote_deleted_file(
    request: RemoteDeletionActionRequest,
    state: State<'_, TauriState>,
) -> Result<bool, String> {
    let parsed = parse_space_id(&request.space_id)?;
    let storage = state.storage.lock().await;
    let conflict = storage
        .get_conflict_by_id(request.notice_id)
        .await
        .map_err(|e| format!("Failed to load remote deletion notice: {e}"))?
        .ok_or_else(|| "Remote deletion notice not found".to_string())?;
    if conflict.space_id != parsed || conflict.remote_device_id != CLOUD_REMOTE_DELETED_DEVICE_ID {
        return Err("Remote deletion notice does not belong to this space".to_string());
    }
    let binding = storage
        .get_cloud_space_binding(&parsed, BAIDU_PROVIDER)
        .await
        .map_err(|e| format!("Failed to load cloud binding: {e}"))?
        .ok_or_else(|| "This space is not bound to Baidu Netdisk".to_string())?;
    let remote_path = format!(
        "{}/{}",
        binding.remote_root_path.trim_end_matches('/'),
        conflict.relative_path.trim_start_matches('/')
    );
    let now = Utc::now();
    let task = syncflow_core::storage::CloudSyncTask {
        id: 0,
        space_id: parsed,
        provider: BAIDU_PROVIDER.to_string(),
        task_kind: "upload".to_string(),
        local_relative_path: conflict.relative_path.clone(),
        remote_path,
        expected_remote_revision: None,
        payload_json: Some("{\"reason\":\"restore_remote_deleted\"}".to_string()),
        attempts: 0,
        last_error: None,
        created_at: now,
        updated_at: now,
        next_attempt_at: Some(now),
    };
    storage
        .enqueue_cloud_sync_task(&task)
        .await
        .map_err(|e| format!("Failed to enqueue restore upload: {e}"))?;
    storage
        .remove_conflict(request.notice_id)
        .await
        .map_err(|e| format!("Failed to clear remote deletion notice: {e}"))?;
    Ok(true)
}

#[tauri::command]
pub async fn dismiss_remote_deleted_notice(
    request: RemoteDeletionActionRequest,
    state: State<'_, TauriState>,
) -> Result<bool, String> {
    let parsed = parse_space_id(&request.space_id)?;
    let storage = state.storage.lock().await;
    let conflict = storage
        .get_conflict_by_id(request.notice_id)
        .await
        .map_err(|e| format!("Failed to load remote deletion notice: {e}"))?
        .ok_or_else(|| "Remote deletion notice not found".to_string())?;
    if conflict.space_id != parsed || conflict.remote_device_id != CLOUD_REMOTE_DELETED_DEVICE_ID {
        return Err("Remote deletion notice does not belong to this space".to_string());
    }
    storage
        .remove_conflict(request.notice_id)
        .await
        .map_err(|e| format!("Failed to dismiss remote deletion notice: {e}"))
}

#[tauri::command]
pub async fn get_discovered_devices(
    state: State<'_, TauriState>,
) -> Result<Vec<DiscoveredDevice>, String> {
    let devices = state.transport.get_discovered_devices().await;
    Ok(devices
        .into_iter()
        .map(|d| DiscoveredDevice {
            device_id: d.device_id,
            device_name: d.device_name,
            ip: d.ip,
            platform: d.platform,
        })
        .collect())
}

#[tauri::command]
pub async fn export_space_invite(
    space_id: String,
    state: State<'_, TauriState>,
) -> Result<String, String> {
    let parsed = parse_space_id(&space_id)?;
    let (account_id, account_secret) = {
        let session = state.session_sync_context.lock().await;
        let account_id = session
            .account_id()
            .ok_or_else(|| "Please log in and unlock the local account first".to_string())?;
        let account_secret = session
            .account_secret()
            .ok_or_else(|| "Account secret is not unlocked".to_string())?;
        (account_id, account_secret)
    };
    let storage = state.storage.lock().await;
    let space = storage
        .get_synced_space(&parsed)
        .await
        .map_err(|e| format!("Failed to load synced space: {e}"))?
        .ok_or_else(|| "Synced space not found".to_string())?;

    let payload = SpaceInvitePayload {
        version: 1,
        account_id: account_id.to_string(),
        account_secret: base64::engine::general_purpose::STANDARD.encode(account_secret),
        space_name: space.name,
        sync_key: space.sync_key,
    };
    let json = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json);
    Ok(format!("syncflow1.{encoded}"))
}

#[tauri::command]
pub async fn join_space_from_invite(
    invite_code: String,
    _password: String,
    state: State<'_, TauriState>,
) -> Result<SyncedSpaceDto, String> {
    let payload = decode_space_invite(&invite_code)?;
    let root_path =
        default_joined_space_root(&state.app_data_dir, &payload.space_name, &payload.sync_key)?;
    tokio::fs::create_dir_all(&root_path)
        .await
        .map_err(|e| format!("Failed to create joined space directory: {e}"))?;

    let now = chrono::Utc::now();
    let space = syncflow_core::storage::SyncedSpace {
        id: Uuid::new_v4(),
        sync_key: payload.sync_key,
        name: payload.space_name,
        root_path: root_path.to_string_lossy().to_string(),
        status: "Monitoring".to_string(),
        created_at: now,
        last_scanned_at: None,
    };

    let storage = state.storage.lock().await;
    let created = storage
        .add_synced_space(&space)
        .await
        .map_err(|e| format!("Failed to add joined space: {e}"))?;

    drop(storage);

    if state.session_sync_context.lock().await.root_key().is_some() {
        if let Err(error) = state.runtime_manager.start_space(created.id).await {
            tracing::warn!(
                "failed to auto-start joined space {}: {}",
                created.id,
                error
            );
        }
    }

    let storage = state.storage.lock().await;
    map_space_dto_with_storage(&storage, created).await
}

fn map_space_dto(space: syncflow_core::storage::SyncedSpace) -> SyncedSpaceDto {
    SyncedSpaceDto {
        id: space.id.to_string(),
        sync_key: space.sync_key,
        name: space.name,
        root_path: space.root_path,
        status: space.status,
        created_at: space.created_at.to_rfc3339(),
        last_scanned_at: space.last_scanned_at.map(|value| value.to_rfc3339()),
        cloud_binding: None,
    }
}

async fn map_space_dto_with_storage(
    storage: &syncflow_core::storage::StorageEngine,
    space: syncflow_core::storage::SyncedSpace,
) -> Result<SyncedSpaceDto, String> {
    let binding = storage
        .get_cloud_space_binding(&space.id, BAIDU_PROVIDER)
        .await
        .map_err(|e| format!("Failed to load cloud binding: {e}"))?;
    let mut dto = map_space_dto(space);
    dto.cloud_binding = binding.map(map_cloud_space_binding_dto);
    Ok(dto)
}

fn map_cloud_space_binding_dto(binding: CloudSpaceBinding) -> CloudSpaceBindingDto {
    CloudSpaceBindingDto {
        space_id: binding.space_id.to_string(),
        provider: binding.provider,
        remote_root_path: binding.remote_root_path,
        remote_root_id: binding.remote_root_id,
        sync_mode: binding.sync_mode,
        plaintext: binding.plaintext,
        created_at: binding.created_at.to_rfc3339(),
        updated_at: binding.updated_at.to_rfc3339(),
    }
}

fn map_cloud_sync_task_dto(task: syncflow_core::storage::CloudSyncTask) -> CloudSyncTaskDto {
    CloudSyncTaskDto {
        id: task.id,
        task_kind: task.task_kind,
        local_relative_path: task.local_relative_path,
        remote_path: task.remote_path,
        attempts: task.attempts,
        last_error: task.last_error,
        created_at: task.created_at.to_rfc3339(),
        updated_at: task.updated_at.to_rfc3339(),
        next_attempt_at: task.next_attempt_at.map(|value| value.to_rfc3339()),
    }
}

fn build_sync_summary(
    space: &syncflow_core::storage::SyncedSpace,
    queue: &[syncflow_core::storage::CloudSyncTask],
    cloud_metadata: &[syncflow_core::storage::RemoteFileMetadata],
    manifest: Option<&SyncManifestSummaryDto>,
) -> SyncSummaryDto {
    let last_successful_sync_at = cloud_metadata
        .iter()
        .filter_map(|metadata| metadata.last_synced_at)
        .max();
    let last_queue_activity_at = queue.iter().map(|task| task.updated_at).max();
    let last_manifest_update_at = manifest
        .and_then(|manifest| manifest.updated_at.as_deref())
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc));
    let last_sync_activity_at = [
        last_successful_sync_at,
        last_queue_activity_at,
        space.last_scanned_at,
        last_manifest_update_at,
    ]
    .into_iter()
    .flatten()
    .max();
    let latest_error_task = queue
        .iter()
        .filter(|task| task.last_error.is_some())
        .max_by_key(|task| task.updated_at);

    SyncSummaryDto {
        last_successful_sync_at: last_successful_sync_at.map(|value| value.to_rfc3339()),
        last_sync_activity_at: last_sync_activity_at.map(|value| value.to_rfc3339()),
        last_sync_error_at: latest_error_task.map(|task| task.updated_at.to_rfc3339()),
        last_sync_error: latest_error_task.and_then(|task| task.last_error.clone()),
    }
}

async fn read_sync_manifest_summary(
    manifest_path: &Path,
) -> Result<Option<SyncManifestSummaryDto>, String> {
    let bytes = match tokio::fs::read(manifest_path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Failed to read SyncFlow manifest: {error}")),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Failed to parse SyncFlow manifest: {e}"))?;
    let entries = value
        .get("entries")
        .and_then(|entries| entries.as_array())
        .cloned()
        .unwrap_or_default();
    let directory_count = entries
        .iter()
        .filter(|entry| {
            entry
                .get("isDirectory")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
        })
        .count();
    let entry_count = entries.len();
    Ok(Some(SyncManifestSummaryDto {
        path: manifest_path.to_string_lossy().to_string(),
        version: value
            .get("version")
            .and_then(|value| value.as_u64())
            .unwrap_or(0) as u32,
        manifest_id: value
            .get("manifestId")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        sequence: value
            .get("sequence")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        base_remote_revision: value
            .get("baseRemoteRevision")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        updated_by_device_id: value
            .get("updatedByDeviceId")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        updated_at: value
            .get("updatedAt")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        entry_count,
        file_count: entry_count.saturating_sub(directory_count),
        directory_count,
    }))
}

fn default_baidu_remote_root(space_name: &str, space_id: &Uuid) -> String {
    let safe_name = sanitize_baidu_path_segment(space_name);
    if safe_name.is_empty() {
        format!("/apps/SyncFlow/{space_id}")
    } else {
        format!("/apps/SyncFlow/{safe_name}")
    }
}

fn normalize_baidu_app_path(path: &str) -> Result<String, String> {
    let normalized = path.trim().replace('\\', "/");
    if normalized.is_empty() {
        return Err("Baidu remote path must not be empty".to_string());
    }
    let with_root = if normalized.starts_with('/') {
        normalized
    } else {
        format!("/apps/SyncFlow/{normalized}")
    };
    if !with_root.starts_with("/apps/SyncFlow/") || with_root.contains("/../") {
        return Err("Baidu remote path must stay under /apps/SyncFlow".to_string());
    }
    Ok(with_root.trim_end_matches('/').to_string())
}

fn sanitize_baidu_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            other if other.is_control() => '-',
            other => other,
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string()
}

async fn ensure_local_account(
    storage: &syncflow_core::storage::StorageEngine,
    username: &str,
    password: &str,
) -> Result<(syncflow_core::storage::AccountRecord, [u8; 32]), String> {
    if let Some(account) = storage
        .get_local_account()
        .await
        .map_err(|e| format!("Failed to load local account: {e}"))?
    {
        let key = derive_password_key(password, &account.password_salt)?;
        let account_secret = decrypt_account_secret(&account.encrypted_account_secret, &key)
            .map_err(|_| "Password is incorrect; unable to unlock local account".to_string())?;
        storage
            .update_account_last_unlocked_at(&account.account_id, chrono::Utc::now())
            .await
            .map_err(|e| format!("Failed to update account unlock time: {e}"))?;
        return Ok((
            syncflow_core::storage::AccountRecord {
                last_unlocked_at: Some(chrono::Utc::now()),
                display_name: if username.trim().is_empty() {
                    account.display_name
                } else {
                    username.trim().to_string()
                },
                ..account
            },
            account_secret,
        ));
    }

    let password_salt = Uuid::new_v4().as_bytes().to_vec();
    let password_key = derive_password_key(password, &password_salt)?;
    let account_secret = generate_account_secret();
    let encrypted_account_secret =
        syncflow_core::crypto::encrypt_data(&account_secret, &password_key)
            .map_err(|e| e.to_string())?;
    let account = syncflow_core::storage::AccountRecord {
        account_id: Uuid::new_v4(),
        display_name: if username.trim().is_empty() {
            "Local Account".to_string()
        } else {
            username.trim().to_string()
        },
        password_salt,
        encrypted_account_secret,
        created_at: chrono::Utc::now(),
        last_unlocked_at: Some(chrono::Utc::now()),
    };
    storage
        .save_account(&account)
        .await
        .map_err(|e| format!("Failed to save local account: {e}"))?;
    Ok((account, account_secret))
}

fn decode_space_invite(invite_code: &str) -> Result<SpaceInvitePayload, String> {
    let encoded = invite_code
        .trim()
        .strip_prefix("syncflow1.")
        .ok_or_else(|| "Invite code format is invalid".to_string())?;
    let json = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| "Invite code encoding is invalid".to_string())?;
    let payload: SpaceInvitePayload =
        serde_json::from_slice(&json).map_err(|_| "Invite code payload is invalid".to_string())?;
    if payload.version != 1 {
        return Err("Invite code version is not supported".to_string());
    }
    if payload.space_name.trim().is_empty() || payload.sync_key.trim().is_empty() {
        return Err("Invite code is missing space information".to_string());
    }
    let _ = Uuid::parse_str(&payload.account_id)
        .map_err(|_| "Invite code account ID is invalid".to_string())?;
    let _ = decode_account_secret(&payload.account_secret)?;
    Ok(payload)
}

fn decode_account_secret(encoded: &str) -> Result<[u8; 32], String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "Invite code account secret is invalid".to_string())?;
    if bytes.len() != 32 {
        return Err("Invite code account secret length is invalid".to_string());
    }
    let mut secret = [0u8; 32];
    secret.copy_from_slice(&bytes);
    Ok(secret)
}

fn default_joined_space_root(
    app_data_dir: &Path,
    space_name: &str,
    sync_key: &str,
) -> Result<PathBuf, String> {
    let mut safe_name: String = space_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if safe_name.is_empty() {
        safe_name = "space".to_string();
    }
    let prefix: String = sync_key.chars().take(8).collect();
    Ok(app_data_dir
        .join("joined-spaces")
        .join(format!("{safe_name}-{prefix}")))
}

fn derive_password_key(password: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    syncflow_core::crypto::derive_root_key(password, salt).map_err(|e| e.to_string())
}

fn decrypt_account_secret(ciphertext: &[u8], key: &[u8; 32]) -> Result<[u8; 32], String> {
    let plaintext =
        syncflow_core::crypto::decrypt_data(ciphertext, key).map_err(|e| e.to_string())?;
    if plaintext.len() != 32 {
        return Err("invalid account secret length".to_string());
    }
    let mut secret = [0u8; 32];
    secret.copy_from_slice(&plaintext);
    Ok(secret)
}

fn generate_account_secret() -> [u8; 32] {
    let left = Uuid::new_v4();
    let right = Uuid::new_v4();
    let mut secret = [0u8; 32];
    secret[..16].copy_from_slice(left.as_bytes());
    secret[16..].copy_from_slice(right.as_bytes());
    secret
}

fn directory_has_children(path: &Path) -> bool {
    std::fs::read_dir(path)
        .ok()
        .and_then(|mut entries| entries.next())
        .is_some()
}

fn resolve_new_child_path(
    root: &Path,
    parent_relative_path: Option<&str>,
    name: &str,
) -> Result<(String, PathBuf), String> {
    let name = validate_new_child_name(name)?;
    let parent_relative_path = parent_relative_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    validate_relative_path(parent_relative_path)?;

    let parent = if parent_relative_path.is_empty() {
        root.to_path_buf()
    } else {
        root.join(parent_relative_path)
    };
    let parent = std::fs::canonicalize(&parent).map_err(|e| format!("父目录不可访问: {e}"))?;
    if !parent.starts_with(root) {
        return Err("父目录超出仓库范围".to_string());
    }
    let metadata = std::fs::metadata(&parent).map_err(|e| format!("读取父目录信息失败: {e}"))?;
    if !metadata.is_dir() {
        return Err("只能在文件夹下创建".to_string());
    }

    let target = parent.join(name);
    if !target.starts_with(root) {
        return Err("目标路径超出仓库范围".to_string());
    }
    let relative_path = strip_root_prefix(root, &target)?;
    Ok((relative_path, target))
}

fn validate_new_child_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("名称不能为空".to_string());
    }
    if name == "." || name == ".." || name.ends_with('.') {
        return Err("名称不合法".to_string());
    }
    if name.chars().any(|ch| {
        ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
    }) {
        return Err("名称包含不允许的字符".to_string());
    }
    Ok(name)
}

fn tree_node_from_path(root: &Path, path: &Path) -> Result<TreeNode, String> {
    let metadata = std::fs::metadata(path).map_err(|e| format!("读取条目信息失败: {e}"))?;
    let is_dir = metadata.is_dir();
    Ok(TreeNode {
        name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_string(),
        relative_path: strip_root_prefix(root, path)?,
        node_type: if is_dir {
            "directory".to_string()
        } else {
            "file".to_string()
        },
        has_children: if is_dir {
            directory_has_children(path)
        } else {
            false
        },
        extension: path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_lowercase()),
        size: if is_dir { None } else { Some(metadata.len()) },
        modified_at: metadata.modified().ok().map(format_system_time),
    })
}

fn safe_join_for_write(root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    validate_relative_path(relative_path)?;
    let target = root.join(relative_path);
    if !target.starts_with(root) {
        return Err("Path escapes the sync space root".to_string());
    }
    Ok(target)
}

fn validate_relative_path(relative_path: &str) -> Result<(), String> {
    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err("Relative path must not be absolute".to_string());
    }
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                return Err("Relative path must not contain '..'".to_string())
            }
            std::path::Component::Prefix(_) => {
                return Err("Relative path must not contain a drive prefix".to_string())
            }
            std::path::Component::RootDir => {
                return Err("Relative path must not contain a root component".to_string())
            }
            std::path::Component::CurDir | std::path::Component::Normal(_) => {}
        }
    }
    Ok(())
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

fn format_system_time(time: std::time::SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Utc> = time.into();
    datetime.to_rfc3339()
}

fn detect_image_mime(path: &Path) -> Result<&'static str, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_lowercase())
        .ok_or_else(|| "File type does not support image preview".to_string())?;

    match extension.as_str() {
        "png" => Ok("image/png"),
        "jpg" | "jpeg" => Ok("image/jpeg"),
        "gif" => Ok("image/gif"),
        "webp" => Ok("image/webp"),
        "svg" => Err("SVG preview is not supported yet".to_string()),
        _ => Err("File type does not support image preview".to_string()),
    }
}

async fn load_baidu_oauth_config(storage: &StorageEngine) -> Result<BaiduOAuthConfig, String> {
    if let Some(config) = storage
        .get_cloud_api_config(BAIDU_PROVIDER)
        .await
        .map_err(|e| format!("Failed to load Baidu API config: {e}"))?
    {
        return Ok(BaiduOAuthConfig {
            device_id: config.device_id,
            client_id: config.client_id,
            client_secret: config.client_secret,
            redirect_uri: config.redirect_uri,
            scopes: config.scopes,
        });
    }

    BaiduOAuthConfig::from_env().map_err(|e| e.to_string())
}

async fn load_baidu_api_config_dto(storage: &StorageEngine) -> Result<BaiduApiConfigDto, String> {
    if let Some(config) = storage
        .get_cloud_api_config(BAIDU_PROVIDER)
        .await
        .map_err(|e| format!("Failed to load Baidu API config: {e}"))?
    {
        return Ok(map_baidu_api_config(Some(config), "local"));
    }

    match BaiduOAuthConfig::from_env() {
        Ok(config) => Ok(BaiduApiConfigDto {
            configured: true,
            provider: BAIDU_PROVIDER.to_string(),
            device_id: config.device_id,
            client_id: config.client_id,
            has_client_secret: config.client_secret.is_some(),
            client_secret: None,
            redirect_uri: config.redirect_uri,
            scopes: config.scopes,
            source: "env".to_string(),
        }),
        Err(_) => Ok(BaiduApiConfigDto {
            configured: false,
            provider: BAIDU_PROVIDER.to_string(),
            device_id: None,
            client_id: String::new(),
            has_client_secret: false,
            client_secret: None,
            redirect_uri: DEFAULT_BAIDU_REDIRECT_URI.to_string(),
            scopes: default_baidu_scopes(),
            source: "none".to_string(),
        }),
    }
}

fn map_baidu_api_config(config: Option<CloudApiConfig>, source: &str) -> BaiduApiConfigDto {
    match config {
        Some(config) => BaiduApiConfigDto {
            configured: true,
            provider: config.provider,
            device_id: config.device_id,
            client_id: config.client_id,
            has_client_secret: config.client_secret.is_some(),
            client_secret: config.client_secret,
            redirect_uri: config.redirect_uri,
            scopes: config.scopes,
            source: source.to_string(),
        },
        None => BaiduApiConfigDto {
            configured: false,
            provider: BAIDU_PROVIDER.to_string(),
            device_id: None,
            client_id: String::new(),
            has_client_secret: false,
            client_secret: None,
            redirect_uri: DEFAULT_BAIDU_REDIRECT_URI.to_string(),
            scopes: default_baidu_scopes(),
            source: "none".to_string(),
        },
    }
}

fn sanitize_baidu_scopes(scopes: Option<Vec<String>>) -> Option<Vec<String>> {
    let scopes: Vec<String> = scopes?
        .into_iter()
        .map(|scope| scope.trim().to_string())
        .filter(|scope| !scope.is_empty())
        .collect();
    if scopes.is_empty() {
        None
    } else {
        Some(scopes)
    }
}

fn default_baidu_scopes() -> Vec<String> {
    vec!["basic".to_string(), "netdisk".to_string()]
}

async fn exchange_baidu_oauth_code(
    config: &BaiduOAuthConfig,
    code: &str,
) -> Result<BaiduTokenResponse, String> {
    let client = reqwest::Client::new();
    let mut params = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code.to_string()),
        ("client_id", config.client_id.clone()),
        ("redirect_uri", config.redirect_uri.clone()),
    ];
    if let Some(client_secret) = &config.client_secret {
        params.push(("client_secret", client_secret.clone()));
    }

    let response = client
        .post(BAIDU_OAUTH_TOKEN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Failed to exchange Baidu OAuth code: {e}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read Baidu OAuth response: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "Baidu OAuth token exchange failed with HTTP {}: {}",
            status,
            sanitize_oauth_error_body(&body)
        ));
    }
    serde_json::from_str::<BaiduTokenResponse>(&body)
        .map_err(|e| format!("Failed to parse Baidu OAuth response: {e}"))
}

fn map_baidu_account_status(account: Option<CloudAccount>) -> BaiduAccountStatusDto {
    let now = Utc::now();
    match account {
        Some(account) => {
            let reconnect_required = account
                .expires_at
                .map(|expires_at| expires_at <= now)
                .unwrap_or(false);
            BaiduAccountStatusDto {
                connected: !reconnect_required,
                provider: account.provider,
                account_id: account.account_id,
                display_name: account.display_name,
                expires_at: account.expires_at.map(|value| value.to_rfc3339()),
                scopes: account.scopes,
                reconnect_required,
            }
        }
        None => BaiduAccountStatusDto {
            connected: false,
            provider: BAIDU_PROVIDER.to_string(),
            account_id: None,
            display_name: None,
            expires_at: None,
            scopes: Vec::new(),
            reconnect_required: false,
        },
    }
}

fn sanitize_oauth_error_body(body: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
    if let serde_json::Value::Object(map) = value {
        let mut sanitized = serde_json::Map::new();
        for (key, value) in map {
            if matches!(
                key.as_str(),
                "access_token" | "refresh_token" | "session_key"
            ) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_image_types() {
        assert_eq!(
            detect_image_mime(Path::new("test.png")).unwrap(),
            "image/png"
        );
        assert!(detect_image_mime(Path::new("test.svg")).is_err());
    }

    #[test]
    fn sanitizes_oauth_tokens_from_error_body() {
        let sanitized = sanitize_oauth_error_body(
            r#"{"error":"invalid_grant","access_token":"secret","refresh_token":"secret2"}"#,
        );
        assert!(sanitized.contains("invalid_grant"));
        assert!(!sanitized.contains("secret"));
        assert!(sanitized.contains("<redacted>"));
    }

    #[test]
    fn marks_implicit_baidu_account_connected_until_expiry() {
        let now = Utc::now();
        let account = CloudAccount {
            provider: BAIDU_PROVIDER.to_string(),
            account_id: Some("baidu:test".to_string()),
            display_name: None,
            access_token_encrypted: b"encrypted-access".to_vec(),
            refresh_token_encrypted: Vec::new(),
            expires_at: Some(now + Duration::hours(1)),
            scopes: default_baidu_scopes(),
            created_at: now,
            updated_at: now,
        };

        let status = map_baidu_account_status(Some(account));

        assert!(status.connected);
        assert!(!status.reconnect_required);
    }

    #[test]
    fn normalizes_baidu_paths_under_app_root() {
        assert_eq!(
            normalize_baidu_app_path("Notes").unwrap(),
            "/apps/SyncFlow/Notes"
        );
        assert_eq!(
            normalize_baidu_app_path("/apps/SyncFlow/Notes/").unwrap(),
            "/apps/SyncFlow/Notes"
        );
        assert!(normalize_baidu_app_path("/work/Notes").is_err());
    }
}
