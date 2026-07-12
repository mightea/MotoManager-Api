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
    models::TorqueSpec,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorqueFilter {
    /// Incremental-sync cursor; see maintenance `?since`.
    pub since: Option<String>,
}

pub async fn list_torque_specs(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Query(filter): Query<TorqueFilter>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let specs = if let Some(since) = filter.since {
        sqlx::query_as::<_, TorqueSpec>(
            "SELECT * FROM torqueSpecs WHERE motorcycleId = ? AND updatedAt > ? \
             ORDER BY category ASC, name ASC",
        )
        .bind(motorcycle_id)
        .bind(since)
        .fetch_all(&pool)
        .await?
    } else {
        sqlx::query_as::<_, TorqueSpec>(
            "SELECT * FROM torqueSpecs WHERE motorcycleId = ? AND deletedAt IS NULL \
             ORDER BY category ASC, name ASC",
        )
        .bind(motorcycle_id)
        .fetch_all(&pool)
        .await?
    };

    Ok(Json(json!({
        "torqueSpecs": specs,
        "torqueSpecifications": specs
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTorqueSpecRequest {
    pub category: String,
    pub name: String,
    pub torque: f64,
    pub torque_end: Option<f64>,
    pub variation: Option<f64>,
    pub tool_size: Option<String>,
    pub description: Option<String>,
    pub unverified: Option<bool>,
    /// Client-generated idempotency key (UUID).
    pub client_id: Option<String>,
}

pub async fn create_torque_spec(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Json(body): Json<CreateTorqueSpecRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    // Idempotency on clientId (see maintenance create).
    if let Some(client_id) = &body.client_id {
        if let Some(existing) = sqlx::query_as::<_, TorqueSpec>(
            "SELECT * FROM torqueSpecs WHERE clientId = ? AND motorcycleId = ?",
        )
        .bind(client_id)
        .bind(motorcycle_id)
        .fetch_optional(&pool)
        .await?
        {
            return Ok((StatusCode::CREATED, Json(json!({ "torqueSpec": existing }))));
        }
    }

    let now = sync_now();

    let id = sqlx::query(
        "INSERT INTO torqueSpecs \
         (motorcycleId, category, name, torque, torqueEnd, variation, toolSize, description, unverified, createdAt, clientId, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(motorcycle_id)
    .bind(&body.category)
    .bind(&body.name)
    .bind(body.torque)
    .bind(body.torque_end)
    .bind(body.variation)
    .bind(&body.tool_size)
    .bind(&body.description)
    .bind(body.unverified.unwrap_or(false))
    .bind(&now)
    .bind(&body.client_id)
    .bind(&now)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    let spec = sqlx::query_as::<_, TorqueSpec>("SELECT * FROM torqueSpecs WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok((StatusCode::CREATED, Json(json!({ "torqueSpec": spec }))))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportTorqueSpecsRequest {
    pub source_spec_ids: Vec<i64>,
}

pub async fn import_torque_specs(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Json(body): Json<ImportTorqueSpecsRequest>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let now = sync_now();
    let mut imported_count: i64 = 0;

    for spec_id in &body.source_spec_ids {
        let source = sqlx::query_as::<_, TorqueSpec>(
            "SELECT t.* FROM torqueSpecs t \
             JOIN motorcycles m ON t.motorcycleId = m.id \
             WHERE t.id = ? AND m.userId = ?",
        )
        .bind(spec_id)
        .bind(user.id)
        .fetch_optional(&pool)
        .await?;

        let Some(spec) = source else { continue };

        sqlx::query(
            "INSERT INTO torqueSpecs \
             (motorcycleId, category, name, torque, torqueEnd, variation, toolSize, description, unverified, createdAt, updatedAt) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(motorcycle_id)
        .bind(&spec.category)
        .bind(&spec.name)
        .bind(spec.torque)
        .bind(spec.torque_end)
        .bind(spec.variation)
        .bind(&spec.tool_size)
        .bind(&spec.description)
        .bind(spec.unverified)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await?;

        imported_count += 1;
    }

    Ok(Json(json!({
        "message": format!("Imported {} torque specs", imported_count),
        "count": imported_count
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTorqueSpecRequest {
    pub category: Option<String>,
    pub name: Option<String>,
    pub torque: Option<f64>,
    pub torque_end: Option<f64>,
    pub variation: Option<f64>,
    pub tool_size: Option<String>,
    pub description: Option<String>,
    pub unverified: Option<bool>,
}

pub async fn update_torque_spec(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((motorcycle_id, tid)): Path<(i64, i64)>,
    Json(body): Json<UpdateTorqueSpecRequest>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let existing = sqlx::query_as::<_, TorqueSpec>(
        "SELECT * FROM torqueSpecs WHERE id = ? AND motorcycleId = ?",
    )
    .bind(tid)
    .bind(motorcycle_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Torque spec not found".to_string()))?;

    let category = body.category.unwrap_or(existing.category);
    let name = body.name.unwrap_or(existing.name);
    let torque = body.torque.unwrap_or(existing.torque);
    let torque_end = body.torque_end.or(existing.torque_end);
    let variation = body.variation.or(existing.variation);
    let tool_size = body.tool_size.or(existing.tool_size);
    let description = body.description.or(existing.description);
    let unverified = body.unverified.unwrap_or(existing.unverified);

    let now = sync_now();

    sqlx::query(
        "UPDATE torqueSpecs SET \
         category = ?, name = ?, torque = ?, torqueEnd = ?, variation = ?, \
         toolSize = ?, description = ?, unverified = ?, updatedAt = ? \
         WHERE id = ?",
    )
    .bind(&category)
    .bind(&name)
    .bind(torque)
    .bind(torque_end)
    .bind(variation)
    .bind(&tool_size)
    .bind(&description)
    .bind(unverified)
    .bind(&now)
    .bind(tid)
    .execute(&pool)
    .await?;

    let spec = sqlx::query_as::<_, TorqueSpec>("SELECT * FROM torqueSpecs WHERE id = ?")
        .bind(tid)
        .fetch_one(&pool)
        .await?;

    Ok(Json(json!({ "torqueSpec": spec })))
}

pub async fn delete_torque_spec(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((motorcycle_id, tid)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let now = sync_now();
    let result = sqlx::query(
        "UPDATE torqueSpecs SET deletedAt = ?, updatedAt = ? \
         WHERE id = ? AND motorcycleId = ? AND deletedAt IS NULL",
    )
    .bind(&now)
    .bind(&now)
    .bind(tid)
    .bind(motorcycle_id)
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Torque spec not found".to_string()));
    }

    Ok(Json(json!({ "message": "Torque spec deleted" })))
}
