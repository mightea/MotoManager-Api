use axum::{
    extract::{Query, State},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::sync::Arc;
use uuid::Uuid;
use webauthn_rs::prelude::*;

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
    tracing::info!(
        "Passkey register options requested for user: {} (ID: {})",
        user.username,
        user.id
    );

    let user_unique_id =
        Uuid::parse_str(&format!("{:032x}", user.id)).unwrap_or_else(|_| Uuid::new_v4());

    let auths = sqlx::query_as::<_, Authenticator>("SELECT * FROM authenticators WHERE userId = ?")
        .bind(user.id)
        .fetch_all(&pool)
        .await?;

    let exclude_credentials = auths
        .iter()
        .filter_map(|a| {
            let passkey: Result<Passkey, _> = serde_json::from_slice(&a.public_key);
            passkey.ok().map(|pk| pk.cred_id().clone())
        })
        .collect::<Vec<_>>();

    let (options, challenge) = webauthn
        .start_passkey_registration(
            user_unique_id,
            &user.username,
            &user.name,
            Some(exclude_credentials),
        )
        .map_err(|e| AppError::Internal(format!("WebAuthn error: {}", e)))?;

    let challenge_json =
        serde_json::to_string(&challenge).map_err(|e| AppError::Internal(e.to_string()))?;
    let expires_at = (Utc::now() + chrono::Duration::minutes(5)).to_rfc3339();
    let challenge_id = Uuid::new_v4().to_string();

    tracing::info!(
        "Passkey register options generated for user {}: challengeId={}",
        user.username,
        challenge_id
    );

    sqlx::query!(
        "INSERT INTO challenges (id, userId, challenge, expiresAt) VALUES (?, ?, ?, ?)",
        challenge_id,
        user.id,
        challenge_json,
        expires_at
    )
    .execute(&pool)
    .await?;

    Ok(Json(json!({
        "options": options,
        "challengeId": challenge_id,
    })))
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
    tracing::info!(
        "Passkey register verify requested for user: {} (ID: {})",
        user.username,
        user.id
    );

    let challenge_row =
        sqlx::query_as::<_, Challenge>("SELECT * FROM challenges WHERE id = ? AND userId = ?")
            .bind(&body.challenge_id)
            .bind(user.id)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| AppError::BadRequest("Challenge not found".to_string()))?;

    let challenge: PasskeyRegistration = serde_json::from_str(&challenge_row.challenge)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let passkey = webauthn
        .finish_passkey_registration(&body.response, &challenge)
        .map_err(|e| {
            tracing::error!("Passkey registration verification failed: {}", e);
            AppError::BadRequest(format!("Verification failed: {}", e))
        })?;

    let cred_id = passkey.cred_id();
    let auth_id = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        cred_id.as_slice(),
    );
    let passkey_json =
        serde_json::to_vec(&passkey).map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query!(
        "INSERT INTO authenticators (id, userId, publicKey, counter, deviceType, backedUp, transports) 
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        auth_id, user.id, passkey_json, 0i64, "passkey", true, None::<String>
    )
    .execute(&pool)
    .await?;

    sqlx::query!("DELETE FROM challenges WHERE id = ?", body.challenge_id)
        .execute(&pool)
        .await?;

    tracing::info!(
        "Passkey registered successfully for user: {} (ID: {})",
        user.username,
        user.id
    );
    Ok(Json(json!({ "verified": true })))
}

pub async fn login_options(
    State(pool): State<SqlitePool>,
    State(webauthn): State<Arc<Webauthn>>,
    Query(query): Query<PasskeyOptionsQuery>,
) -> AppResult<Json<Value>> {
    tracing::info!(
        "Passkey login options requested for username: {:?}",
        query.username
    );

    let mut allow_credentials = Vec::new();

    let auths = if let Some(username) = query.username {
        sqlx::query_as::<_, Authenticator>(
            "SELECT a.* FROM authenticators a JOIN users u ON u.id = a.userId WHERE u.username = ?",
        )
        .bind(username)
        .fetch_all(&pool)
        .await?
    } else {
        // If no username, we allow all credentials we know about for this RP
        sqlx::query_as::<_, Authenticator>("SELECT * FROM authenticators")
            .fetch_all(&pool)
            .await?
    };

    for a in auths {
        if let Ok(passkey) = serde_json::from_slice::<Passkey>(&a.public_key) {
            allow_credentials.push(passkey);
        }
    }

    let (options, challenge) = webauthn
        .start_passkey_authentication(&allow_credentials)
        .map_err(|e| AppError::Internal(format!("WebAuthn error: {}", e)))?;

    let challenge_json =
        serde_json::to_string(&challenge).map_err(|e| AppError::Internal(e.to_string()))?;
    let expires_at = (Utc::now() + chrono::Duration::minutes(5)).to_rfc3339();
    let challenge_id = Uuid::new_v4().to_string();

    sqlx::query!(
        "INSERT INTO challenges (id, challenge, expiresAt) VALUES (?, ?, ?)",
        challenge_id,
        challenge_json,
        expires_at
    )
    .execute(&pool)
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

    let challenge_row = sqlx::query_as::<_, Challenge>("SELECT * FROM challenges WHERE id = ?")
        .bind(&body.challenge_id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::BadRequest("Challenge not found".to_string()))?;

    let challenge: PasskeyAuthentication = serde_json::from_str(&challenge_row.challenge)
        .map_err(|e| AppError::Internal(format!("Failed to parse challenge JSON: {}", e)))?;

    // Identify the credential from the response
    let cred_id_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        &body.response.id,
    )
    .map_err(|e| AppError::BadRequest(format!("Invalid credential ID encoding: {}", e)))?;

    let auth_id =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &cred_id_bytes);

    let auth_row = sqlx::query_as::<_, Authenticator>("SELECT * FROM authenticators WHERE id = ?")
        .bind(&auth_id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| {
            tracing::error!("No authenticator found for ID: {}", auth_id);
            AppError::Unauthorized
        })?;

    let auth_result = webauthn
        .finish_passkey_authentication(&body.response, &challenge)
        .map_err(|e| {
            tracing::error!("Passkey login verification failed: {}", e);
            AppError::BadRequest(format!("Verification failed: {}", e))
        })?;

    let new_counter = auth_result.counter();
    sqlx::query!(
        "UPDATE authenticators SET counter = ? WHERE id = ?",
        new_counter,
        auth_id
    )
    .execute(&pool)
    .await?;

    let user_id = auth_row.user_id;
    let token = session::create_session(&pool, user_id).await?;

    sqlx::query!("DELETE FROM challenges WHERE id = ?", body.challenge_id)
        .execute(&pool)
        .await?;

    tracing::info!("Passkey login successful for user ID: {}", user_id);
    Ok(Json(json!({
        "verified": true,
        "token": token,
    })))
}
