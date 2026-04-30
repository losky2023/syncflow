pub mod discovery;
pub mod sdp_exchange;
pub mod webrtc_peer;

#[cfg(test)]
mod tests;

pub use discovery::{DiscoveredDevice, DiscoveryService};
pub use sdp_exchange::{start_sdp_server, SdpAnswerResponse, SdpDeviceResponse, SdpServerState};
pub use webrtc_peer::*;

use crate::error::{Result, SyncFlowError};
use bytes::Bytes;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use webrtc::data_channel::data_channel_state::RTCDataChannelState;
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
    discovered_devices: Arc<RwLock<Vec<DiscoveredDevice>>>,
    connecting_peers: Arc<RwLock<HashSet<String>>>,
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
            ice_servers: Vec::new(),
            discovered_devices: Arc::new(RwLock::new(Vec::new())),
            connecting_peers: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Register a discovered device from mDNS.
    pub async fn register_discovered_device(&self, device: DiscoveredDevice) {
        let mut devices = self.discovered_devices.write().await;
        // Avoid duplicates
        if !devices.iter().any(|d| d.device_id == device.device_id) {
            devices.push(device);
        }
    }

    /// Get list of discovered (but not necessarily connected) devices.
    pub async fn get_discovered_devices(&self) -> Vec<DiscoveredDevice> {
        self.discovered_devices.read().await.clone()
    }

    /// Subscribe to transport events.
    pub fn subscribe(&self) -> broadcast::Receiver<TransportEvent> {
        self.event_tx.subscribe()
    }

    /// Get list of connected peer IDs.
    pub async fn connected_peers(&self) -> Vec<String> {
        self.data_channels
            .read()
            .await
            .iter()
            .filter(|(_, dc)| dc.ready_state() == RTCDataChannelState::Open)
            .map(|(peer_id, _)| peer_id.clone())
            .collect()
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
        {
            let mut connecting = self.connecting_peers.write().await;
            if connecting.contains(&device.device_id) {
                return Ok(());
            }
            connecting.insert(device.device_id.clone());
        }

        let connect_result = self.connect_peer_inner(device).await;
        self.connecting_peers
            .write()
            .await
            .remove(&device.device_id);
        connect_result
    }

    async fn connect_peer_inner(&self, device: &DiscoveredDevice) -> Result<()> {
        let pc = Arc::new(create_peer_connection(&self.ice_servers).await?);

        // Create data channel
        let dc = create_data_channel(&pc, &self.device_id).await?;
        self.attach_data_channel_handler(&device.device_id, dc.clone())
            .await;
        self.setup_data_channel_handlers(&pc).await;

        // Create SDP offer
        let offer_sdp = create_offer(&pc).await?;

        // Send offer to peer's SDP server
        let client = reqwest::Client::new();
        let url = format!("{}/sdp/offer", device.base_url());
        let body = serde_json::json!({
            "sdp": offer_sdp,
            "device_id": &self.device_id,
        });

        let response = client.post(&url).json(&body).send().await.map_err(|e| {
            SyncFlowError::WebRtc(format!(
                "Failed to send SDP offer to {}: {}",
                device.device_id, e
            ))
        })?;

        let answer: crate::transport::sdp_exchange::SdpAnswerResponse = response
            .json()
            .await
            .map_err(|e| SyncFlowError::WebRtc(format!("Failed to parse SDP answer: {}", e)))?;

        if answer.sdp.is_empty() {
            return Err(SyncFlowError::WebRtc("Empty SDP answer received".into()));
        }

        // Set remote answer
        set_remote_answer(&pc, &answer.sdp).await?;

        // Store peer
        self.peers
            .write()
            .await
            .insert(device.device_id.clone(), pc);

        tracing::info!("Connected to peer {} ({})", device.device_name, device.ip);
        Ok(())
    }

    pub async fn accept_offer(&self, remote_device_id: &str, offer_sdp: &str) -> Result<String> {
        if self.peers.read().await.contains_key(remote_device_id) {
            return Err(SyncFlowError::WebRtc(format!(
                "Peer {} is already connected",
                remote_device_id
            )));
        }

        let pc = Arc::new(create_peer_connection(&self.ice_servers).await?);
        self.setup_data_channel_handlers(&pc).await;

        set_remote_offer(&pc, offer_sdp).await?;
        let answer_sdp = create_answer(&pc).await?;

        self.peers
            .write()
            .await
            .insert(remote_device_id.to_string(), pc);

        Ok(answer_sdp)
    }

    /// Set up data channel event handlers on a peer connection.
    async fn setup_data_channel_handlers(&self, pc: &RTCPeerConnection) {
        let event_tx = self.event_tx.clone();
        let data_channels = self.data_channels.clone();
        let device_id = self.device_id.clone();

        pc.on_data_channel(Box::new(move |dc| {
            let dc = dc.clone();
            let tx = event_tx.clone();
            let channels = data_channels.clone();
            let local_device_id = device_id.clone();
            Box::pin(async move {
                let peer_id = dc.label().to_string();
                let peer_id_for_messages = peer_id.clone();
                let tx_for_messages = tx.clone();
                dc.on_message(Box::new(move |msg| {
                    let tx = tx_for_messages.clone();
                    let pid = peer_id_for_messages.clone();
                    Box::pin(async move {
                        let _ = tx.send(TransportEvent::DataReceived {
                            from: pid,
                            data: msg.data.to_vec(),
                        });
                    })
                }));
                if peer_id != local_device_id {
                    let open_peer_id = peer_id.clone();
                    let open_tx = tx.clone();
                    let open_channels = channels.clone();
                    let open_dc = dc.clone();
                    dc.on_open(Box::new(move || {
                        let peer_id = open_peer_id.clone();
                        let tx = open_tx.clone();
                        let channels = open_channels.clone();
                        let dc = open_dc.clone();
                        Box::pin(async move {
                            channels.write().await.insert(peer_id.clone(), dc);
                            let _ = tx.send(TransportEvent::PeerConnected { device_id: peer_id });
                        })
                    }));
                }
            })
        }));
    }

    async fn attach_data_channel_handler(&self, peer_id: &str, dc: Arc<RTCDataChannel>) {
        let event_tx = self.event_tx.clone();
        let pid = peer_id.to_string();
        let channels = self.data_channels.clone();
        let dc_for_open = dc.clone();
        let pid_for_open = pid.clone();
        let tx_for_open = event_tx.clone();

        dc.on_open(Box::new(move || {
            let peer_id = pid_for_open.clone();
            let tx = tx_for_open.clone();
            let channels = channels.clone();
            let dc = dc_for_open.clone();
            Box::pin(async move {
                channels.write().await.insert(peer_id.clone(), dc);
                let _ = tx.send(TransportEvent::PeerConnected { device_id: peer_id });
            })
        }));

        dc.on_message(Box::new(move |msg| {
            let tx = event_tx.clone();
            let from = pid.clone();
            Box::pin(async move {
                let _ = tx.send(TransportEvent::DataReceived {
                    from,
                    data: msg.data.to_vec(),
                });
            })
        }));
    }
}
