use chrono::{Duration, Utc};
use rand::RngCore;
use sqlx::{Row, SqlitePool};

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

    sqlx::query(
        "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&token)
    .bind(user_id)
    .bind(&expires_at_str)
    .bind(&created_at)
    .execute(pool)
    .await?;

    Ok(token)
}

/// Look up a user from a session token, checking expiry.
pub async fn get_user_from_token(pool: &SqlitePool, token: &str) -> AppResult<User> {
    let now = Utc::now().to_rfc3339();

    let row = sqlx::query(
        "SELECT u.id, u.email, u.username, u.name, u.password_hash, u.role, \
         u.created_at, u.updated_at, u.last_login_at \
         FROM sessions s \
         JOIN users u ON u.id = s.user_id \
         WHERE s.token = ? AND s.expires_at > ?",
    )
    .bind(token)
    .bind(&now)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(User {
            id: r.get("id"),
            email: r.get("email"),
            username: r.get("username"),
            name: r.get("name"),
            password_hash: r.get("password_hash"),
            role: r.get("role"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            last_login_at: r.get("last_login_at"),
        }),
        None => Err(AppError::Unauthorized),
    }
}

/// Delete a session (logout).
pub async fn delete_session(pool: &SqlitePool, token: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM sessions WHERE token = ?")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}
