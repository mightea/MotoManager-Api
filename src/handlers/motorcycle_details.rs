use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::{maintenance::sync_now, motorcycles::verify_motorcycle_ownership},
    models::MotorcycleDetail,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailFilter {
    /// Incremental-sync cursor; see maintenance `?since`.
    pub since: Option<String>,
}

pub async fn list_details(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Query(filter): Query<DetailFilter>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let details = if let Some(since) = filter.since {
        sqlx::query_as::<_, MotorcycleDetail>(
            "SELECT * FROM motorcycleDetails WHERE motorcycleId = ? AND updatedAt > ? \
             ORDER BY title ASC, id ASC",
        )
        .bind(motorcycle_id)
        .bind(since)
        .fetch_all(&pool)
        .await?
    } else {
        sqlx::query_as::<_, MotorcycleDetail>(
            "SELECT * FROM motorcycleDetails WHERE motorcycleId = ? AND deletedAt IS NULL \
             ORDER BY title ASC, id ASC",
        )
        .bind(motorcycle_id)
        .fetch_all(&pool)
        .await?
    };

    Ok(Json(json!({ "motorcycleDetails": details })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDetailRequest {
    pub title: String,
    pub value: String,
    /// Client-generated idempotency key (UUID).
    pub client_id: Option<String>,
}

pub async fn create_detail(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Json(body): Json<CreateDetailRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    if body.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".to_string()));
    }
    if body.value.trim().is_empty() {
        return Err(AppError::BadRequest("value is required".to_string()));
    }

    // Idempotency on clientId (see maintenance create).
    if let Some(client_id) = &body.client_id {
        if let Some(existing) = sqlx::query_as::<_, MotorcycleDetail>(
            "SELECT * FROM motorcycleDetails WHERE clientId = ? AND motorcycleId = ?",
        )
        .bind(client_id)
        .bind(motorcycle_id)
        .fetch_optional(&pool)
        .await?
        {
            return Ok((
                StatusCode::CREATED,
                Json(json!({ "motorcycleDetail": existing })),
            ));
        }
    }

    let now = sync_now();

    let id = sqlx::query(
        "INSERT INTO motorcycleDetails (motorcycleId, title, value, createdAt, clientId, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(motorcycle_id)
    .bind(body.title.trim())
    .bind(body.value.trim())
    .bind(&now)
    .bind(&body.client_id)
    .bind(&now)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    let detail =
        sqlx::query_as::<_, MotorcycleDetail>("SELECT * FROM motorcycleDetails WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "motorcycleDetail": detail })),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDetailRequest {
    pub title: Option<String>,
    pub value: Option<String>,
}

pub async fn update_detail(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((motorcycle_id, did)): Path<(i64, i64)>,
    Json(body): Json<UpdateDetailRequest>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let existing = sqlx::query_as::<_, MotorcycleDetail>(
        "SELECT * FROM motorcycleDetails WHERE id = ? AND motorcycleId = ?",
    )
    .bind(did)
    .bind(motorcycle_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Detail not found".to_string()))?;

    let title = body.title.unwrap_or(existing.title);
    let value = body.value.unwrap_or(existing.value);
    if title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".to_string()));
    }
    if value.trim().is_empty() {
        return Err(AppError::BadRequest("value is required".to_string()));
    }

    let now = sync_now();

    sqlx::query("UPDATE motorcycleDetails SET title = ?, value = ?, updatedAt = ? WHERE id = ?")
        .bind(title.trim())
        .bind(value.trim())
        .bind(&now)
        .bind(did)
        .execute(&pool)
        .await?;

    let detail =
        sqlx::query_as::<_, MotorcycleDetail>("SELECT * FROM motorcycleDetails WHERE id = ?")
            .bind(did)
            .fetch_one(&pool)
            .await?;

    Ok(Json(json!({ "motorcycleDetail": detail })))
}

pub async fn delete_detail(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((motorcycle_id, did)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let now = sync_now();
    let result = sqlx::query(
        "UPDATE motorcycleDetails SET deletedAt = ?, updatedAt = ? \
         WHERE id = ? AND motorcycleId = ? AND deletedAt IS NULL",
    )
    .bind(&now)
    .bind(&now)
    .bind(did)
    .bind(motorcycle_id)
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Detail not found".to_string()));
    }

    Ok(Json(json!({ "message": "Detail deleted" })))
}
