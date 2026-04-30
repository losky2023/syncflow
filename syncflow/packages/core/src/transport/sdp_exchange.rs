use axum::{extract::State, routing::get, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::error::{Result, SyncFlowError};
use crate::transport::TransportLayer;

#[derive(Debug, Deserialize)]
pub struct SdpOfferRequest {
    pub sdp: String,
    pub device_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SdpAnswerResponse {
    pub sdp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SdpDeviceResponse {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub port: u16,
}

/// Shared state for the SDP exchange server.
pub struct SdpServerState {
    pub transport: Arc<TransportLayer>,
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub port: u16,
}

/// Start a local HTTP server for SDP offer/answer exchange.
///
/// Listens on 0.0.0.0:<port> and provides:
/// - POST /sdp/offer — receive an offer, create and return an answer
/// - POST /sdp/answer — receive an answer (for one-way notification)
pub async fn start_sdp_server(
    port: u16,
    device_id: String,
    device_name: String,
    platform: String,
    transport: Arc<TransportLayer>,
) -> Result<tokio::task::JoinHandle<()>> {
    let state = SdpServerState {
        transport,
        device_id,
        device_name,
        platform,
        port,
    };

    let app = Router::new()
        .route("/sdp/device", get(handle_device))
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

async fn handle_device(State(state): State<Arc<SdpServerState>>) -> Json<SdpDeviceResponse> {
    Json(SdpDeviceResponse {
        device_id: state.device_id.clone(),
        device_name: state.device_name.clone(),
        platform: state.platform.clone(),
        port: state.port,
    })
}

async fn handle_offer(
    State(state): State<Arc<SdpServerState>>,
    Json(req): Json<SdpOfferRequest>,
) -> Json<SdpAnswerResponse> {
    match do_handle_offer(&state, &req.device_id, &req.sdp).await {
        Ok(answer_sdp) => Json(SdpAnswerResponse { sdp: answer_sdp }),
        Err(e) => {
            tracing::error!("Failed to handle offer: {}", e);
            Json(SdpAnswerResponse { sdp: String::new() })
        }
    }
}

async fn do_handle_offer(
    state: &SdpServerState,
    remote_device_id: &str,
    sdp: &str,
) -> Result<String> {
    state.transport.accept_offer(remote_device_id, sdp).await
}

async fn handle_answer(
    State(_state): State<Arc<SdpServerState>>,
    Json(req): Json<SdpOfferRequest>,
) -> Json<SdpAnswerResponse> {
    use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

    let answer = RTCSessionDescription::answer(req.sdp.clone())
        .map_err(|e| SyncFlowError::WebRtc(format!("Invalid answer SDP: {}", e)));

    match answer {
        Ok(_answer) => Json(SdpAnswerResponse {
            sdp: "ok".to_string(),
        }),
        Err(e) => {
            tracing::error!("Failed to handle answer: {}", e);
            Json(SdpAnswerResponse { sdp: String::new() })
        }
    }
}
