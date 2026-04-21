use serde::Serialize;
use tauri::State;
use crate::TauriState;

#[derive(Serialize)]
pub struct AuthResult {
    pub success: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn login(
    password: String,
    state: State<'_, TauriState>,
) -> Result<AuthResult, String> {
    tracing::info!("Login successful for device {}", state.device_id);

    Ok(AuthResult {
        success: true,
        error: None,
    })
}

#[derive(Serialize)]
pub struct FolderInfo {
    pub path: String,
    pub status: String,
    pub file_count: u32,
}

#[tauri::command]
pub async fn get_synced_folders(
    _state: State<'_, TauriState>,
) -> Result<Vec<FolderInfo>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn add_synced_folder(
    path: String,
    _state: State<'_, TauriState>,
) -> Result<bool, String> {
    tracing::info!("Add synced folder: {}", path);
    Ok(true)
}

#[derive(Serialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub is_online: bool,
}

#[tauri::command]
pub async fn get_device_info(
    _state: State<'_, TauriState>,
) -> Result<Vec<DeviceInfo>, String> {
    Ok(vec![])
}

#[derive(Serialize)]
pub struct ConflictInfo {
    pub file_path: String,
    pub local_version: String,
    pub remote_version: String,
    pub remote_device: String,
}

#[tauri::command]
pub async fn get_conflicts(
    _state: State<'_, TauriState>,
) -> Result<Vec<ConflictInfo>, String> {
    Ok(vec![])
}

#[derive(Serialize)]
pub struct DiscoveredDevice {
    pub device_id: String,
    pub device_name: String,
    pub ip: String,
    pub platform: String,
}

#[tauri::command]
pub async fn start_sync(
    password: String,
    device_name: String,
    state: State<'_, TauriState>,
) -> Result<bool, String> {
    use syncflow_core::sync::SyncEngine;
    use std::sync::Arc;

    let salt = b"syncflow-local-salt!";
    let root_key =
        syncflow_core::crypto::derive_root_key(&password, salt).map_err(|e| e.to_string())?;

    let storage = {
        let guard = state.storage.lock().await;
        Arc::new((*guard).clone())
    };

    let transport = state.transport.clone();

    let engine = SyncEngine::new(
        storage,
        transport,
        state.device_id.to_string(),
        root_key,
    );

    let mut guard = state.sync_engine.lock().await;
    *guard = Some(engine);

    tracing::info!("Sync engine started for device: {}", device_name);
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
    _state: State<'_, TauriState>,
) -> Result<Vec<DiscoveredDevice>, String> {
    // TODO: return actual discovered devices from discovery service
    Ok(vec![])
}
