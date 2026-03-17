use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use sqlx::{SqlitePool};

use crate::{
    auth::{
        extract_bearer_token,
        password::{hash_password, verify_password},
        session::{create_session, delete_session},
        AuthUser,
    },
    config::Config,
    error::{AppError, AppResult},
    models::{PublicUser, User},
};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub identifier: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub name: String,
    pub email: String,
    pub username: String,
    pub password: String,
    #[serde(rename = "confirmPassword")]
    pub confirm_password: String,
}

pub async fn login(
    State(pool): State<SqlitePool>,
    Json(body): Json<LoginRequest>,
) -> AppResult<Response> {
    tracing::info!("Login attempt for identifier: {}", body.identifier);
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE ? IN (email, username)",
    )
    .bind(&body.identifier)
    .fetch_optional(&pool)
    .await?;

    let user = match user {
        Some(u) => u,
        None => {
            tracing::warn!(
                "Login failed: user not found for identifier: {}",
                body.identifier
            );
            return Err(AppError::Unauthorized);
        }
    };

    if !verify_password(&body.password, &user.password_hash)? {
        tracing::warn!(
            "Login failed: incorrect password for user: {}",
            user.username
        );
        return Err(AppError::Unauthorized);
    }

    let now = Utc::now().to_rfc3339();

    sqlx::query!(
        "UPDATE users SET lastLoginAt = ? WHERE id = ?",
        now, user.id
    )
    .execute(&pool)
    .await?;

    let token = create_session(&pool, user.id).await?;
    let public_user = PublicUser::from(user);

    tracing::info!(
        "User logged in: {} (ID: {})",
        public_user.username,
        public_user.id
    );
    Ok((
        StatusCode::OK,
        Json(json!({ "user": public_user, "token": token })),
    )
        .into_response())
}

pub async fn logout(State(pool): State<SqlitePool>, headers: HeaderMap) -> AppResult<Response> {
    if let Some(token) = extract_bearer_token(&headers) {
        tracing::info!(
            "Logout attempt with token prefix: {}",
            &token[..std::cmp::min(token.len(), 8)]
        );
        let _ = delete_session(&pool, &token).await;
    }
    Ok((StatusCode::OK, Json(json!({ "message": "Logged out" }))).into_response())
}

pub async fn me(AuthUser(user): AuthUser) -> AppResult<Json<serde_json::Value>> {
    tracing::debug!(
        "User info requested for: {} (ID: {})",
        user.username,
        user.id
    );
    Ok(Json(json!({ "user": PublicUser::from(user) })))
}

pub async fn register(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    Json(body): Json<RegisterRequest>,
) -> AppResult<Response> {
    tracing::info!(
        "Registration attempt for email: {}, username: {}",
        body.email,
        body.username
    );
    let user_count: i64 = sqlx::query!("SELECT COUNT(*) as cnt FROM users")
        .fetch_one(&pool)
        .await?
        .cnt as i64;

    if user_count > 0 && !config.enable_registration {
        tracing::warn!("Registration attempt while registration is disabled");
        return Err(AppError::Forbidden);
    }

    if body.password != body.confirm_password {
        return Err(AppError::BadRequest("Passwords do not match".to_string()));
    }

    if body.password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    let existing: i64 =
        sqlx::query!("SELECT COUNT(*) as cnt FROM users WHERE email = ? OR username = ?", body.email, body.username)
            .fetch_one(&pool)
            .await?
            .cnt as i64;

    if existing > 0 {
        return Err(AppError::Conflict(
            "Email or username already in use".to_string(),
        ));
    }

    let password_hash = hash_password(&body.password)?;
    let now = Utc::now().to_rfc3339();
    let role = if user_count == 0 { "admin" } else { "user" };

    let user_id = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role, createdAt, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&body.email)
    .bind(&body.username)
    .bind(&body.name)
    .bind(&password_hash)
    .bind(role)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    sqlx::query!("INSERT OR IGNORE INTO userSettings (userId, updatedAt) VALUES (?, ?)", user_id, now)
        .execute(&pool)
        .await?;

    let token = create_session(&pool, user_id).await?;

    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    let public_user = PublicUser::from(user);

    tracing::info!(
        "User registered: {} (ID: {}, role: {})",
        public_user.username,
        public_user.id,
        role
    );
    Ok((
        StatusCode::CREATED,
        Json(json!({ "user": public_user, "token": token })),
    )
        .into_response())
}

pub async fn status(State(pool): State<SqlitePool>) -> AppResult<Json<serde_json::Value>> {
    let user_count: i64 = sqlx::query!("SELECT COUNT(*) as cnt FROM users")
        .fetch_one(&pool)
        .await?
        .cnt as i64;
    Ok(Json(json!({ "userCount": user_count })))
}
