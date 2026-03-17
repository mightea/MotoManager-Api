use chrono::{Duration, Utc};
use rand::RngCore;
use sqlx::{SqlitePool};

use crate::{
    auth::SESSION_DURATION_DAYS,
    error::{AppError, AppResult},
    models::User,
};

/// Generate a random session token: 40 bytes as 80-char hex string.
pub fn generate_session_token() -> String {
    let mut bytes = [0u8; 40];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Create a new session for the user.
pub async fn create_session(pool: &SqlitePool, user_id: i64) -> AppResult<String> {
    let token = generate_session_token();
    let now = Utc::now();
    let expires_at = now + Duration::days(SESSION_DURATION_DAYS);
    let created_at = now.to_rfc3339();
    let expires_at_str = expires_at.to_rfc3339();

    sqlx::query!(
        "INSERT INTO sessions (token, userId, expiresAt, createdAt) VALUES (?, ?, ?, ?)",
        token, user_id, expires_at_str, created_at
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
    .bind(token)
    .bind(&now)
    .fetch_optional(pool)
    .await?;

    user.ok_or(AppError::Unauthorized)
}

/// Delete a session (logout).
pub async fn delete_session(pool: &SqlitePool, token: &str) -> AppResult<()> {
    sqlx::query!("DELETE FROM sessions WHERE token = ?", token)
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
    fn test_generate_session_token_uniqueness() {
        let t1 = generate_session_token();
        let t2 = generate_session_token();
        assert_ne!(t1, t2);
    }
}
