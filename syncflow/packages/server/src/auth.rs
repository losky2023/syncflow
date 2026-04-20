use crate::signal::SignalState;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, SaltString},
    Argon2, PasswordHasher, PasswordVerifier,
};
use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};

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

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: String,
}

/// Hash a password using Argon2id.
fn hash_password(password: &str) -> Result<String, StatusCode> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Verify a password against an Argon2id hash.
fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .ok()
        .map(|h| {
            Argon2::default()
                .verify_password(password.as_bytes(), &h)
                .is_ok()
        })
        .unwrap_or(false)
}

/// Generate a JWT token for a user.
fn generate_token(user_id: &str, secret: &str) -> Result<String, StatusCode> {
    let claims = serde_json::json!({
        "sub": user_id,
        "exp": (Utc::now().timestamp() + 86400 * 30) as usize,
    });
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn register(
    State(state): State<SignalState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    let exists = sqlx::query!("SELECT id FROM users WHERE username = ?", req.username)
        .fetch_optional(&state.app.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if exists.is_some() {
        return Err(StatusCode::CONFLICT);
    }

    let password_hash = hash_password(&req.password)?;
    let created_at = Utc::now().to_rfc3339();
    let result = sqlx::query!(
        "INSERT INTO users (username, password_hash, public_key, created_at) VALUES (?, ?, ?, ?)",
        req.username,
        password_hash,
        req.public_key,
        created_at,
    )
    .execute(&state.app.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user_id = result.last_insert_rowid().to_string();
    let token = generate_token(&user_id, &state.app.config.jwt_secret)?;

    Ok(Json(AuthResponse { token, user_id }))
}

pub async fn login(
    State(state): State<SignalState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    let user = sqlx::query!(
        "SELECT id, password_hash FROM users WHERE username = ?",
        req.username
    )
    .fetch_optional(&state.app.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user = user.ok_or(StatusCode::UNAUTHORIZED)?;

    if !verify_password(&req.password, &user.password_hash) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let user_id = user.id.expect("user id should exist");
    let token = generate_token(&user_id.to_string(), &state.app.config.jwt_secret)?;

    Ok(Json(AuthResponse {
        token,
        user_id: user_id.to_string(),
    }))
}
