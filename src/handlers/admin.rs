use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::{
    auth::{password::hash_password, AdminUser},
    error::{AppError, AppResult},
    models::PublicUser,
};

// ─── User Management ─────────────────────────────────────────────────────────

fn user_row_to_value(row: &sqlx::sqlite::SqliteRow) -> crate::models::User {
    crate::models::User {
        id: row.get("id"),
        email: row.get("email"),
        username: row.get("username"),
        name: row.get("name"),
        password_hash: row.get("passwordHash"),
        role: row.get("role"),
        created_at: row.get("createdAt"),
        updated_at: row.get("updatedAt"),
        last_login_at: row.get("lastLoginAt"),
    }
}

pub async fn list_users(
    State(pool): State<SqlitePool>,
    AdminUser(_admin): AdminUser,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query(
        "SELECT id, email, username, name, passwordHash, role, createdAt, updatedAt, lastLoginAt \
         FROM users ORDER BY createdAt ASC",
    )
    .fetch_all(&pool)
    .await?;

    let mut users = Vec::new();
    for row in &rows {
        let user_id: i64 = row.get("id");
        let settings_row = sqlx::query(
            "SELECT id, userId, tireInterval, batteryLithiumInterval, batteryDefaultInterval, \
             engineOilInterval, gearboxOilInterval, finalDriveOilInterval, forkOilInterval, \
             brakeFluidInterval, coolantInterval, chainInterval, tireKmInterval, \
             engineOilKmInterval, gearboxOilKmInterval, finalDriveOilKmInterval, \
             forkOilKmInterval, brakeFluidKmInterval, coolantKmInterval, chainKmInterval, \
             updatedAt FROM userSettings WHERE userId = ?",
        )
        .bind(user_id)
        .fetch_optional(&pool)
        .await?;

        let settings = settings_row.map(|sr| {
            json!({
                "id": sr.get::<i64, _>("id"),
                "userId": sr.get::<i64, _>("userId"),
                "tireInterval": sr.get::<i64, _>("tireInterval"),
                "batteryLithiumInterval": sr.get::<i64, _>("batteryLithiumInterval"),
                "batteryDefaultInterval": sr.get::<i64, _>("batteryDefaultInterval"),
                "engineOilInterval": sr.get::<i64, _>("engineOilInterval"),
                "gearboxOilInterval": sr.get::<i64, _>("gearboxOilInterval"),
                "finalDriveOilInterval": sr.get::<i64, _>("finalDriveOilInterval"),
                "forkOilInterval": sr.get::<i64, _>("forkOilInterval"),
                "brakeFluidInterval": sr.get::<i64, _>("brakeFluidInterval"),
                "coolantInterval": sr.get::<i64, _>("coolantInterval"),
                "chainInterval": sr.get::<i64, _>("chainInterval"),
                "tireKmInterval": sr.get::<Option<i64>, _>("tireKmInterval"),
                "engineOilKmInterval": sr.get::<Option<i64>, _>("engineOilKmInterval"),
                "gearboxOilKmInterval": sr.get::<Option<i64>, _>("gearboxOilKmInterval"),
                "finalDriveOilKmInterval": sr.get::<Option<i64>, _>("finalDriveOilKmInterval"),
                "forkOilKmInterval": sr.get::<Option<i64>, _>("forkOilKmInterval"),
                "brakeFluidKmInterval": sr.get::<Option<i64>, _>("brakeFluidKmInterval"),
                "coolantKmInterval": sr.get::<Option<i64>, _>("coolantKmInterval"),
                "chainKmInterval": sr.get::<Option<i64>, _>("chainKmInterval"),
                "updatedAt": sr.get::<Option<String>, _>("updatedAt"),
            })
        });

        let user = user_row_to_value(row);
        let pub_user = PublicUser::from(user);
        users.push(json!({
            "id": user_id,
            "email": pub_user.email,
            "username": pub_user.username,
            "name": pub_user.name,
            "role": pub_user.role,
            "createdAt": pub_user.created_at,
            "updatedAt": pub_user.updated_at,
            "lastLoginAt": pub_user.last_login_at,
            "settings": settings,
        }));
    }

    Ok(Json(json!({ "users": users })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserRequest {
    pub email: String,
    pub username: String,
    pub name: String,
    pub password: String,
    pub role: Option<String>,
}

pub async fn create_user(
    State(pool): State<SqlitePool>,
    AdminUser(admin): AdminUser,
    Json(body): Json<CreateUserRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    tracing::info!("Admin {} (ID: {}) creating user: {}", admin.username, admin.id, body.email);
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
    let role = body.role.unwrap_or_else(|| "user".to_string());

    let user_id = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role, createdAt, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&body.email)
    .bind(&body.username)
    .bind(&body.name)
    .bind(&password_hash)
    .bind(&role)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    sqlx::query("INSERT OR IGNORE INTO userSettings (userId, updatedAt) VALUES (?, ?)")
        .bind(user_id)
        .bind(&now)
        .execute(&pool)
        .await?;

    let row = sqlx::query(
        "SELECT id, email, username, name, passwordHash, role, createdAt, updatedAt, lastLoginAt \
         FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    tracing::info!("User created by admin: {} (ID: {})", body.email, user_id);
    Ok((
        StatusCode::CREATED,
        Json(json!({ "user": PublicUser::from(user_row_to_value(&row)) })),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub username: Option<String>,
    pub name: Option<String>,
    pub password: Option<String>,
    pub role: Option<String>,
}

pub async fn update_user(
    State(pool): State<SqlitePool>,
    AdminUser(admin): AdminUser,
    Path(uid): Path<i64>,
    Json(body): Json<UpdateUserRequest>,
) -> AppResult<Json<Value>> {
    tracing::info!("Admin {} (ID: {}) updating user ID: {}", admin.username, admin.id, uid);
    let existing = sqlx::query(
        "SELECT id, email, username, name, passwordHash, role, createdAt, updatedAt, lastLoginAt \
         FROM users WHERE id = ?",
    )
    .bind(uid)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let email = body.email.unwrap_or_else(|| existing.get("email"));
    let username = body.username.unwrap_or_else(|| existing.get("username"));
    let name = body.name.unwrap_or_else(|| existing.get("name"));
    let role = body.role.unwrap_or_else(|| existing.get("role"));
    let now = Utc::now().to_rfc3339();

    let password_hash = if let Some(new_pw) = body.password {
        hash_password(&new_pw)?
    } else {
        existing.get("passwordHash")
    };

    sqlx::query(
        "UPDATE users SET email = ?, username = ?, name = ?, passwordHash = ?, role = ?, updatedAt = ? \
         WHERE id = ?",
    )
    .bind(&email)
    .bind(&username)
    .bind(&name)
    .bind(&password_hash)
    .bind(&role)
    .bind(&now)
    .bind(uid)
    .execute(&pool)
    .await?;

    let row = sqlx::query(
        "SELECT id, email, username, name, passwordHash, role, createdAt, updatedAt, lastLoginAt \
         FROM users WHERE id = ?",
    )
    .bind(uid)
    .fetch_one(&pool)
    .await?;

    tracing::info!("User updated by admin ID: {}", uid);
    Ok(Json(json!({ "user": PublicUser::from(user_row_to_value(&row)) })))
}

pub async fn delete_user(
    State(pool): State<SqlitePool>,
    AdminUser(admin): AdminUser,
    Path(uid): Path<i64>,
) -> AppResult<Json<Value>> {
    tracing::info!("Admin {} (ID: {}) deleting user ID: {}", admin.username, admin.id, uid);
    let result = sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(uid)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        tracing::warn!("Admin delete failed: user ID: {} not found", uid);
        return Err(AppError::NotFound("User not found".to_string()));
    }

    tracing::info!("User deleted by admin ID: {}", uid);
    Ok(Json(json!({ "message": "User deleted" })))
}

// ─── Currency Management ──────────────────────────────────────────────────────

fn currency_row_to_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": r.get::<i64, _>("id"),
        "code": r.get::<String, _>("code"),
        "symbol": r.get::<String, _>("symbol"),
        "label": r.get::<Option<String>, _>("label"),
        "conversionFactor": r.get::<f64, _>("conversionFactor"),
        "createdAt": r.get::<String, _>("createdAt"),
    })
}

pub async fn list_currencies(
    State(pool): State<SqlitePool>,
    AdminUser(_admin): AdminUser,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query(
        "SELECT id, code, symbol, label, conversionFactor, createdAt \
         FROM currencies ORDER BY code ASC",
    )
    .fetch_all(&pool)
    .await?;

    let currencies: Vec<Value> = rows.iter().map(currency_row_to_value).collect();
    Ok(Json(json!({ "currencies": currencies })))
}

pub async fn list_currencies_public(
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query(
        "SELECT id, code, symbol, label, conversionFactor, createdAt \
         FROM currencies ORDER BY code ASC",
    )
    .fetch_all(&pool)
    .await?;

    let currencies: Vec<Value> = rows.iter().map(currency_row_to_value).collect();
    Ok(Json(json!({ "currencies": currencies })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCurrencyRequest {
    pub code: String,
    pub symbol: String,
    pub label: Option<String>,
}

pub async fn create_currency(
    State(pool): State<SqlitePool>,
    AdminUser(_admin): AdminUser,
    Json(body): Json<CreateCurrencyRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    let existing: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM currencies WHERE code = ?")
        .bind(&body.code)
        .fetch_one(&pool)
        .await?
        .get("cnt");

    if existing > 0 {
        return Err(AppError::Conflict(format!(
            "Currency with code '{}' already exists",
            body.code
        )));
    }

    let conversion_factor = fetch_conversion_factor(&body.code).await.unwrap_or(1.0);
    let now = Utc::now().to_rfc3339();

    let id = sqlx::query(
        "INSERT INTO currencies (code, symbol, label, conversionFactor, createdAt) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&body.code)
    .bind(&body.symbol)
    .bind(&body.label)
    .bind(conversion_factor)
    .bind(&now)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    let row = sqlx::query(
        "SELECT id, code, symbol, label, conversionFactor, createdAt FROM currencies WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "currency": currency_row_to_value(&row) })),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCurrencyRequest {
    pub code: Option<String>,
    pub symbol: Option<String>,
    pub label: Option<String>,
    pub conversion_factor: Option<f64>,
}

pub async fn update_currency(
    State(pool): State<SqlitePool>,
    AdminUser(_admin): AdminUser,
    Path(cid): Path<i64>,
    Json(body): Json<UpdateCurrencyRequest>,
) -> AppResult<Json<Value>> {
    let existing = sqlx::query(
        "SELECT id, code, symbol, label, conversionFactor, createdAt FROM currencies WHERE id = ?",
    )
    .bind(cid)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Currency not found".to_string()))?;

    let code = body.code.unwrap_or_else(|| existing.get("code"));
    let symbol = body.symbol.unwrap_or_else(|| existing.get("symbol"));
    let label: Option<String> = body.label.or_else(|| existing.get("label"));
    let conversion_factor = body
        .conversion_factor
        .unwrap_or_else(|| existing.get("conversionFactor"));

    sqlx::query(
        "UPDATE currencies SET code = ?, symbol = ?, label = ?, conversionFactor = ? WHERE id = ?",
    )
    .bind(&code)
    .bind(&symbol)
    .bind(&label)
    .bind(conversion_factor)
    .bind(cid)
    .execute(&pool)
    .await?;

    let row = sqlx::query(
        "SELECT id, code, symbol, label, conversionFactor, createdAt FROM currencies WHERE id = ?",
    )
    .bind(cid)
    .fetch_one(&pool)
    .await?;

    Ok(Json(json!({ "currency": currency_row_to_value(&row) })))
}

pub async fn delete_currency(
    State(pool): State<SqlitePool>,
    AdminUser(_admin): AdminUser,
    Path(cid): Path<i64>,
) -> AppResult<Json<Value>> {
    let result = sqlx::query("DELETE FROM currencies WHERE id = ?")
        .bind(cid)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Currency not found".to_string()));
    }

    Ok(Json(json!({ "message": "Currency deleted" })))
}

// ─── Exchange Rate Helper ─────────────────────────────────────────────────────

async fn fetch_conversion_factor(currency_code: &str) -> Option<f64> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let host = "api.frankfurter.app";
    let path = format!("/latest?from=CHF&to={}", currency_code);
    let request = format!(
        "GET {} HTTP/1.0\r\nHost: {}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        path, host
    );

    let mut stream = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        TcpStream::connect((host, 80u16)),
    )
    .await
    .ok()?
    .ok()?;

    stream.write_all(request.as_bytes()).await.ok()?;

    let mut response = Vec::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        stream.read_to_end(&mut response),
    )
    .await
    .ok()?
    .ok()?;

    let response_str = String::from_utf8_lossy(&response);
    let body = response_str.split("\r\n\r\n").nth(1)?;
    let json: serde_json::Value = serde_json::from_str(body.trim()).ok()?;
    let rate = json.get("rates")?.get(currency_code)?.as_f64()?;

    if rate > 0.0 {
        Some(1.0 / rate)
    } else {
        None
    }
}
