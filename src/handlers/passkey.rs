use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize};
use serde_json::{json, Value};
use sqlx::{SqlitePool};
use std::sync::Arc;
use uuid::Uuid;
use webauthn_rs::prelude::*;
use chrono::Utc;

use crate::{
    auth::{session, AuthUser},
    error::{AppError, AppResult},
    models::{Authenticator, Challenge},
};

#[derive(Debug, Deserialize)]
pub struct PasskeyOptionsQuery {
    pub username: Option<String>,
}

pub async fn register_options(
    State(pool): State<SqlitePool>,
    State(webauthn): State<Arc<Webauthn>>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    tracing::info!("Passkey register options requested for user: {} (ID: {})", user.username, user.id);
    let user_unique_id = Uuid::parse_str(&format!("{:032x}", user.id)).unwrap_or_else(|_| Uuid::new_v4());
    
    let auths = sqlx::query_as::<_, Authenticator>(
        "SELECT * FROM authenticators WHERE userId = ?"
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    let exclude_credentials = auths.iter().map(|a| {
        CredentialID::from(a.public_key.clone())
    }).collect::<Vec<_>>();

    let (options, challenge) = webauthn.start_passkey_registration(
        user_unique_id,
        &user.username,
        &user.name,
        Some(exclude_credentials),
    ).map_err(|e| AppError::Internal(format!("WebAuthn error: {}", e)))?;

    let challenge_json = serde_json::to_string(&challenge).map_err(|e| AppError::Internal(e.to_string()))?;
    let expires_at = (Utc::now() + chrono::Duration::minutes(5)).to_rfc3339();
    let challenge_id = Uuid::new_v4().to_string();

    tracing::info!("Passkey register options generated for user {}: challengeId={}", user.username, challenge_id);

    sqlx::query!(
        "INSERT INTO challenges (id, userId, challenge, expiresAt) VALUES (?, ?, ?, ?)",
        challenge_id, user.id, challenge_json, expires_at
    )
    .execute(&pool)
    .await?;

    let response_json = json!({
        "options": options,
        "challengeId": challenge_id,
    });
    tracing::info!("Passkey register options response body: {}", serde_json::to_string(&response_json).unwrap_or_default());

    Ok(Json(response_json))
}

#[derive(Debug, Deserialize)]
pub struct RegisterVerifyRequest {
    #[serde(rename = "challengeId")]
    pub challenge_id: String,
    pub response: RegisterPublicKeyCredential,
}

pub async fn register_verify(
    State(pool): State<SqlitePool>,
    State(webauthn): State<Arc<Webauthn>>,
    AuthUser(user): AuthUser,
    Json(body): Json<RegisterVerifyRequest>,
) -> AppResult<Json<Value>> {
    tracing::info!("Passkey register verify requested for user: {} (ID: {})", user.username, user.id);
    let challenge_row = sqlx::query_as::<_, Challenge>(
        "SELECT * FROM challenges WHERE id = ? AND userId = ?"
    )
    .bind(&body.challenge_id)
    .bind(user.id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Challenge not found".to_string()))?;

    let challenge: PasskeyRegistration = serde_json::from_str(&challenge_row.challenge)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let passkey = webauthn.finish_passkey_registration(&body.response, &challenge)
        .map_err(|e| AppError::BadRequest(format!("Verification failed: {}", e)))?;

    let auth_id = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, passkey.cred_id().as_slice());
    let public_key = passkey.cred_id().as_slice().to_vec();
    
    sqlx::query!(
        "INSERT INTO authenticators (id, userId, publicKey, counter, deviceType, backedUp, transports) 
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        auth_id, user.id, public_key, 0i64, "passkey", true, None::<String>
    )
    .execute(&pool)
    .await?;

    sqlx::query!("DELETE FROM challenges WHERE id = ?", body.challenge_id).execute(&pool).await?;

    tracing::info!("Passkey registered successfully for user: {} (ID: {})", user.username, user.id);
    Ok(Json(json!({ "verified": true })))
}

pub async fn login_options(
    State(_pool): State<SqlitePool>,
    State(webauthn): State<Arc<Webauthn>>,
    Query(_query): Query<PasskeyOptionsQuery>,
) -> AppResult<Json<Value>> {
    tracing::info!("Passkey login options requested");
    let (options, challenge) = webauthn.start_passkey_authentication(&[])
        .map_err(|e| AppError::Internal(format!("WebAuthn error: {}", e)))?;

    let challenge_json = serde_json::to_string(&challenge).map_err(|e| AppError::Internal(e.to_string()))?;
    let expires_at = (Utc::now() + chrono::Duration::minutes(5)).to_rfc3339();
    let challenge_id = Uuid::new_v4().to_string();

    sqlx::query!(
        "INSERT INTO challenges (id, challenge, expiresAt) VALUES (?, ?, ?)",
        challenge_id, challenge_json, expires_at
    )
    .execute(&_pool)
    .await?;

    Ok(Json(json!({
        "options": options,
        "challengeId": challenge_id,
    })))
}

#[derive(Debug, Deserialize)]
pub struct LoginVerifyRequest {
    #[serde(rename = "challengeId")]
    pub challenge_id: String,
    pub response: PublicKeyCredential,
}

pub async fn login_verify(
    State(pool): State<SqlitePool>,
    State(webauthn): State<Arc<Webauthn>>,
    Json(body): Json<LoginVerifyRequest>,
) -> AppResult<Json<Value>> {
    tracing::info!("Passkey login verify requested");
    let challenge_row = sqlx::query_as::<_, Challenge>(
        "SELECT * FROM challenges WHERE id = ?"
    )
    .bind(&body.challenge_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Challenge not found".to_string()))?;

    let challenge: PasskeyAuthentication = serde_json::from_str(&challenge_row.challenge)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let auth_result = webauthn.finish_passkey_authentication(&body.response, &challenge)
        .map_err(|e| AppError::BadRequest(format!("Verification failed: {}", e)))?;

    let cred_id = auth_result.cred_id();
    let auth_id = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, cred_id.as_slice());

    let auth_row = sqlx::query!(
        "SELECT userId FROM authenticators WHERE id = ?",
        auth_id
    )
    .fetch_optional(&pool)
    .await?;

    let auth_data = auth_row.ok_or_else(|| AppError::Unauthorized)?;
    let user_id = auth_data.userId;

    let token = session::create_session(&pool, user_id).await?;

    sqlx::query!("DELETE FROM challenges WHERE id = ?", body.challenge_id).execute(&pool).await?;

    tracing::info!("Passkey login successful for user ID: {}", user_id);
    Ok(Json(json!({
        "verified": true,
        "token": token,
    })))
}
