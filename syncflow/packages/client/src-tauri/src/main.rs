#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod fs_safety;
mod runtime;

use sqlx::sqlite::SqlitePoolOptions;
use std::path::PathBuf;
use std::sync::Arc;
use syncflow_core::storage::{initialize_schema, StorageEngine};
use syncflow_core::transport::{
    start_sdp_server, DiscoveredDevice, DiscoveryService, SdpDeviceResponse, TransportLayer,
};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use uuid::Uuid;

use runtime::{SessionSyncContext, SyncRuntimeManager};

fn env_or_default(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn env_port_or_default(name: &str, default: u16) -> u16 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default)
}

fn env_bool_or_default(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(default)
}

struct NetworkRuntimeHandles {
    #[allow(dead_code)]
    discovery_service: DiscoveryService,
    #[allow(dead_code)]
    discovery_task: JoinHandle<()>,
    #[allow(dead_code)]
    transport_event_task: JoinHandle<()>,
    #[allow(dead_code)]
    localhost_peer_task: Option<JoinHandle<()>>,
    #[allow(dead_code)]
    sdp_server_task: JoinHandle<()>,
}

struct TauriState {
    storage: Arc<Mutex<StorageEngine>>,
    runtime_manager: Arc<SyncRuntimeManager>,
    session_sync_context: Arc<Mutex<SessionSyncContext>>,
    transport: Arc<TransportLayer>,
    device_id: Uuid,
    device_name: String,
    app_data_dir: PathBuf,
    #[allow(dead_code)]
    network_runtime: NetworkRuntimeHandles,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let profile = env_or_default("SYNCFLOW_PROFILE", "default");
    let data_dir_name = if profile == "default" {
        "syncflow".to_string()
    } else {
        format!("syncflow-{profile}")
    };
    let data_dir = std::env::var("SYNCFLOW_DATA_DIR")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(data_dir_name)
        });
    std::fs::create_dir_all(&data_dir)?;
    let sdp_port = env_port_or_default("SYNCFLOW_SDP_PORT", 18080);

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

    let storage_engine = StorageEngine::new(pool);
    let storage = Arc::new(Mutex::new(storage_engine.clone()));

    // Generate device ID
    let device_id = Uuid::new_v4();

    // Create transport layer
    let transport = Arc::new(TransportLayer::new(device_id.to_string(), sdp_port));
    let session_sync_context = Arc::new(Mutex::new(SessionSyncContext::default()));
    let runtime_manager = Arc::new(SyncRuntimeManager::new(
        Arc::new(storage_engine),
        transport.clone(),
        device_id.to_string(),
    ));
    let default_hostname = whoami::fallible::hostname().unwrap_or_else(|_| "Unknown-Device".into());
    let hostname = env_or_default("SYNCFLOW_DEVICE_NAME", &default_hostname);
    let platform = std::env::consts::OS.to_string();
    let (discovery_service, mut discovery_rx) =
        DiscoveryService::new(&device_id.to_string(), &hostname, &platform, sdp_port)?;

    let discovery_transport = transport.clone();
    let local_device_id = device_id.to_string();
    let discovery_task = tokio::spawn(async move {
        while let Some(device) = discovery_rx.recv().await {
            let should_connect = local_device_id < device.device_id;
            tracing::info!(
                "Discovered peer {} ({}) at {}:{}",
                device.device_name,
                device.device_id,
                device.ip,
                device.port
            );
            discovery_transport
                .register_discovered_device(device.clone())
                .await;
            if should_connect {
                if let Err(error) = discovery_transport.connect_peer(&device).await {
                    tracing::warn!(
                        "Failed to auto-connect to discovered peer {} ({}): {}",
                        device.device_name,
                        device.device_id,
                        error
                    );
                }
            }
        }
    });
    let localhost_peer_task =
        start_localhost_peer_fallback(&profile, device_id.to_string(), transport.clone());
    let mut transport_events = transport.subscribe();
    let runtime_manager_for_events = runtime_manager.clone();
    let transport_event_task = tokio::spawn(async move {
        loop {
            match transport_events.recv().await {
                Ok(event) => {
                    runtime_manager_for_events
                        .handle_transport_event(event)
                        .await
                }
                Err(error) => {
                    tracing::warn!("transport event stream error: {}", error);
                    break;
                }
            }
        }
    });

    let sdp_server_task = start_sdp_server(
        sdp_port,
        device_id.to_string(),
        hostname.clone(),
        platform.clone(),
        transport.clone(),
    )
    .await?;

    let state = TauriState {
        storage,
        runtime_manager,
        session_sync_context,
        transport,
        device_id,
        device_name: hostname,
        app_data_dir: data_dir,
        network_runtime: NetworkRuntimeHandles {
            discovery_service,
            discovery_task,
            transport_event_task,
            localhost_peer_task,
            sdp_server_task,
        },
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::login,
            commands::get_baidu_api_config,
            commands::save_baidu_api_config,
            commands::clear_baidu_api_config,
            commands::get_baidu_account_status,
            commands::start_baidu_oauth,
            commands::open_url,
            commands::complete_baidu_oauth,
            commands::complete_baidu_implicit_oauth,
            commands::disconnect_baidu_account,
            commands::pick_folder,
            commands::get_synced_folders,
            commands::add_synced_folder,
            commands::bind_baidu_space,
            commands::create_baidu_synced_space,
            commands::remove_synced_folder,
            commands::get_tree_children,
            commands::create_tree_file,
            commands::create_tree_folder,
            commands::get_file_details,
            commands::preview_file_image,
            commands::save_text_file,
            commands::open_file,
            commands::preview_file_text,
            commands::get_device_info,
            commands::get_conflicts,
            commands::get_conflict_detail,
            commands::resolve_conflict_keep_local,
            commands::resolve_conflict_keep_remote,
            commands::dismiss_conflict,
            commands::start_sync,
            commands::stop_sync,
            commands::start_space_sync,
            commands::stop_space_sync,
            commands::get_sync_status,
            commands::get_all_sync_statuses,
            commands::get_sync_diagnostics,
            commands::retry_cloud_sync_task,
            commands::ignore_cloud_sync_task,
            commands::restore_remote_deleted_file,
            commands::dismiss_remote_deleted_notice,
            commands::get_discovered_devices,
            commands::export_space_invite,
            commands::join_space_from_invite,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri");

    Ok(())
}

fn start_localhost_peer_fallback(
    profile: &str,
    local_device_id: String,
    transport: Arc<TransportLayer>,
) -> Option<JoinHandle<()>> {
    if !env_bool_or_default("SYNCFLOW_LOCALHOST_PEER_FALLBACK", true) {
        return None;
    }

    let peer_port = match profile {
        "a" | "default" => env_port_or_default("SYNCFLOW_LOCALHOST_PEER_PORT", 18081),
        "b" => env_port_or_default("SYNCFLOW_LOCALHOST_PEER_PORT", 18080),
        _ => return None,
    };

    Some(tokio::spawn(async move {
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{peer_port}/sdp/device");
        loop {
            if let Ok(response) = client.get(&url).send().await {
                if let Ok(peer) = response.json::<SdpDeviceResponse>().await {
                    if peer.device_id != local_device_id {
                        let device = DiscoveredDevice {
                            device_id: peer.device_id.clone(),
                            device_name: peer.device_name,
                            ip: "127.0.0.1".to_string(),
                            port: peer.port,
                            platform: peer.platform,
                        };
                        transport.register_discovered_device(device.clone()).await;
                        if local_device_id < device.device_id {
                            tracing::info!(
                                "Connecting to localhost peer {} ({}) on port {}",
                                device.device_name,
                                device.device_id,
                                device.port
                            );
                            if let Err(error) = transport.connect_peer(&device).await {
                                tracing::warn!(
                                    "Failed to connect to localhost peer {} ({}): {}",
                                    device.device_name,
                                    device.device_id,
                                    error
                                );
                            }
                        }
                    }
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }))
}
