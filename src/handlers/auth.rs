use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::CookieJar;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Row, SqlitePool};

use crate::{
    auth::{
        password::{hash_password, verify_password},
        session::{create_session, delete_session},
        AuthUser, SESSION_COOKIE_NAME, SESSION_DURATION_DAYS,
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

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user: PublicUser,
    pub token: String,
}

fn make_session_cookie(token: &str, secure: bool) -> String {
    let expires = chrono::Utc::now() + chrono::Duration::days(SESSION_DURATION_DAYS);
    let expires_str = expires.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
    let secure_flag = if secure { "; Secure" } else { "" };
    format!(
        "{}={}; HttpOnly; SameSite=Lax{}; Path=/; Expires={}",
        SESSION_COOKIE_NAME, token, secure_flag, expires_str
    )
}

fn row_to_user(row: &sqlx::sqlite::SqliteRow) -> User {
    User {
        id: row.get("id"),
        email: row.get("email"),
        username: row.get("username"),
        name: row.get("name"),
        password_hash: row.get("password_hash"),
        role: row.get("role"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        last_login_at: row.get("last_login_at"),
    }
}

pub async fn login(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    Json(body): Json<LoginRequest>,
) -> AppResult<Response> {
    let row = sqlx::query(
        "SELECT id, email, username, name, password_hash, role, created_at, updated_at, last_login_at \
         FROM users WHERE email = ? OR username = ?",
    )
    .bind(&body.identifier)
    .bind(&body.identifier)
    .fetch_optional(&pool)
    .await?;

    let user = match row {
        Some(r) => row_to_user(&r),
        None => return Err(AppError::Unauthorized),
    };

    let valid = verify_password(&body.password, &user.password_hash)?;
    if !valid {
        return Err(AppError::Unauthorized);
    }

    // Update last_login_at
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE users SET last_login_at = ? WHERE id = ?")
        .bind(&now)
        .bind(user.id)
        .execute(&pool)
        .await?;

    let token = create_session(&pool, user.id).await?;
    let secure = config.origin.starts_with("https://");
    let cookie = make_session_cookie(&token, secure);

    let public_user = PublicUser::from(user);
    let body = json!({ "user": public_user, "token": token });

    Ok((
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(body),
    )
        .into_response())
}

pub async fn logout(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    jar: CookieJar,
) -> AppResult<Response> {
    if let Some(cookie) = jar.get(SESSION_COOKIE_NAME) {
        let token = cookie.value().to_string();
        let _ = delete_session(&pool, &token).await;
    }

    let secure = config.origin.starts_with("https://");
    let secure_flag = if secure { "; Secure" } else { "" };
    let clear_cookie = format!(
        "{}=; HttpOnly; SameSite=Lax{}; Path=/; Max-Age=0",
        SESSION_COOKIE_NAME, secure_flag
    );

    Ok((
        StatusCode::OK,
        [(header::SET_COOKIE, clear_cookie)],
        Json(json!({ "message": "Logged out" })),
    )
        .into_response())
}

pub async fn me(AuthUser(user): AuthUser) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(json!({ "user": PublicUser::from(user) })))
}

pub async fn register(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    Json(body): Json<RegisterRequest>,
) -> AppResult<Response> {
    // Check if registration is allowed
    let user_count: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM users")
        .fetch_one(&pool)
        .await?
        .get("cnt");

    if user_count > 0 && !config.enable_registration {
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

    // Check for existing user
    let existing: i64 = sqlx::query(
        "SELECT COUNT(*) as cnt FROM users WHERE email = ? OR username = ?",
    )
    .bind(&body.email)
    .bind(&body.username)
    .fetch_one(&pool)
    .await?
    .get("cnt");

    if existing > 0 {
        return Err(AppError::Conflict(
            "Email or username already in use".to_string(),
        ));
    }

    let password_hash = hash_password(&body.password)?;
    let now = Utc::now().to_rfc3339();
    let role = if user_count == 0 { "admin" } else { "user" };

    let user_id = sqlx::query(
        "INSERT INTO users (email, username, name, password_hash, role, created_at, updated_at) \
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

    // Create default settings
    sqlx::query("INSERT OR IGNORE INTO user_settings (user_id, updated_at) VALUES (?, ?)")
        .bind(user_id)
        .bind(&now)
        .execute(&pool)
        .await?;

    let token = create_session(&pool, user_id).await?;
    let secure = config.origin.starts_with("https://");
    let cookie = make_session_cookie(&token, secure);

    let user_row = sqlx::query(
        "SELECT id, email, username, name, password_hash, role, created_at, updated_at, last_login_at \
         FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    let public_user = PublicUser::from(row_to_user(&user_row));
    let resp_body = json!({ "user": public_user, "token": token });

    Ok((
        StatusCode::CREATED,
        [(header::SET_COOKIE, cookie)],
        Json(resp_body),
    )
        .into_response())
}
