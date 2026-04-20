use axum::{Json, extract::State};
use serde::Serialize;
use crate::signal::SignalState;

#[derive(Serialize)]
pub struct StunConfig {
    pub ice_servers: Vec<IceServer>,
}

#[derive(Serialize)]
pub struct IceServer {
    pub urls: Vec<String>,
}

pub async fn get_config(
    State(state): State<SignalState>,
) -> Json<StunConfig> {
    let urls = state.app.config.stun_servers.clone();
    Json(StunConfig {
        ice_servers: vec![IceServer { urls }],
    })
}
