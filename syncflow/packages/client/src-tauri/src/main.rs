#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use syncflow_core::storage::StorageEngine;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::sync::Arc;
use tokio::sync::Mutex;

struct TauriState {
    storage: Arc<Mutex<StorageEngine>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("syncflow");
    std::fs::create_dir_all(&data_dir)?;

    let db_path = format!("sqlite:{}/syncflow.db", data_dir.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_path)
        .await?;

    // Create tables
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS file_metadata (
            id INTEGER PRIMARY KEY, path TEXT UNIQUE NOT NULL,
            hash TEXT NOT NULL, size BIGINT NOT NULL,
            modified_at TEXT NOT NULL, version_vector TEXT NOT NULL,
            created_at TEXT NOT NULL)"#
    ).execute(&pool).await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS sync_state (
            id INTEGER PRIMARY KEY, peer_id TEXT NOT NULL UNIQUE,
            last_sync_at TEXT, sync_status TEXT NOT NULL,
            pending_changes INTEGER DEFAULT 0)"#
    ).execute(&pool).await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS file_versions (
            id INTEGER PRIMARY KEY, file_path TEXT NOT NULL,
            hash TEXT NOT NULL, version_vector TEXT NOT NULL,
            device_id TEXT NOT NULL, is_conflict BOOLEAN DEFAULT FALSE,
            created_at TEXT NOT NULL)"#
    ).execute(&pool).await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS devices (
            id INTEGER PRIMARY KEY, device_id TEXT UNIQUE NOT NULL,
            device_name TEXT NOT NULL, platform TEXT NOT NULL,
            public_key TEXT NOT NULL, last_seen_at TEXT)"#
    ).execute(&pool).await?;

    let storage = Arc::new(Mutex::new(StorageEngine::new(pool)));

    tauri::Builder::default()
        .manage(TauriState { storage })
        .invoke_handler(tauri::generate_handler![
            commands::login,
            commands::get_synced_folders,
            commands::add_synced_folder,
            commands::get_device_info,
            commands::get_conflicts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri");

    Ok(())
}
