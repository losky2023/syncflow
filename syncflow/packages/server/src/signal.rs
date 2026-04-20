use axum::{
    extract::ws::{WebSocketUpgrade, WebSocket, Message},
    response::IntoResponse,
    extract::State,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, Mutex};
use chrono::Utc;
use crate::AppState;

/// Client to Server signal messages
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "device_online")]
    DeviceOnline { device_id: String, token: String },
    #[serde(rename = "device_offline")]
    DeviceOffline { device_id: String },
    #[serde(rename = "sdp_offer")]
    SdpOffer { target: String, sdp: String },
    #[serde(rename = "sdp_answer")]
    SdpAnswer { target: String, sdp: String },
    #[serde(rename = "ice_candidate")]
    IceCandidate { target: String, candidate: String },
    #[serde(rename = "sync_request")]
    SyncRequest { target: String },
}

/// Server to Client signal messages
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "device_online")]
    DeviceOnline { device_id: String },
    #[serde(rename = "device_offline")]
    DeviceOffline { device_id: String },
    #[serde(rename = "sdp_offer")]
    SdpOffer { from: String, sdp: String },
    #[serde(rename = "sdp_answer")]
    SdpAnswer { from: String, sdp: String },
    #[serde(rename = "ice_candidate")]
    IceCandidate { from: String, candidate: String },
    #[serde(rename = "error")]
    Error { code: String, message: String },
}

/// Registry: device_id to mpsc sender for broadcast/forward messages
type WsSender = Arc<Mutex<futures::stream::SplitSink<WebSocket, Message>>>;
pub type DeviceRegistry = Arc<RwLock<HashMap<String, mpsc::UnboundedSender<ServerMessage>>>>;

#[derive(Clone)]
pub struct SignalState {
    pub app: AppState,
    pub registry: DeviceRegistry,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SignalState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: SignalState) {
    let (ws_sender, mut ws_receiver) = socket.split();
    let ws_sender: WsSender = Arc::new(Mutex::new(ws_sender));
    let mut device_id: Option<String> = None;

    while let Some(Ok(msg)) = ws_receiver.next().await {
        if let Message::Text(text) = msg {
            match serde_json::from_str::<ClientMessage>(&text) {
                Ok(client_msg) => {
                    match handle_client_message(&client_msg, &mut device_id, &state, &ws_sender).await {
                        Ok(()) => {}
                        Err(e) => {
                            let err_msg = ServerMessage::Error {
                                code: "internal_error".into(),
                                message: e,
                            };
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(
                                serde_json::to_string(&err_msg).unwrap_or_default(),
                            )).await;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Invalid signal message: {} - {:?}", e, text);
                }
            }
        }
    }

    // Client disconnected
    if let Some(ref did) = device_id {
        let mut registry = state.registry.write().await;
        registry.remove(did);
        drop(registry);

        // Update DB
        let _ = sqlx::query(
            "UPDATE server_devices SET is_online = FALSE, last_seen_at = ? WHERE device_id = ?",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(did)
        .execute(&state.app.pool)
        .await;

        // Broadcast offline
        broadcast_server_message(
            &state.registry,
            &ServerMessage::DeviceOffline { device_id: did.clone() },
        ).await;
        tracing::info!("Device {} disconnected", did);
    }
}

async fn handle_client_message(
    msg: &ClientMessage,
    device_id: &mut Option<String>,
    state: &SignalState,
    ws_sender: &WsSender,
) -> Result<(), String> {
    match msg {
        ClientMessage::DeviceOnline { device_id: did, token: _ } => {
            tracing::info!("Device {} came online", did);

            // Create channel for this device
            let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<ServerMessage>();

            // Spawn task to forward registry messages to WebSocket
            let ws_sender = Arc::clone(ws_sender);
            tokio::spawn(async move {
                while let Some(msg) = msg_rx.recv().await {
                    let text = match serde_json::to_string(&msg) {
                        Ok(t) => t,
                        Err(_) => continue,
                    };
                    let mut sender = ws_sender.lock().await;
                    if sender.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }
            });

            *device_id = Some(did.clone());

            // Update DB
            let now_str = Utc::now().to_rfc3339();
            let _ = sqlx::query(
                "UPDATE server_devices SET is_online = TRUE, last_seen_at = ? WHERE device_id = ?",
            )
            .bind(now_str)
            .bind(did)
            .execute(&state.app.pool)
            .await;

            // Register in registry
            state.registry.write().await.insert(did.clone(), msg_tx);

            // Broadcast online notification
            broadcast_server_message(
                &state.registry,
                &ServerMessage::DeviceOnline { device_id: did.clone() },
            ).await;

            Ok(())
        }
        ClientMessage::DeviceOffline { device_id: did } => {
            state.registry.write().await.remove(did);
            broadcast_server_message(
                &state.registry,
                &ServerMessage::DeviceOffline { device_id: did.clone() },
            ).await;
            Ok(())
        }
        ClientMessage::SdpOffer { target, sdp } => {
            forward_to_target(state, target, ServerMessage::SdpOffer {
                from: device_id.clone().unwrap_or_default(),
                sdp: sdp.clone(),
            }).await
        }
        ClientMessage::SdpAnswer { target, sdp } => {
            forward_to_target(state, target, ServerMessage::SdpAnswer {
                from: device_id.clone().unwrap_or_default(),
                sdp: sdp.clone(),
            }).await
        }
        ClientMessage::IceCandidate { target, candidate } => {
            forward_to_target(state, target, ServerMessage::IceCandidate {
                from: device_id.clone().unwrap_or_default(),
                candidate: candidate.clone(),
            }).await
        }
        ClientMessage::SyncRequest { target } => {
            forward_to_target(state, target, ServerMessage::Error {
                code: "sync_request_received".into(),
                message: format!("Sync requested by {}", device_id.clone().unwrap_or_default()),
            }).await
        }
    }
}

async fn forward_to_target(
    state: &SignalState,
    target: &str,
    message: ServerMessage,
) -> Result<(), String> {
    let registry = state.registry.read().await;
    if let Some(tx) = registry.get(target) {
        tx.send(message.clone())
            .map_err(|e| format!("Send failed: {}", e))?;
        tracing::debug!("Forwarded to {}: {:?}", target, message);
        Ok(())
    } else {
        Err(format!("Target device {} not online", target))
    }
}

async fn broadcast_server_message(registry: &DeviceRegistry, message: &ServerMessage) {
    let registry = registry.read().await;
    for tx in registry.values() {
        let _ = tx.send(message.clone());
    }
}
