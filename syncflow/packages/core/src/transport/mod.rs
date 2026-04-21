pub mod discovery;
pub mod signal_client;
pub mod webrtc_peer;

#[cfg(test)]
mod tests;

pub use signal_client::*;
pub use webrtc_peer::*;

use crate::auth::UserSession;
use crate::error::{Result, SyncFlowError};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_tungstenite::tungstenite::Bytes;
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::RTCPeerConnection;

/// Transport layer manages WebRTC connections to peers and signaling.
pub struct TransportLayer {
    signal_url: String,
    session: Arc<UserSession>,
    peers: Arc<RwLock<HashMap<String, Arc<RTCPeerConnection>>>>,
    data_channels: Arc<RwLock<HashMap<String, Arc<RTCDataChannel>>>>,
    event_tx: broadcast::Sender<TransportEvent>,
}

/// Events emitted by the transport layer.
#[derive(Debug, Clone)]
pub enum TransportEvent {
    PeerConnected { device_id: String },
    PeerDisconnected { device_id: String },
    DataReceived { from: String, data: Vec<u8> },
}

impl TransportLayer {
    pub fn new(signal_url: String, session: Arc<UserSession>) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            signal_url,
            session,
            peers: Arc::new(RwLock::new(HashMap::new())),
            data_channels: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
        }
    }

    /// Subscribe to transport events.
    pub fn subscribe(&self) -> broadcast::Receiver<TransportEvent> {
        self.event_tx.subscribe()
    }

    /// Connect to a peer via WebRTC.
    pub async fn connect_to_peer(&self, peer_id: &str) -> Result<()> {
        if self.peers.read().await.contains_key(peer_id) {
            return Ok(());
        }

        let ice_servers = vec!["stun:stun.l.google.com:19302".to_string()];
        let pc = Arc::new(create_peer_connection(&ice_servers).await?);

        // Set up data channel event handler BEFORE creating channels
        let event_tx = self.event_tx.clone();
        let peer_id_str = peer_id.to_string();
        pc.on_data_channel(Box::new(move |dc| {
            let dc = dc.clone();
            let tx = event_tx.clone();
            let pid = peer_id_str.clone();
            Box::pin(async move {
                dc.on_message(Box::new(move |msg| {
                    let tx = tx.clone();
                    let pid = pid.clone();
                    Box::pin(async move {
                        let _ = tx.send(TransportEvent::DataReceived {
                            from: pid,
                            data: msg.data.to_vec(),
                        });
                    })
                }));
            })
        }));

        let dc = create_data_channel(&pc, "syncflow").await?;

        // Create and send SDP offer
        let offer_sdp = create_offer(&pc).await?;
        tracing::debug!(
            "SDP offer created for peer {}: {} chars",
            peer_id,
            offer_sdp.len()
        );

        self.peers.write().await.insert(peer_id.to_string(), pc);
        self.data_channels
            .write()
            .await
            .insert(peer_id.to_string(), dc);

        Ok(())
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

    /// Get list of connected peer IDs.
    pub async fn connected_peers(&self) -> Vec<String> {
        self.peers.read().await.keys().cloned().collect()
    }
}
