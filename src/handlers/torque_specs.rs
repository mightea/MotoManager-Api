use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{SqlitePool};

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::motorcycles::verify_motorcycle_ownership,
    models::TorqueSpec,
};

pub async fn list_torque_specs(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let specs = sqlx::query_as::<_, TorqueSpec>(
        "SELECT * FROM torqueSpecs WHERE motorcycleId = ? ORDER BY category ASC, name ASC",
    )
    .bind(motorcycle_id)
    .fetch_all(&pool)
    .await?;

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
}

pub async fn create_torque_spec(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Json(body): Json<CreateTorqueSpecRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let now = Utc::now().to_rfc3339();

    let id = sqlx::query(
        "INSERT INTO torqueSpecs \
         (motorcycleId, category, name, torque, torqueEnd, variation, toolSize, description, createdAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(motorcycle_id)
    .bind(&body.category)
    .bind(&body.name)
    .bind(body.torque)
    .bind(body.torque_end)
    .bind(body.variation)
    .bind(&body.tool_size)
    .bind(&body.description)
    .bind(&now)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    let spec = sqlx::query_as::<_, TorqueSpec>("SELECT * FROM torqueSpecs WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "torqueSpec": spec })),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportTorqueSpecsRequest {
    pub from_motorcycle_id: i64,
}

pub async fn import_torque_specs(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Json(body): Json<ImportTorqueSpecsRequest>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;
    verify_motorcycle_ownership(&pool, body.from_motorcycle_id, user.id).await?;

    let source_specs = sqlx::query_as::<_, TorqueSpec>(
        "SELECT * FROM torqueSpecs WHERE motorcycleId = ?",
    )
    .bind(body.from_motorcycle_id)
    .fetch_all(&pool)
    .await?;

    let now = Utc::now().to_rfc3339();
    let mut imported_count: i64 = 0;

    for spec in &source_specs {
        sqlx::query(
            "INSERT INTO torqueSpecs \
             (motorcycleId, category, name, torque, torqueEnd, variation, toolSize, description, createdAt) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(motorcycle_id)
        .bind(&spec.category)
        .bind(&spec.name)
        .bind(spec.torque)
        .bind(spec.torque_end)
        .bind(spec.variation)
        .bind(&spec.tool_size)
        .bind(&spec.description)
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

    sqlx::query(
        "UPDATE torqueSpecs SET \
         category = ?, name = ?, torque = ?, torqueEnd = ?, variation = ?, \
         toolSize = ?, description = ? \
         WHERE id = ?",
    )
    .bind(&category)
    .bind(&name)
    .bind(torque)
    .bind(torque_end)
    .bind(variation)
    .bind(&tool_size)
    .bind(&description)
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

    let result = sqlx::query("DELETE FROM torqueSpecs WHERE id = ? AND motorcycleId = ?")
        .bind(tid)
        .bind(motorcycle_id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Torque spec not found".to_string()));
    }

    Ok(Json(json!({ "message": "Torque spec deleted" })))
}
