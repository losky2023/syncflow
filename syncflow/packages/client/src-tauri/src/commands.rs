use serde::Serialize;
use tauri::State;
use tokio::sync::Mutex;
use crate::TauriState;
use syncflow_core::storage::StorageEngine;

#[derive(Serialize)]
pub struct AuthResult {
    pub success: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn login(username: String, password: String, _state: State<'_, TauriState>) -> Result<AuthResult, String> {
    Ok(AuthResult { success: true, error: None })
}

#[derive(Serialize)]
pub struct FolderInfo {
    pub path: String,
    pub status: String,
    pub file_count: u32,
}

#[tauri::command]
pub async fn get_synced_folders(_state: State<'_, TauriState>) -> Result<Vec<FolderInfo>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn add_synced_folder(path: String, _state: State<'_, TauriState>) -> Result<bool, String> {
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
pub async fn get_device_info(_state: State<'_, TauriState>) -> Result<Vec<DeviceInfo>, String> {
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
pub async fn get_conflicts(_state: State<'_, TauriState>) -> Result<Vec<ConflictInfo>, String> {
    Ok(vec![])
}
