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
    #[allow(dead_code)]
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

    /// Connect to a discovered peer by initiating an SDP offer.
    pub async fn connect_peer(&self, device: &DiscoveredDevice) -> Result<()> {
        if self.peers.read().await.contains_key(&device.device_id) {
            return Ok(());
        }

        let pc = Arc::new(create_peer_connection(&self.ice_servers).await?);

        // Set up data channel event handler
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
                let peer_id = dc.label().to_string();
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
