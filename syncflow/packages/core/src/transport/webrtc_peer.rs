use std::sync::Arc;

use crate::error::{Result, SyncFlowError};
use webrtc::api::APIBuilder;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

/// Create a new RTCPeerConnection with the given ICE servers.
pub async fn create_peer_connection(ice_servers: &[String]) -> Result<RTCPeerConnection> {
    let config = RTCConfiguration {
        ice_servers: ice_servers
            .iter()
            .map(|url| RTCIceServer {
                urls: vec![url.clone()],
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };

    let api = APIBuilder::new().build();
    let pc = api
        .new_peer_connection(config)
        .await
        .map_err(|e| SyncFlowError::WebRtc(format!("Failed to create peer connection: {}", e)))?;

    Ok(pc)
}

/// Create a data channel on the peer connection.
pub async fn create_data_channel(
    pc: &RTCPeerConnection,
    label: &str,
) -> Result<Arc<RTCDataChannel>> {
    let dc = pc
        .create_data_channel(label, None)
        .await
        .map_err(|e| SyncFlowError::WebRtc(format!("Failed to create data channel: {}", e)))?;

    Ok(dc)
}

/// Create an SDP offer and set it as local description.
pub async fn create_offer(pc: &RTCPeerConnection) -> Result<String> {
    let offer = pc
        .create_offer(None)
        .await
        .map_err(|e| SyncFlowError::WebRtc(format!("Failed to create offer: {}", e)))?;

    pc.set_local_description(offer.clone())
        .await
        .map_err(|e| SyncFlowError::WebRtc(format!("Failed to set local description: {}", e)))?;

    Ok(offer.sdp)
}

/// Set remote description from an SDP answer.
pub async fn set_remote_answer(pc: &RTCPeerConnection, sdp: &str) -> Result<()> {
    let answer = RTCSessionDescription::answer(sdp.to_string())
        .map_err(|e| SyncFlowError::WebRtc(format!("Invalid answer SDP: {}", e)))?;

    pc.set_remote_description(answer)
        .await
        .map_err(|e| SyncFlowError::WebRtc(format!("Failed to set remote description: {}", e)))?;

    Ok(())
}

/// Set remote description from an SDP offer (callee side).
pub async fn set_remote_offer(pc: &RTCPeerConnection, sdp: &str) -> Result<()> {
    let offer = RTCSessionDescription::offer(sdp.to_string())
        .map_err(|e| SyncFlowError::WebRtc(format!("Invalid offer SDP: {}", e)))?;

    pc.set_remote_description(offer)
        .await
        .map_err(|e| SyncFlowError::WebRtc(format!("Failed to set remote description: {}", e)))?;

    Ok(())
}

/// Create an SDP answer and set it as local description.
pub async fn create_answer(pc: &RTCPeerConnection) -> Result<String> {
    let answer = pc
        .create_answer(None)
        .await
        .map_err(|e| SyncFlowError::WebRtc(format!("Failed to create answer: {}", e)))?;

    pc.set_local_description(answer.clone())
        .await
        .map_err(|e| SyncFlowError::WebRtc(format!("Failed to set local description: {}", e)))?;

    Ok(answer.sdp)
}
