use axum::{extract::State, routing::post, Json, Router};
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

#[derive(Debug, Serialize, Deserialize)]
pub struct SdpAnswerResponse {
    pub sdp: String,
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
    let listener = TcpListener::bind(&addr).await.map_err(|e| {
        SyncFlowError::Signal(format!("Failed to bind SDP server on {}: {}", addr, e))
    })?;

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

async fn do_handle_offer(state: &SdpServerState, sdp: &str) -> Result<String> {
    use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

    let offer = RTCSessionDescription::offer(sdp.to_string())
        .map_err(|e| SyncFlowError::WebRtc(format!("Invalid offer SDP: {}", e)))?;

    state
        .peer_connection
        .set_remote_description(offer)
        .await
        .map_err(|e| SyncFlowError::WebRtc(format!("Failed to set remote description: {}", e)))?;

    let answer = state
        .peer_connection
        .create_answer(None)
        .await
        .map_err(|e| SyncFlowError::WebRtc(format!("Failed to create answer: {}", e)))?;

    state
        .peer_connection
        .set_local_description(answer.clone())
        .await
        .map_err(|e| SyncFlowError::WebRtc(format!("Failed to set local description: {}", e)))?;

    Ok(answer.sdp)
}

async fn handle_answer(
    State(state): State<Arc<SdpServerState>>,
    Json(req): Json<SdpOfferRequest>,
) -> Json<SdpAnswerResponse> {
    use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

    let answer = RTCSessionDescription::answer(req.sdp.clone())
        .map_err(|e| SyncFlowError::WebRtc(format!("Invalid answer SDP: {}", e)));

    match answer {
        Ok(answer) => {
            let _ = state.peer_connection.set_remote_description(answer).await;
            Json(SdpAnswerResponse {
                sdp: "ok".to_string(),
            })
        }
        Err(e) => {
            tracing::error!("Failed to handle answer: {}", e);
            Json(SdpAnswerResponse { sdp: String::new() })
        }
    }
}
