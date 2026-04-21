use crate::error::{Result, SyncFlowError};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ClientSignalMessage {
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ServerSignalMessage {
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

/// Incoming signal events from the server.
#[derive(Debug, Clone)]
pub enum SignalEvent {
    DeviceOnline { device_id: String },
    DeviceOffline { device_id: String },
    SdpOffer { from: String, sdp: String },
    SdpAnswer { from: String, sdp: String },
    IceCandidate { from: String, candidate: String },
}

pub struct SignalClient {
    sender: Arc<RwLock<Option<mpsc::UnboundedSender<ClientSignalMessage>>>>,
    event_tx: mpsc::Sender<SignalEvent>,
}

impl SignalClient {
    pub fn new(event_tx: mpsc::Sender<SignalEvent>) -> Self {
        Self {
            sender: Arc::new(RwLock::new(None)),
            event_tx,
        }
    }

    /// Connect to the signal server.
    pub async fn connect(&self, url: &str, token: &str, device_id: &str) -> Result<()> {
        let ws_url = format!("{}/ws/signal?token={}", url, token);
        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .map_err(|e| SyncFlowError::Signal(format!("WebSocket connection failed: {}", e)))?;

        let (mut write, mut read) = ws_stream.split();

        // Send device_online message
        let online_msg = ClientSignalMessage::DeviceOnline {
            device_id: device_id.to_string(),
            token: token.to_string(),
        };
        let text = serde_json::to_string(&online_msg).unwrap();
        write
            .send(Message::Text(text.into()))
            .await
            .map_err(|e| SyncFlowError::Signal(format!("Failed to send device_online: {}", e)))?;

        // Create mpsc channel for outbound messages
        let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<ClientSignalMessage>();

        // Spawn task to forward outbound messages
        tokio::spawn(async move {
            while let Some(msg) = msg_rx.recv().await {
                let text = match serde_json::to_string(&msg) {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::error!("Failed to serialize signal message: {}", e);
                        continue;
                    }
                };
                if write.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
        });

        // Spawn task to process inbound messages
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            while let Some(Ok(msg)) = read.next().await {
                if let Message::Text(text) = msg {
                    match serde_json::from_str::<ServerSignalMessage>(&text) {
                        Ok(server_msg) => {
                            let event = match server_msg {
                                ServerSignalMessage::DeviceOnline { device_id } => {
                                    SignalEvent::DeviceOnline { device_id }
                                }
                                ServerSignalMessage::DeviceOffline { device_id } => {
                                    SignalEvent::DeviceOffline { device_id }
                                }
                                ServerSignalMessage::SdpOffer { from, sdp } => {
                                    SignalEvent::SdpOffer { from, sdp }
                                }
                                ServerSignalMessage::SdpAnswer { from, sdp } => {
                                    SignalEvent::SdpAnswer { from, sdp }
                                }
                                ServerSignalMessage::IceCandidate { from, candidate } => {
                                    SignalEvent::IceCandidate { from, candidate }
                                }
                                ServerSignalMessage::Error { code, message } => {
                                    tracing::warn!("Signal server error: {} - {}", code, message);
                                    continue;
                                }
                            };
                            let _ = event_tx.send(event).await;
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse signal message: {}", e);
                        }
                    }
                }
            }
        });

        // Store sender for later use
        *self.sender.write().await = Some(msg_tx);

        Ok(())
    }

    /// Send a signal message to the server.
    pub async fn send(&self, message: ClientSignalMessage) -> Result<()> {
        let sender = self.sender.read().await;
        if let Some(ref sender) = *sender {
            sender
                .send(message)
                .map_err(|e| SyncFlowError::Signal(format!("Send failed: {}", e)))?;
        }
        Ok(())
    }
}
