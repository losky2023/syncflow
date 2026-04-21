# mDNS P2P Device Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the WebSocket signal server with mDNS-based LAN device discovery and local HTTP SDP exchange for pure P2P file sync.

**Architecture:** Each device registers an mDNS service on startup and browses for other devices. When a peer is discovered, a local HTTP server handles SDP offer/answer exchange to establish WebRTC Data Channels.

**Tech Stack:** mdns-sd 0.12 (mDNS), axum 0.7 (local HTTP), webrtc-rs 0.12 (WebRTC), tokio (async), Tauri 2.0 (desktop client)

---

### Task 1: Update dependencies — add mDNS/axum, remove tungstenite

**Files:**
- Modify: `syncflow/Cargo.toml`
- Modify: `syncflow/packages/core/Cargo.toml`

- [ ] **Step 1: Update workspace Cargo.toml**

Modify `syncflow/Cargo.toml`:
- Remove from `[workspace.dependencies]`:
  ```toml
  tokio-tungstenite = { version = "0.26", features = ["tokio-native-tls"] }
  url = "2"
  ```
- Add to `[workspace.dependencies]`:
  ```toml
  mdns-sd = "0.12"
  axum = "0.7"
  tower-http = { version = "0.5", features = ["trace"] }
  hyper = "1"
  ```

Expected final workspace `[workspace.dependencies]` section:
```toml
[workspace.dependencies]
# Crypto
chacha20poly1305 = "0.10"
argon2 = "0.5"
ed25519-dalek = { version = "2", features = ["rand_core"] }
blake3 = "1.5"
aead = "0.5"
# Async
tokio = { version = "1.45", features = ["full"] }
futures = "0.3"
# Storage
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "migrate"] }
# File watching
notify = "6"
notify-debouncer-mini = "0.4"
# WebRTC
webrtc = "0.12"
# mDNS & local HTTP
mdns-sd = "0.12"
axum = "0.7"
tower-http = { version = "0.5", features = ["trace"] }
hyper = "1"
bytes = "1"
reqwest = { version = "0.12", features = ["json"] }
# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
# Utilities
thiserror = "2"
chrono = { version = "0.4", features = ["serde"] }
secrecy = "0.10"
lru = "0.12"
rand = "0.8"
uuid = { version = "1", features = ["v4", "serde"] }
tracing = "0.1"
tracing-subscriber = "0.3"
```

- [ ] **Step 2: Update core Cargo.toml**

Modify `syncflow/packages/core/Cargo.toml`:
- Remove:
  ```toml
  tokio-tungstenite = { workspace = true }
  url = { workspace = true }
  ```
- Add:
  ```toml
  mdns-sd = { workspace = true }
  axum = { workspace = true }
  tower-http = { workspace = true }
  hyper = { workspace = true }
  ```

Expected final file:
```toml
[package]
name = "syncflow-core"
version = "0.1.0"
edition = "2021"

[dependencies]
chacha20poly1305 = { workspace = true }
argon2 = { workspace = true }
ed25519-dalek = { workspace = true }
blake3 = { workspace = true }
aead = { workspace = true }
tokio = { workspace = true }
futures = { workspace = true }
sqlx = { workspace = true }
notify = { workspace = true }
notify-debouncer-mini = { workspace = true }
webrtc = { workspace = true }
mdns-sd = { workspace = true }
axum = { workspace = true }
tower-http = { workspace = true }
hyper = { workspace = true }
bytes = { workspace = true }
reqwest = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
chrono = { workspace = true }
secrecy = { workspace = true }
lru = { workspace = true }
rand = { workspace = true }
uuid = { version = "1", features = ["v4", "serde"] }
tracing = { workspace = true }
```

- [ ] **Step 3: Verify compilation**

Run:
```bash
cd syncflow && cargo check --workspace
```
Expected: Compiles successfully (existing warnings about unused fields are OK, no new errors from missing deps).

- [ ] **Step 4: Commit**

```bash
cd syncflow
git add Cargo.toml packages/core/Cargo.toml
git commit -m "refactor: replace tokio-tungstenite/url with mdns-sd/axum for local P2P discovery"
```

---

### Task 2: Implement mDNS device discovery (`transport/discovery.rs`)

**Files:**
- Create: `syncflow/packages/core/src/transport/discovery.rs`
- Test: `syncflow/packages/core/src/transport/tests.rs`

- [ ] **Step 1: Write the failing test**

Append to `syncflow/packages/core/src/transport/tests.rs` (keep existing tests for now — they will be replaced in Task 5 when signal_client.rs is removed):

```rust
use crate::transport::discovery::{DiscoveredDevice, DiscoveryService};
use uuid::Uuid;

#[tokio::test]
async fn test_discovered_device_from_service_info() {
    let info = mdns_sd::ServiceInfo::new(
        "_syncflow._tcp.local.",
        "test-device-id",
        "test-device_device",
        "192.168.1.10",
        18080,
        &[("device_name", "test-device"), ("platform", "windows")],
    )
    .unwrap();

    let device = DiscoveredDevice::from_service_info(&info).unwrap();
    assert_eq!(device.device_id, "test-device-id");
    assert_eq!(device.device_name, "test-device");
    assert_eq!(device.ip, "192.168.1.10");
    assert_eq!(device.port, 18080);
    assert_eq!(device.platform, "windows");
}

#[tokio::test]
async fn test_discovery_service_create_and_stop() {
    let device_id = Uuid::new_v4().to_string();
    let (service, _rx) =
        DiscoveryService::new(&device_id, "my-pc", "windows", 18080).unwrap();
    // Should not panic; service can be created and dropped cleanly
    drop(service);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:
```bash
cargo test -p syncflow-core test_discovered_device_from_service_info 2>&1 | tail -3
```
Expected: FAIL with "unresolved module discovery"

- [ ] **Step 3: Write the discovery module**

Create `syncflow/packages/core/src/transport/discovery.rs`:

```rust
use mdns_sd::{ServiceDaemon, ServiceInfo, TxtProperties};
use std::net::IpAddr;
use tracing;

const SERVICE_TYPE: &str = "_syncflow._tcp.local.";

/// A device discovered on the LAN via mDNS.
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub device_id: String,
    pub device_name: String,
    pub ip: String,
    pub port: u16,
    pub platform: String,
}

impl DiscoveredDevice {
    pub fn from_service_info(info: &mdns_sd::ServiceInfo) -> Option<Self> {
        let device_id = info.get_subtype().or_else(|| info.get_fullname().split('.').next()).map(String::from)?;

        // Extract device_id from the instance name (subtype before the dot)
        let instance_name = info.get_instance_name();
        let device_id = instance_name.to_string();

        let device_name = info
            .get_property("device_name")
            .map(|p| p.val_str())
            .unwrap_or(&device_id);

        let platform = info
            .get_property("platform")
            .map(|p| p.val_str())
            .unwrap_or("unknown");

        let ip = info
            .get_addresses()
            .iter()
            .next()
            .map(|a| a.to_string())
            .unwrap_or_default();

        let port = info.get_port();

        Some(Self {
            device_id,
            device_name: device_name.to_string(),
            ip,
            port,
            platform: platform.to_string(),
        })
    }

    /// Build the base URL for this device's SDP exchange server.
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.ip, self.port)
    }
}

/// mDNS discovery service that registers this device and browses for peers.
pub struct DiscoveryService {
    daemon: ServiceDaemon,
}

impl DiscoveryService {
    /// Register this device on the LAN and start browsing for peers.
    ///
    /// Returns the discovery service and a receiver for discovered devices.
    pub fn new(
        device_id: &str,
        device_name: &str,
        platform: &str,
        port: u16,
    ) -> Result<(Self, tokio::sync::mpsc::Receiver<DiscoveredDevice>), crate::error::SyncFlowError> {
        let daemon = ServiceDaemon::new().map_err(|e| {
            crate::error::SyncFlowError::Signal(format!("Failed to create mDNS daemon: {}", e))
        })?;

        // Register this device
        let properties = TxtProperties::new();
        let props = vec![
            ("device_name".to_string(), device_name.to_string()),
            ("platform".to_string(), platform.to_string()),
        ];
        // mdns-sd uses the properties in the ServiceInfo::new call
        let service = ServiceInfo::new(
            SERVICE_TYPE,
            device_id,
            &format!("{}_device", device_name),
            "",
            port,
            &props[..],
        )
        .map_err(|e| {
            crate::error::SyncFlowError::Signal(format!("Failed to register mDNS service: {}", e))
        })?;

        daemon.register(service).map_err(|e| {
            crate::error::SyncFlowError::Signal(format!("Failed to register mDNS service: {}", e))
        })?;

        // Start browsing
        let rx = daemon.browse(SERVICE_TYPE).map_err(|e| {
            crate::error::SyncFlowError::Signal(format!("Failed to start mDNS browse: {}", e))
        })?;

        // Spawn a task to convert mDNS events to discovered devices
        let (tx, local_rx) = tokio::sync::mpsc::channel(100);
        std::thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                    if let Some(device) = DiscoveredDevice::from_service_info(&info) {
                        let _ = tx.blocking_send(device);
                    }
                }
            }
        });

        Ok((Self { daemon }, local_rx))
    }

    /// Stop the discovery service.
    pub fn stop(self) {
        let _ = self.daemon.shutdown();
    }
}
```

- [ ] **Step 4: Add discovery module to transport/mod.rs**

Add to `syncflow/packages/core/src/transport/mod.rs` at the top (after existing `pub mod` lines):

```rust
pub mod discovery;
```

- [ ] **Step 5: Run test to verify it passes**

Run:
```bash
cargo test -p syncflow-core discovery -- --nocapture
```
Expected: 2 tests pass.

- [ ] **Step 6: Commit**

```bash
cd syncflow
git add packages/core/src/transport/discovery.rs packages/core/src/transport/tests.rs
git commit -m "feat: implement mDNS device discovery with DiscoveredDevice and DiscoveryService"
```

---

### Task 3: Implement local SDP exchange HTTP server (`transport/sdp_exchange.rs`)

**Files:**
- Create: `syncflow/packages/core/src/transport/sdp_exchange.rs`
- Modify: `syncflow/packages/core/src/transport/mod.rs`

- [ ] **Step 1: Create the SDP exchange module**

Create `syncflow/packages/core/src/transport/sdp_exchange.rs`:

```rust
use axum::{
    extract::State,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use webrtc::peer_connection::RTCPeerConnection;

use crate::error::{Result, SyncFlowError};

#[derive(Debug, Deserialize)]
pub struct SdpOfferRequest {
    pub sdp: String,
    pub device_id: String,
}

#[derive(Debug, Serialize)]
pub struct SdpAnswerResponse {
    pub sdp: String,
}

#[derive(Debug, Serialize)]
pub struct SdpErrorResponse {
    pub error: String,
}

/// Shared state for the SDP exchange server.
pub struct SdpServerState {
    pub peer_connection: Arc<RTCPeerConnection>,
}

/// Start a local HTTP server for SDP offer/answer exchange.
///
/// Listens on 0.0.0.0:<port> and provides:
/// - POST /sdp/offer — receive an offer, create and return an answer
/// - POST /sdp/answer — receive an answer (for one-way notification)
pub async fn start_sdp_server(
    port: u16,
    pc: Arc<RTCPeerConnection>,
) -> Result<tokio::task::JoinHandle<()>> {
    let state = SdpServerState {
        peer_connection: pc,
    };

    let app = Router::new()
        .route("/sdp/offer", post(handle_offer))
        .route("/sdp/answer", post(handle_answer))
        .with_state(Arc::new(state));

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| SyncFlowError::Signal(format!("Failed to bind SDP server on {}: {}", addr, e)))?;

    let handle = tokio::spawn(async move {
        tracing::info!("SDP exchange server listening on {}", addr);
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("SDP server error: {}", e);
        }
    });

    Ok(handle)
}

async fn handle_offer(
    State(state): State<Arc<SdpServerState>>,
    Json(req): Json<SdpOfferRequest>,
) -> Json<SdpAnswerResponse> {
    match do_handle_offer(&state, &req.sdp).await {
        Ok(answer_sdp) => Json(SdpAnswerResponse { sdp: answer_sdp }),
        Err(e) => {
            tracing::error!("Failed to handle offer: {}", e);
            Json(SdpAnswerResponse { sdp: String::new() })
        }
    }
}

async fn do_handle_offer(
    state: &SdpServerState,
    sdp: &str,
) -> Result<String> {
    use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

    let offer = RTCSessionDescription::offer(sdp.to_string())
        .map_err(|e| SyncFlowError::WebRtc(format!("Invalid offer SDP: {}", e)))?;

    state.peer_connection.set_remote_description(offer).await
        .map_err(|e| SyncFlowError::WebRtc(format!("Failed to set remote description: {}", e)))?;

    let answer = state.peer_connection.create_answer(None).await
        .map_err(|e| SyncFlowError::WebRtc(format!("Failed to create answer: {}", e)))?;

    state.peer_connection.set_local_description(answer.clone()).await
        .map_err(|e| SyncFlowError::WebRtc(format!("Failed to set local description: {}", e)))?;

    Ok(answer.sdp)
}

async fn handle_answer(
    State(_state): State<Arc<SdpServerState>>,
    Json(req): Json<SdpOfferRequest>,
) -> Json<SdpAnswerResponse> {
    use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

    let answer = RTCSessionDescription::answer(req.sdp.clone())
        .map_err(|e| SyncFlowError::WebRtc(format!("Invalid answer SDP: {}", e)));

    match answer {
        Ok(answer) => {
            let _ = _state.peer_connection.set_remote_description(answer).await;
            Json(SdpAnswerResponse { sdp: "ok".to_string() })
        }
        Err(e) => {
            tracing::error!("Failed to handle answer: {}", e);
            Json(SdpAnswerResponse { sdp: String::new() })
        }
    }
}
```

- [ ] **Step 2: Add module to transport/mod.rs**

Add to `syncflow/packages/core/src/transport/mod.rs`:

```rust
pub mod sdp_exchange;
```

- [ ] **Step 3: Commit**

```bash
cd syncflow
git add packages/core/src/transport/sdp_exchange.rs packages/core/src/transport/mod.rs
git commit -m "feat: implement local HTTP SDP exchange server on /sdp/offer and /sdp/answer"
```

---

### Task 4: Rewrite TransportLayer — remove signal, add mDNS + SDP

**Files:**
- Modify: `syncflow/packages/core/src/transport/mod.rs`
- Delete: `syncflow/packages/core/src/transport/signal_client.rs`

- [ ] **Step 1: Delete signal_client.rs**

```bash
rm syncflow/packages/core/src/transport/signal_client.rs
```

- [ ] **Step 2: Rewrite transport/mod.rs**

Replace the entire contents of `syncflow/packages/core/src/transport/mod.rs` with:

```rust
pub mod discovery;
pub mod sdp_exchange;
pub mod webrtc_peer;

#[cfg(test)]
mod tests;

pub use discovery::{DiscoveredDevice, DiscoveryService};
pub use sdp_exchange::{start_sdp_server, SdpServerState};
pub use webrtc_peer::*;

use bytes::Bytes;
use crate::error::{Result, SyncFlowError};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::RTCPeerConnection;

/// Transport layer manages WebRTC connections to LAN-discovered peers.
pub struct TransportLayer {
    peers: Arc<RwLock<HashMap<String, Arc<RTCPeerConnection>>>>,
    data_channels: Arc<RwLock<HashMap<String, Arc<RTCDataChannel>>>>,
    event_tx: broadcast::Sender<TransportEvent>,
    local_port: u16,
    device_id: String,
    ice_servers: Vec<String>,
}

/// Events emitted by the transport layer.
#[derive(Debug, Clone)]
pub enum TransportEvent {
    PeerConnected { device_id: String },
    PeerDisconnected { device_id: String },
    DataReceived { from: String, data: Vec<u8> },
    PeerDiscovered { device: DiscoveredDevice },
}

impl TransportLayer {
    pub fn new(device_id: String, local_port: u16) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            data_channels: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            local_port,
            device_id,
            ice_servers: vec!["stun:stun.l.google.com:19302".to_string()],
        }
    }

    /// Subscribe to transport events.
    pub fn subscribe(&self) -> broadcast::Receiver<TransportEvent> {
        self.event_tx.subscribe()
    }

    /// Get list of connected peer IDs.
    pub async fn connected_peers(&self) -> Vec<String> {
        self.peers.read().await.keys().cloned().collect()
    }

    /// Send data to a peer.
    pub async fn send_data(&self, peer_id: &str, data: &[u8]) -> Result<()> {
        let channels = self.data_channels.read().await;
        let dc = channels
            .get(peer_id)
            .ok_or_else(|| SyncFlowError::WebRtc(format!("No connection to peer {}", peer_id)))?;

        dc.send(&Bytes::from(data.to_vec()))
            .await
            .map_err(|e| SyncFlowError::WebRtc(format!("Failed to send data: {}", e)))?;

        Ok(())
    }

    /// Start the transport: create a peer connection, start SDP server, and begin discovery.
    ///
    /// Returns the discovery receiver and SDP server handle.
    pub async fn start(
        &self,
        device_name: &str,
        platform: &str,
    ) -> Result<(
        tokio::sync::mpsc::Receiver<DiscoveredDevice>,
        tokio::task::JoinHandle<()>,
    )> {
        // Start discovery
        let (discovery, rx) = DiscoveryService::new(
            &self.device_id,
            device_name,
            platform,
            self.local_port,
        )?;

        // Create a template peer connection for receiving offers
        let pc = Arc::new(create_peer_connection(&self.ice_servers).await?);

        // Set up data channel handler for incoming connections
        let event_tx = self.event_tx.clone();
        self.setup_data_channel_handlers(&pc).await;

        // Start SDP server
        let sdp_handle = start_sdp_server(self.local_port, pc).await?;

        // Store the template PC
        // Note: the SDP server holds one Arc, we need another for future offerers
        let _ = discovery; // kept alive by the spawned thread

        Ok((rx, sdp_handle))
    }

    /// Connect to a discovered peer by initiating an SDP offer.
    pub async fn connect_peer(&self, device: &DiscoveredDevice) -> Result<()> {
        if self.peers.read().await.contains_key(&device.device_id) {
            return Ok(());
        }

        let pc = Arc::new(create_peer_connection(&self.ice_servers).await?);

        // Set up data channel event handler
        let event_tx = self.event_tx.clone();
        let peer_id_str = device.device_id.clone();
        self.setup_data_channel_handlers(&pc).await;

        // Create data channel
        let dc = create_data_channel(&pc, "syncflow").await?;

        // Create SDP offer
        let offer_sdp = create_offer(&pc).await?;

        // Send offer to peer's SDP server
        let client = reqwest::Client::new();
        let url = format!("{}/sdp/offer", device.base_url());
        let body = serde_json::json!({
            "sdp": offer_sdp,
            "device_id": &self.device_id,
        });

        let response = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| SyncFlowError::WebRtc(format!("Failed to send SDP offer to {}: {}", device.device_id, e)))?;

        let answer: crate::transport::sdp_exchange::SdpAnswerResponse = response
            .json()
            .await
            .map_err(|e| SyncFlowError::WebRtc(format!("Failed to parse SDP answer: {}", e)))?;

        if answer.sdp.is_empty() {
            return Err(SyncFlowError::WebRtc(
                "Empty SDP answer received".into(),
            ));
        }

        // Set remote answer
        set_remote_answer(&pc, &answer.sdp).await?;

        // Store peer
        self.peers
            .write()
            .await
            .insert(device.device_id.clone(), pc);
        self.data_channels
            .write()
            .await
            .insert(device.device_id.clone(), dc);

        let _ = self.event_tx.send(TransportEvent::PeerConnected {
            device_id: device.device_id.clone(),
        });

        tracing::info!("Connected to peer {} ({})", device.device_name, device.ip);
        Ok(())
    }

    /// Set up data channel event handlers on a peer connection.
    async fn setup_data_channel_handlers(&self, pc: &RTCPeerConnection) {
        let event_tx = self.event_tx.clone();

        pc.on_data_channel(Box::new(move |dc| {
            let dc = dc.clone();
            let tx = event_tx.clone();
            Box::pin(async move {
                let peer_id = dc.label();
                dc.on_message(Box::new(move |msg| {
                    let tx = tx.clone();
                    let pid = peer_id.clone();
                    Box::pin(async move {
                        let _ = tx.send(TransportEvent::DataReceived {
                            from: pid,
                            data: msg.data.to_vec(),
                        });
                    })
                }));
            })
        }));
    }
}
```

- [ ] **Step 3: Verify compilation**

Run:
```bash
cd syncflow && cargo check -p syncflow-core
```
Expected: Compiles with no errors (warnings about dead_code are OK for now).

- [ ] **Step 4: Commit**

```bash
cd syncflow
git add Cargo.toml packages/core/Cargo.toml packages/core/src/transport/mod.rs packages/core/src/transport/signal_client.rs
git commit -m "refactor: rewrite TransportLayer to use mDNS discovery and local SDP exchange, remove signal client"
```

---

### Task 5: Update tests for new transport module

**Files:**
- Modify: `syncflow/packages/core/src/transport/tests.rs`

- [ ] **Step 1: Rewrite tests**

Replace the entire `syncflow/packages/core/src/transport/tests.rs` (removes old signal_client tests from previous codebase):

```rust
#[cfg(test)]
mod tests {
    use crate::transport::discovery::DiscoveredDevice;

    #[test]
    fn test_discovered_device_base_url() {
        let device = DiscoveredDevice {
            device_id: "abc123".to_string(),
            device_name: "my-pc".to_string(),
            ip: "192.168.1.10".to_string(),
            port: 18080,
            platform: "windows".to_string(),
        };
        assert_eq!(device.base_url(), "http://192.168.1.10:18080");
    }

    #[test]
    fn test_transport_layer_new() {
        use crate::transport::TransportLayer;
        let tl = TransportLayer::new("device-1".to_string(), 18080);
        // Should not panic
    }
}
```

- [ ] **Step 2: Run tests**

Run:
```bash
cargo test -p syncflow-core transport -- --nocapture
```
Expected: 2 tests pass.

- [ ] **Step 3: Run all workspace tests**

Run:
```bash
cargo test --workspace
```
Expected: All tests pass (24 from before + 2 new = 26 total).

- [ ] **Step 4: Commit**

```bash
cd syncflow
git add packages/core/src/transport/tests.rs
git commit -m "test: update transport tests for mDNS discovery module"
```

---

### Task 6: Update client main.rs — wire up TransportLayer and SyncEngine

**Files:**
- Modify: `syncflow/packages/client/src-tauri/src/main.rs`
- Modify: `syncflow/packages/client/src-tauri/src/commands.rs`

- [ ] **Step 1: Rewrite main.rs**

Replace `syncflow/packages/client/src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use syncflow_core::storage::StorageEngine;
use syncflow_core::transport::TransportLayer;
use syncflow_core::sync::SyncEngine;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

struct TauriState {
    storage: Arc<Mutex<StorageEngine>>,
    sync_engine: Arc<Mutex<Option<SyncEngine>>>,
    transport: Arc<TransportLayer>,
    device_id: Uuid,
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
            created_at TEXT NOT NULL)"#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS sync_state (
            id INTEGER PRIMARY KEY, peer_id TEXT NOT NULL UNIQUE,
            last_sync_at TEXT, sync_status TEXT NOT NULL,
            pending_changes INTEGER DEFAULT 0)"#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS file_versions (
            id INTEGER PRIMARY KEY, file_path TEXT NOT NULL,
            hash TEXT NOT NULL, version_vector TEXT NOT NULL,
            device_id TEXT NOT NULL, is_conflict BOOLEAN DEFAULT FALSE,
            created_at TEXT NOT NULL)"#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS devices (
            id INTEGER PRIMARY KEY, device_id TEXT UNIQUE NOT NULL,
            device_name TEXT NOT NULL, platform TEXT NOT NULL,
            public_key TEXT NOT NULL, last_seen_at TEXT)"#,
    )
    .execute(&pool)
    .await?;

    let storage = Arc::new(Mutex::new(StorageEngine::new(pool)));

    // Generate device ID
    let device_id = Uuid::new_v4();

    // Create transport layer
    let transport = Arc::new(TransportLayer::new(
        device_id.to_string(),
        18080,
    ));

    let state = TauriState {
        storage,
        sync_engine: Arc::new(Mutex::new(None)),
        transport,
        device_id,
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::login,
            commands::get_synced_folders,
            commands::add_synced_folder,
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
```

- [ ] **Step 2: Rewrite commands.rs**

Replace `syncflow/packages/client/src-tauri/src/commands.rs`:

```rust
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
    // Derive root key from password with a fixed salt (for local-only auth)
    let salt = b"syncflow-local-salt!";
    let root_key = syncflow_core::crypto::derive_root_key(&password, salt)
        .map_err(|e| e.to_string())?;

    // Store root key in sync engine later during start_sync
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
        Arc::new(guard.clone())
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
```

- [ ] **Step 3: Verify compilation**

Run:
```bash
cd syncflow && cargo check -p syncflow-client
```
Expected: Compiles with warnings about unused fields (acceptable for stub commands).

- [ ] **Step 4: Commit**

```bash
cd syncflow
git add packages/client/src-tauri/src/main.rs packages/client/src-tauri/src/commands.rs
git commit -m "feat: wire up TransportLayer and SyncEngine in Tauri client with start/stop sync commands"
```

---

### Task 7: Update documentation — README and CLAUDE.md

**Files:**
- Modify: `CLAUDE.md` (repo root)
- Modify: `README.md` (repo root)

- [ ] **Step 1: Update CLAUDE.md**

Replace the transport section in `CLAUDE.md` to reflect mDNS instead of signal server:

```markdown
| transport | `transport/mod.rs` | WebRTC peer connections, mDNS discovery (mdns-sd), local SDP exchange (axum) |
```

Update the Tech Stack section:
```
- **mDNS**: LAN device discovery via mdns-sd 0.12 (Bonjour/Avahi compatible)
- **Local HTTP**: SDP offer/answer exchange via lightweight axum server on port 18080
```

Remove the signal server from the architecture description.

- [ ] **Step 2: Update README.md**

Remove signal server from the architecture diagram. Update the API Endpoints section to remove server endpoints. Update the "Run Signal Server" section:

Replace with:
```bash
### Run Desktop Client

```bash
cd packages/client/src-tauri
cargo tauri dev
```

Devices on the same LAN will automatically discover each other via mDNS.
```

- [ ] **Step 3: Run all tests one final time**

```bash
cd syncflow && cargo test --workspace
```
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
cd syncflow
git add CLAUDE.md README.md
git commit -m "docs: update README and CLAUDE.md for mDNS P2P architecture"
```
