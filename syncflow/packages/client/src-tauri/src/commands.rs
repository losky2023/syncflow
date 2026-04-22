use base64::Engine;
use serde::Serialize;
use std::path::Path;
use tauri::State;
use uuid::Uuid;

use crate::fs_safety::{parse_space_id, resolve_space_path, strip_root_prefix};
use crate::TauriState;

#[derive(Serialize)]
pub struct AuthResult {
    pub success: bool,
    pub error: Option<String>,
    pub device_id: String,
    pub device_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncedSpaceDto {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub status: String,
    pub created_at: String,
    pub last_scanned_at: Option<String>,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePreviewResult {
    pub data_url: String,
    pub mime_type: String,
    pub size: u64,
    pub truncated: bool,
}

#[derive(Serialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub is_online: bool,
}

#[derive(Serialize)]
pub struct ConflictInfo {
    pub file_path: String,
    pub local_version: String,
    pub remote_version: String,
    pub remote_device: String,
}

#[derive(Serialize)]
pub struct DiscoveredDevice {
    pub device_id: String,
    pub device_name: String,
    pub ip: String,
    pub platform: String,
}

#[tauri::command]
pub async fn login(
    _username: String,
    _password: String,
    state: State<'_, TauriState>,
) -> Result<AuthResult, String> {
    tracing::info!(
        "Login successful for device {} ({})",
        state.device_name,
        state.device_id
    );

    Ok(AuthResult {
        success: true,
        error: None,
        device_id: state.device_id.to_string(),
        device_name: state.device_name.clone(),
    })
}

#[tauri::command]
pub async fn pick_folder() -> Result<Option<String>, String> {
    let task = rfd::AsyncFileDialog::new()
        .set_title("选择要同步的文件夹")
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
        .map_err(|e| format!("读取同步空间失败: {e}"))?;

    Ok(spaces.into_iter().map(map_space_dto).collect())
}

#[tauri::command]
pub async fn add_synced_folder(
    path: String,
    state: State<'_, TauriState>,
) -> Result<SyncedSpaceDto, String> {
    let canonical_path = std::fs::canonicalize(&path).map_err(|e| format!("路径不可访问: {e}"))?;
    let meta = tokio::fs::metadata(&canonical_path)
        .await
        .map_err(|e| format!("路径不可访问: {e}"))?;
    if !meta.is_dir() {
        return Err("该路径不是文件夹".to_string());
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
        .map_err(|e| format!("注册同步空间失败: {e}"))?;

    Ok(map_space_dto(created))
}

#[tauri::command]
pub async fn remove_synced_folder(
    space_id: String,
    state: State<'_, TauriState>,
) -> Result<bool, String> {
    let parsed = parse_space_id(&space_id)?;
    let storage = state.storage.lock().await;
    storage
        .remove_synced_space(&parsed)
        .await
        .map_err(|e| format!("移除同步空间失败: {e}"))
}

#[tauri::command]
pub async fn get_tree_children(
    space_id: String,
    parent_relative_path: Option<String>,
    state: State<'_, TauriState>,
) -> Result<Vec<TreeNode>, String> {
    let (_space, parent_path) =
        resolve_space_path(&state, &space_id, parent_relative_path.as_deref())?;
    let root = resolve_space_path(&state, &space_id, None)?.1;

    let entries = std::fs::read_dir(&parent_path).map_err(|e| format!("读取目录失败: {e}"))?;

    let mut nodes = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|e| format!("读取文件信息失败: {e}"))?;
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
    let (space, resolved_path) = resolve_space_path(&state, &space_id, Some(&relative_path))?;
    let metadata = std::fs::metadata(&resolved_path).map_err(|e| format!("读取详情失败: {e}"))?;

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
pub async fn preview_file_text(
    space_id: String,
    relative_path: String,
    max_bytes: Option<usize>,
    state: State<'_, TauriState>,
) -> Result<TextPreviewResult, String> {
    let (_, resolved_path) = resolve_space_path(&state, &space_id, Some(&relative_path))?;
    let metadata = std::fs::metadata(&resolved_path).map_err(|e| format!("读取文件失败: {e}"))?;
    if metadata.is_dir() {
        return Err("目录不支持文本预览".to_string());
    }

    let max_bytes = max_bytes.unwrap_or(100_000);
    let bytes = tokio::fs::read(&resolved_path)
        .await
        .map_err(|e| format!("无法读取文件: {e}"))?;
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
pub async fn preview_file_image(
    space_id: String,
    relative_path: String,
    max_bytes: Option<usize>,
    state: State<'_, TauriState>,
) -> Result<ImagePreviewResult, String> {
    let (_, resolved_path) = resolve_space_path(&state, &space_id, Some(&relative_path))?;
    let metadata = std::fs::metadata(&resolved_path).map_err(|e| format!("读取文件失败: {e}"))?;
    if metadata.is_dir() {
        return Err("目录不支持图片预览".to_string());
    }

    let mime_type = detect_image_mime(&resolved_path)?;
    let max_bytes = max_bytes.unwrap_or(5 * 1024 * 1024).min(5 * 1024 * 1024);
    let size = metadata.len() as usize;
    if size > max_bytes {
        return Err("图片文件过大，无法预览".to_string());
    }

    let bytes = tokio::fs::read(&resolved_path)
        .await
        .map_err(|e| format!("无法读取图片: {e}"))?;
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
    let (_, resolved_path) = resolve_space_path(&state, &space_id, Some(&relative_path))?;
    let file_path = resolved_path.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", &file_path])
            .spawn()
            .map_err(|e| format!("无法打开文件: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("无法打开文件: {e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("无法打开文件: {e}"))?;
    }
    Ok(true)
}

#[tauri::command]
pub async fn get_device_info(state: State<'_, TauriState>) -> Result<Vec<DeviceInfo>, String> {
    let connected = state.transport.connected_peers().await;
    Ok(connected
        .iter()
        .map(|peer_id| DeviceInfo {
            device_id: peer_id.clone(),
            device_name: peer_id.clone(),
            platform: "unknown".to_string(),
            is_online: true,
        })
        .collect())
}

#[tauri::command]
pub async fn get_conflicts(_state: State<'_, TauriState>) -> Result<Vec<ConflictInfo>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn start_sync(
    password: String,
    device_name: String,
    state: State<'_, TauriState>,
) -> Result<bool, String> {
    use std::sync::Arc;
    use syncflow_core::sync::SyncEngine;

    let salt = b"syncflow-local-salt!";
    let root_key =
        syncflow_core::crypto::derive_root_key(&password, salt).map_err(|e| e.to_string())?;

    let storage = {
        let guard = state.storage.lock().await;
        Arc::new((*guard).clone())
    };

    let transport = state.transport.clone();

    let engine = SyncEngine::new(storage, transport, state.device_id.to_string(), root_key);

    let mut guard = state.sync_engine.lock().await;
    *guard = Some(engine);

    if !device_name.is_empty() {
        tracing::info!("Sync engine started for device: {}", device_name);
    }
    Ok(true)
}

#[tauri::command]
pub async fn stop_sync(state: State<'_, TauriState>) -> Result<bool, String> {
    let mut guard = state.sync_engine.lock().await;
    *guard = None;
    tracing::info!("Sync engine stopped");
    Ok(true)
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

fn map_space_dto(space: syncflow_core::storage::SyncedSpace) -> SyncedSpaceDto {
    SyncedSpaceDto {
        id: space.id.to_string(),
        name: space.name,
        root_path: space.root_path,
        status: space.status,
        created_at: space.created_at.to_rfc3339(),
        last_scanned_at: space.last_scanned_at.map(|value| value.to_rfc3339()),
    }
}

fn directory_has_children(path: &Path) -> bool {
    std::fs::read_dir(path)
        .ok()
        .and_then(|mut entries| entries.next())
        .is_some()
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
        .ok_or_else(|| "文件类型不支持图片预览".to_string())?;

    match extension.as_str() {
        "png" => Ok("image/png"),
        "jpg" | "jpeg" => Ok("image/jpeg"),
        "gif" => Ok("image/gif"),
        "webp" => Ok("image/webp"),
        "svg" => Err("SVG 暂不支持图片预览".to_string()),
        _ => Err("文件类型不支持图片预览".to_string()),
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
}
