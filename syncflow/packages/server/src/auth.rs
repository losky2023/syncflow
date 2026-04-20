use axum::{Json, extract::State};
use serde::Deserialize;
use crate::signal::SignalState;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub public_key: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(serde::Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: String,
}

pub async fn register(
    State(_state): State<SignalState>,
    Json(_req): Json<RegisterRequest>,
) -> Json<AuthResponse> {
    Json(AuthResponse {
        token: "placeholder".into(),
        user_id: "placeholder".into(),
    })
}

pub async fn login(
    State(_state): State<SignalState>,
    Json(_req): Json<LoginRequest>,
) -> Json<AuthResponse> {
    Json(AuthResponse {
        token: "placeholder".into(),
        user_id: "placeholder".into(),
    })
}
