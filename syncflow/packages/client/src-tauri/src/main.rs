#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod fs_safety;

use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use syncflow_core::storage::{initialize_schema, StorageEngine};
use syncflow_core::sync::SyncEngine;
use syncflow_core::transport::TransportLayer;
use tokio::sync::Mutex;
use uuid::Uuid;

struct TauriState {
    storage: Arc<Mutex<StorageEngine>>,
    sync_engine: Arc<Mutex<Option<SyncEngine>>>,
    transport: Arc<TransportLayer>,
    device_id: Uuid,
    device_name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("syncflow");
    std::fs::create_dir_all(&data_dir)?;

    let db_path = data_dir.join("syncflow.db");

    // SQLite on Windows needs the file to exist before connecting
    if !db_path.exists() {
        std::fs::File::create(&db_path)?;
    }

    let db_url = format!("sqlite:{}", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    initialize_schema(&pool).await?;

    let storage = Arc::new(Mutex::new(StorageEngine::new(pool)));

    // Generate device ID
    let device_id = Uuid::new_v4();

    // Create transport layer
    let transport = Arc::new(TransportLayer::new(device_id.to_string(), 18080));

    let hostname = whoami::fallible::hostname().unwrap_or_else(|_| "Unknown-Device".into());
    let state = TauriState {
        storage,
        sync_engine: Arc::new(Mutex::new(None)),
        transport,
        device_id,
        device_name: hostname,
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::login,
            commands::pick_folder,
            commands::get_synced_folders,
            commands::add_synced_folder,
            commands::remove_synced_folder,
            commands::get_tree_children,
            commands::get_file_details,
            commands::preview_file_image,
            commands::open_file,
            commands::preview_file_text,
            commands::get_device_info,
            commands::get_conflicts,
            commands::start_sync,
            commands::stop_sync,
            commands::get_discovered_devices,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri");

    Ok(())
}
