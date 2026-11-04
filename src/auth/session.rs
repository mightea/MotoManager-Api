use chrono::{Duration, Utc};
use rand::Rng;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::{
    auth::{IMPERSONATION_DURATION_HOURS, SESSION_DURATION_DAYS},
    error::{AppError, AppResult},
    models::User,
};

/// Generate a random session token: 40 bytes as 80-char hex string.
pub fn generate_session_token() -> String {
    let mut bytes = [0u8; 40];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Raw tokens are 40 random bytes as hex; stored hashes are SHA-256 hex.
/// The lengths differ, which is how [`hash_legacy_session_tokens`] tells
/// old plaintext rows apart from hashed ones.
const RAW_TOKEN_HEX_LEN: i64 = 80;

/// What `sessions.token` stores: the SHA-256 (hex) of the bearer token. The raw
/// token only ever lives on the client, so a leaked database or backup can't
/// be replayed as a login. The token is 320 random bits, so a fast unsalted
/// hash is enough (same scheme as API tokens and OAuth codes).
pub fn hash_session_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// One-time upgrade for sessions created before tokens were hashed: replaces
/// each remaining plaintext token with its hash, so existing logins keep
/// working. Idempotent; run at startup after migrations.
pub async fn hash_legacy_session_tokens(pool: &SqlitePool) -> AppResult<u64> {
    let legacy: Vec<String> =
        sqlx::query_scalar("SELECT token FROM sessions WHERE length(token) = ?")
            .bind(RAW_TOKEN_HEX_LEN)
            .fetch_all(pool)
            .await?;
    let mut tx = pool.begin().await?;
    for token in &legacy {
        sqlx::query("UPDATE sessions SET token = ? WHERE token = ?")
            .bind(hash_session_token(token))
            .bind(token)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(legacy.len() as u64)
}

/// Create a new session for the user. Returns the raw token for the client;
/// only its hash is stored.
pub async fn create_session(pool: &SqlitePool, user_id: i64) -> AppResult<String> {
    let token = generate_session_token();
    let token_hash = hash_session_token(&token);
    let now = Utc::now();
    let expires_at = now + Duration::days(SESSION_DURATION_DAYS);
    let created_at = now.to_rfc3339();
    let expires_at_str = expires_at.to_rfc3339();

    sqlx::query!(
        "INSERT INTO sessions (token, userId, expiresAt, createdAt) VALUES (?, ?, ?, ?)",
        token_hash,
        user_id,
        expires_at_str,
        created_at
    )
    .execute(pool)
    .await?;

    Ok(token)
}

/// Create a short-lived session acting as `user_id`, recording the admin who
/// impersonates. Expires after [`IMPERSONATION_DURATION_HOURS`] instead of the
/// regular two weeks — support access should not outlive the support case.
pub async fn create_impersonation_session(
    pool: &SqlitePool,
    user_id: i64,
    impersonator_id: i64,
) -> AppResult<String> {
    let token = generate_session_token();
    let token_hash = hash_session_token(&token);
    let now = Utc::now();
    let expires_at_str = (now + Duration::hours(IMPERSONATION_DURATION_HOURS)).to_rfc3339();
    let created_at = now.to_rfc3339();

    sqlx::query!(
        "INSERT INTO sessions (token, userId, expiresAt, createdAt, impersonatorId) \
         VALUES (?, ?, ?, ?, ?)",
        token_hash,
        user_id,
        expires_at_str,
        created_at,
        impersonator_id
    )
    .execute(pool)
    .await?;

    Ok(token)
}

/// Look up a user from a session token, checking expiry.
pub async fn get_user_from_token(pool: &SqlitePool, token: &str) -> AppResult<User> {
    let now = Utc::now().to_rfc3339();

    let user = sqlx::query_as::<_, User>(
        "SELECT u.* \
         FROM sessions s \
         JOIN users u ON u.id = s.userId \
         WHERE s.token = ? AND s.expiresAt > ?",
    )
    .bind(hash_session_token(token))
    .bind(&now)
    .fetch_optional(pool)
    .await?;

    user.ok_or(AppError::Unauthorized)
}

/// Delete a session (logout).
pub async fn delete_session(pool: &SqlitePool, token: &str) -> AppResult<()> {
    let token_hash = hash_session_token(token);
    sqlx::query!("DELETE FROM sessions WHERE token = ?", token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_session_token_length() {
        let token = generate_session_token();
        assert_eq!(token.len(), 80); // 40 bytes * 2 hex chars
    }

    #[test]
    fn test_hash_session_token_differs_from_raw_length() {
        let token = generate_session_token();
        let hash = hash_session_token(&token);
        assert_eq!(hash.len(), 64);
        assert_ne!(hash.len() as i64, RAW_TOKEN_HEX_LEN);
        assert_eq!(hash, hash_session_token(&token));
    }

    #[test]
    fn test_generate_session_token_uniqueness() {
        let t1 = generate_session_token();
        let t2 = generate_session_token();
        assert_ne!(t1, t2);
    }
}
