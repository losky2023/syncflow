use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use crate::signal::SignalState;

#[derive(Deserialize)]
pub struct DeviceRegisterRequest {
    pub user_id: String,
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub public_key: String,
}

#[derive(Serialize, Debug)]
pub struct DeviceInfoResponse {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub last_seen_at: Option<String>,
}

pub async fn register_device(
    State(state): State<SignalState>,
    Json(req): Json<DeviceRegisterRequest>,
) -> Result<StatusCode, StatusCode> {
    let now = Utc::now().to_rfc3339();

    sqlx::query!(
        r#"
        INSERT INTO server_devices (user_id, device_id, device_name, platform, public_key, last_seen_at, is_online)
        VALUES (?, ?, ?, ?, ?, ?, FALSE)
        ON CONFLICT(device_id) DO UPDATE SET
            device_name = excluded.device_name,
            platform = excluded.platform,
            public_key = excluded.public_key,
            last_seen_at = excluded.last_seen_at,
            is_online = FALSE
        "#,
        req.user_id,
        req.device_id,
        req.device_name,
        req.platform,
        req.public_key,
        now,
    )
    .execute(&state.app.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::CREATED)
}

pub async fn list_devices(
    State(state): State<SignalState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<DeviceInfoResponse>>, StatusCode> {
    let user_id = params.get("user_id").ok_or(StatusCode::BAD_REQUEST)?;

    let rows = sqlx::query!(
        "SELECT device_id, device_name, platform, last_seen_at FROM server_devices WHERE user_id = ?",
        user_id
    )
    .fetch_all(&state.app.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let devices = rows.into_iter().map(|r| DeviceInfoResponse {
        device_id: r.device_id,
        device_name: r.device_name,
        platform: r.platform,
        last_seen_at: r.last_seen_at,
    }).collect();

    Ok(Json(devices))
}
