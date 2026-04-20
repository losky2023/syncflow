use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub stun_servers: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".into(),
            port: 3000,
            database_url: "sqlite:signal.db".into(),
            jwt_secret: "change-me-in-production".into(),
            stun_servers: vec!["stun:stun.l.google.com:19302".into()],
        }
    }
}

impl ServerConfig {
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("SYNCFLOW_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: std::env::var("SYNCFLOW_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            database_url: std::env::var("SYNCFLOW_DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:signal.db".into()),
            jwt_secret: std::env::var("SYNCFLOW_JWT_SECRET")
                .unwrap_or_else(|_| "change-me-in-production".into()),
            stun_servers: std::env::var("SYNCFLOW_STUN_SERVERS")
                .ok()
                .map(|s| s.split(',').map(String::from).collect())
                .unwrap_or_else(|| vec!["stun:stun.l.google.com:19302".into()]),
        }
    }
}
