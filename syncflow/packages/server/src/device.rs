use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use crate::signal::SignalState;

#[derive(Deserialize)]
pub struct DeviceRegisterRequest {
    pub user_id: String,
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub public_key: String,
}

#[derive(Serialize)]
pub struct DeviceInfoResponse {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub last_seen_at: Option<String>,
}

pub async fn register_device(
    State(_state): State<SignalState>,
    Json(_req): Json<DeviceRegisterRequest>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

pub async fn list_devices(
    State(_state): State<SignalState>,
) -> Json<Vec<DeviceInfoResponse>> {
    Json(vec![])
}
