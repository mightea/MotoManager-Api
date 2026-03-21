use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use pdfium_render::prelude::*;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    auth::{password::hash_password, AdminUser},
    config::Config,
    error::{AppError, AppResult},
    models::{CurrencySetting, PublicUser, User, UserSettings},
};

// ─── User Management ─────────────────────────────────────────────────────────

pub async fn list_users(
    State(pool): State<SqlitePool>,
    AdminUser(_admin): AdminUser,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY createdAt ASC")
        .fetch_all(&pool)
        .await?;

    let mut users = Vec::new();
    for user in rows {
        let settings =
            sqlx::query_as::<_, UserSettings>("SELECT * FROM userSettings WHERE userId = ?")
                .bind(user.id)
                .fetch_optional(&pool)
                .await?;

        let pub_user = PublicUser::from(user);
        let mut user_val = serde_json::to_value(pub_user).unwrap_or(json!({}));
        if let Some(obj) = user_val.as_object_mut() {
            obj.insert(
                "settings".to_string(),
                serde_json::to_value(settings).unwrap_or(Value::Null),
            );
        }
        users.push(user_val);
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
    tracing::info!(
        "Admin {} (ID: {}) creating user: {}",
        admin.username,
        admin.id,
        body.email
    );
    let existing: i64 = sqlx::query!(
        "SELECT COUNT(*) as cnt FROM users WHERE email = ? OR username = ?",
        body.email,
        body.username
    )
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

    sqlx::query!(
        "INSERT OR IGNORE INTO userSettings (userId, updatedAt) VALUES (?, ?)",
        user_id,
        now
    )
    .execute(&pool)
    .await?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&pool)
        .await?;

    tracing::info!("User created by admin: {} (ID: {})", body.email, user_id);
    Ok((
        StatusCode::CREATED,
        Json(json!({ "user": PublicUser::from(user) })),
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
    tracing::info!(
        "Admin {} (ID: {}) updating user ID: {}",
        admin.username,
        admin.id,
        uid
    );
    let existing = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(uid)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let email = body.email.unwrap_or(existing.email);
    let username = body.username.unwrap_or(existing.username);
    let name = body.name.unwrap_or(existing.name);
    let role = body.role.unwrap_or(existing.role);
    let now = Utc::now().to_rfc3339();

    let password_hash = if let Some(new_pw) = body.password {
        hash_password(&new_pw)?
    } else {
        existing.password_hash
    };

    sqlx::query!(
        "UPDATE users SET email = ?, username = ?, name = ?, passwordHash = ?, role = ?, updatedAt = ? \
         WHERE id = ?",
        email, username, name, password_hash, role, now, uid
    )
    .execute(&pool)
    .await?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(uid)
        .fetch_one(&pool)
        .await?;

    tracing::info!("User updated by admin ID: {}", uid);
    Ok(Json(json!({ "user": PublicUser::from(user) })))
}

pub async fn delete_user(
    State(pool): State<SqlitePool>,
    AdminUser(admin): AdminUser,
    Path(uid): Path<i64>,
) -> AppResult<Json<Value>> {
    tracing::info!(
        "Admin {} (ID: {}) deleting user ID: {}",
        admin.username,
        admin.id,
        uid
    );
    let result = sqlx::query!("DELETE FROM users WHERE id = ?", uid)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("User not found".to_string()));
    }

    Ok(Json(json!({ "message": "User deleted" })))
}

pub async fn regenerate_previews(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AdminUser(_admin): AdminUser,
) -> AppResult<Json<Value>> {
    tracing::info!("Regenerating all document previews and motorcycle resized images...");

    let mut doc_count = 0;
    let mut moto_count = 0;

    // 1. Process Documents
    let docs = sqlx::query_as::<_, crate::models::Document>("SELECT * FROM documents")
        .fetch_all(&pool)
        .await?;

    for doc in docs {
        let filename = doc
            .file_path
            .replace("/data/documents/", "")
            .replace("data/documents/", "");

        let ext = std::path::Path::new(&filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ["jpg", "jpeg", "png", "webp", "gif"].contains(&ext.as_str()) {
            let full_path = config.documents_dir().join(&filename);
            if let Ok(file_data) = tokio::fs::read(&full_path).await {
                let uuid = Uuid::new_v4().to_string();
                match generate_image_preview_internal(&config, &file_data, &uuid) {
                    Ok(preview_filename) => {
                        sqlx::query!(
                            "UPDATE documents SET previewPath = ? WHERE id = ?",
                            preview_filename,
                            doc.id
                        )
                        .execute(&pool)
                        .await?;
                        doc_count += 1;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to generate preview for doc {}: {}", doc.id, e)
                    }
                }
            }
        } else if ext == "pdf" {
            let full_path = config.documents_dir().join(&filename);
            if let Ok(file_data) = tokio::fs::read(&full_path).await {
                let uuid = Uuid::new_v4().to_string();
                match generate_pdf_preview_internal(&config, &file_data, &uuid) {
                    Ok(preview_filename) => {
                        sqlx::query!(
                            "UPDATE documents SET previewPath = ? WHERE id = ?",
                            preview_filename,
                            doc.id
                        )
                        .execute(&pool)
                        .await?;
                        doc_count += 1;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to generate PDF preview for doc {}: {}", doc.id, e)
                    }
                }
            }
        }
    }

    // 2. Process Motorcycles
    let motos = sqlx::query_as::<_, crate::models::Motorcycle>(
        "SELECT * FROM motorcycles WHERE image IS NOT NULL",
    )
    .fetch_all(&pool)
    .await?;

    for moto in motos {
        let image_path = moto.image.unwrap();
        let filename = image_path
            .replace("/data/images/", "")
            .replace("data/images/", "");

        let ext = std::path::Path::new(&filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ["jpg", "jpeg", "png", "webp", "gif"].contains(&ext.as_str()) {
            let full_path = config.images_dir().join(&filename);
            if let Ok(file_data) = tokio::fs::read(&full_path).await {
                let format = if ext == "webp" {
                    image::ImageFormat::WebP
                } else if ext == "png" {
                    image::ImageFormat::Png
                } else {
                    image::ImageFormat::Jpeg
                };

                let cache_ext = if ext == "webp" {
                    "webp"
                } else if ext == "png" {
                    "png"
                } else {
                    "jpg"
                };
                let stem = std::path::Path::new(&filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&filename);
                let cache_filename = format!("{}_400x400.{}", stem, cache_ext);
                let cache_path = config.resized_images_dir().join(&cache_filename);

                if let Ok(img) = image::load_from_memory(&file_data) {
                    let thumbnail = img.thumbnail(400, 400);
                    if thumbnail.save_with_format(&cache_path, format).is_ok() {
                        moto_count += 1;
                    }
                }
            }
        }
    }

    Ok(Json(json!({
        "message": format!("Regenerated {} document previews and {} motorcycle thumbnails", doc_count, moto_count),
        "docCount": doc_count,
        "motoCount": moto_count
    })))
}

fn generate_image_preview_internal(config: &Config, data: &[u8], uuid: &str) -> AppResult<String> {
    let img = image::load_from_memory(data)
        .map_err(|e| AppError::Image(format!("Failed to load image: {}", e)))?;

    let thumbnail = img.thumbnail(400, 400);
    let preview_filename = format!("{}.jpg", uuid);
    let preview_path = config.previews_dir().join(&preview_filename);

    thumbnail
        .save_with_format(&preview_path, image::ImageFormat::Jpeg)
        .map_err(|e| AppError::Image(format!("Failed to save preview: {}", e)))?;

    Ok(preview_filename)
}

fn generate_pdf_preview_internal(config: &Config, data: &[u8], uuid: &str) -> AppResult<String> {
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
            .or_else(|_| Pdfium::bind_to_system_library())
            .map_err(|e| AppError::Image(format!("Could not bind to Pdfium library: {}", e)))?,
    );

    let document = pdfium
        .load_pdf_from_byte_slice(data, None)
        .map_err(|e| AppError::Image(format!("Failed to load PDF: {:?}", e)))?;

    let first_page = document
        .pages()
        .get(0)
        .map_err(|e| AppError::Image(format!("Failed to get first page of PDF: {:?}", e)))?;

    let bitmap = first_page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(800)
                .set_maximum_height(1200),
        )
        .map_err(|e| AppError::Image(format!("Failed to render PDF page: {:?}", e)))?;

    let preview_filename = format!("{}.jpg", uuid);
    let preview_path = config.previews_dir().join(&preview_filename);

    bitmap
        .as_image()
        .thumbnail(400, 400)
        .save_with_format(&preview_path, image::ImageFormat::Jpeg)
        .map_err(|e| AppError::Image(format!("Failed to save PDF preview: {}", e)))?;

    Ok(preview_filename)
}

// ─── Currency Management ──────────────────────────────────────────────────────

pub async fn list_currencies(
    State(pool): State<SqlitePool>,
    AdminUser(_admin): AdminUser,
) -> AppResult<Json<Value>> {
    let currencies =
        sqlx::query_as::<_, CurrencySetting>("SELECT * FROM currencies ORDER BY code ASC")
            .fetch_all(&pool)
            .await?;

    Ok(Json(json!({ "currencies": currencies })))
}

pub async fn list_currencies_public(State(pool): State<SqlitePool>) -> AppResult<Json<Value>> {
    let currencies =
        sqlx::query_as::<_, CurrencySetting>("SELECT * FROM currencies ORDER BY code ASC")
            .fetch_all(&pool)
            .await?;

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
    let existing = sqlx::query!(
        "SELECT COUNT(*) as cnt FROM currencies WHERE code = ?",
        body.code
    )
    .fetch_one(&pool)
    .await?
    .cnt;

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

    let currency = sqlx::query_as::<_, CurrencySetting>("SELECT * FROM currencies WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok((StatusCode::CREATED, Json(json!({ "currency": currency }))))
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
    let existing = sqlx::query_as::<_, CurrencySetting>("SELECT * FROM currencies WHERE id = ?")
        .bind(cid)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Currency not found".to_string()))?;

    let code = body.code.unwrap_or(existing.code);
    let symbol = body.symbol.unwrap_or(existing.symbol);
    let label = body.label.or(existing.label);
    let conversion_factor = body.conversion_factor.unwrap_or(existing.conversion_factor);

    sqlx::query!(
        "UPDATE currencies SET code = ?, symbol = ?, label = ?, conversionFactor = ? WHERE id = ?",
        code,
        symbol,
        label,
        conversion_factor,
        cid
    )
    .execute(&pool)
    .await?;

    let currency = sqlx::query_as::<_, CurrencySetting>("SELECT * FROM currencies WHERE id = ?")
        .bind(cid)
        .fetch_one(&pool)
        .await?;

    Ok(Json(json!({ "currency": currency })))
}

pub async fn delete_currency(
    State(pool): State<SqlitePool>,
    AdminUser(_admin): AdminUser,
    Path(cid): Path<i64>,
) -> AppResult<Json<Value>> {
    let result = sqlx::query!("DELETE FROM currencies WHERE id = ?", cid)
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
