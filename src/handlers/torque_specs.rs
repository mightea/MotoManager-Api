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
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::motorcycles::verify_motorcycle_ownership,
};

fn row_to_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": r.get::<i64, _>("id"),
        "motorcycleId": r.get::<i64, _>("motorcycleId"),
        "category": r.get::<String, _>("category"),
        "name": r.get::<String, _>("name"),
        "torque": r.get::<f64, _>("torque"),
        "torqueEnd": r.get::<Option<f64>, _>("torqueEnd"),
        "variation": r.get::<Option<f64>, _>("variation"),
        "toolSize": r.get::<Option<String>, _>("toolSize"),
        "description": r.get::<Option<String>, _>("description"),
        "createdAt": r.get::<String, _>("createdAt"),
    })
}

const SELECT_COLS: &str =
    "id, motorcycleId, category, name, torque, torqueEnd, variation, toolSize, \
     description, createdAt";

pub async fn list_torque_specs(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let rows = sqlx::query(&format!(
        "SELECT {} FROM torqueSpecs WHERE motorcycleId = ? ORDER BY category ASC, name ASC",
        SELECT_COLS
    ))
    .bind(motorcycle_id)
    .fetch_all(&pool)
    .await?;

    let specs: Vec<Value> = rows.iter().map(row_to_value).collect();
    Ok(Json(json!({ "torqueSpecs": specs })))
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

    let row = sqlx::query(&format!(
        "SELECT {} FROM torqueSpecs WHERE id = ?",
        SELECT_COLS
    ))
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "torqueSpec": row_to_value(&row) })),
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

    let source_rows = sqlx::query(
        "SELECT category, name, torque, torqueEnd, variation, toolSize, description \
         FROM torqueSpecs WHERE motorcycleId = ?",
    )
    .bind(body.from_motorcycle_id)
    .fetch_all(&pool)
    .await?;

    let now = Utc::now().to_rfc3339();
    let mut imported_count: i64 = 0;

    for row in &source_rows {
        let category: String = row.get("category");
        let name: String = row.get("name");
        let torque: f64 = row.get("torque");
        let torque_end: Option<f64> = row.get("torqueEnd");
        let variation: Option<f64> = row.get("variation");
        let tool_size: Option<String> = row.get("toolSize");
        let description: Option<String> = row.get("description");

        sqlx::query(
            "INSERT INTO torqueSpecs \
             (motorcycleId, category, name, torque, torqueEnd, variation, toolSize, description, createdAt) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(motorcycle_id)
        .bind(&category)
        .bind(&name)
        .bind(torque)
        .bind(torque_end)
        .bind(variation)
        .bind(&tool_size)
        .bind(&description)
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

    let existing = sqlx::query(&format!(
        "SELECT {} FROM torqueSpecs WHERE id = ? AND motorcycleId = ?",
        SELECT_COLS
    ))
    .bind(tid)
    .bind(motorcycle_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Torque spec not found".to_string()))?;

    let category = body.category.unwrap_or_else(|| existing.get("category"));
    let name = body.name.unwrap_or_else(|| existing.get("name"));
    let torque = body.torque.unwrap_or_else(|| existing.get("torque"));
    let torque_end: Option<f64> = body.torque_end.or_else(|| existing.get("torqueEnd"));
    let variation: Option<f64> = body.variation.or_else(|| existing.get("variation"));
    let tool_size: Option<String> = body.tool_size.or_else(|| existing.get("toolSize"));
    let description: Option<String> = body.description.or_else(|| existing.get("description"));

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

    let row = sqlx::query(&format!(
        "SELECT {} FROM torqueSpecs WHERE id = ?",
        SELECT_COLS
    ))
    .bind(tid)
    .fetch_one(&pool)
    .await?;

    Ok(Json(json!({ "torqueSpec": row_to_value(&row) })))
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
