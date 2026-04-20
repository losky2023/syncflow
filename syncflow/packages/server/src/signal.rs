use axum::{
    extract::ws::{WebSocketUpgrade, WebSocket, Message},
    response::IntoResponse,
    extract::State,
};
use crate::AppState;
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use futures::stream::SplitSink;

pub type DeviceRegistry = Arc<RwLock<HashMap<String, SplitSink<WebSocket, Message>>>>;

#[derive(Clone)]
pub struct SignalState {
    pub app: AppState,
    pub registry: DeviceRegistry,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(_state): State<SignalState>,
) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            axum::extract::ws::Message::Text(text) => {
                tracing::debug!("Signal message: {}", text);
            }
            axum::extract::ws::Message::Close(reason) => {
                tracing::info!("Client disconnected: {:?}", reason);
                break;
            }
            _ => {}
        }
    }
}
