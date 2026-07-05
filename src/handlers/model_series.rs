use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::ModelSeries,
};

/// Helper: a series is usable by a user when it is a global seed row
/// (userId NULL) or one of their own custom entries.
pub async fn verify_series_accessible(
    pool: &SqlitePool,
    series_id: i64,
    user_id: i64,
) -> AppResult<()> {
    let count: i64 = sqlx::query(
        "SELECT COUNT(*) as cnt FROM modelSeries WHERE id = ? AND (userId IS NULL OR userId = ?)",
    )
    .bind(series_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?
    .get("cnt");
    if count == 0 {
        return Err(AppError::NotFound("Model series not found".to_string()));
    }
    Ok(())
}

pub async fn list_model_series(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let series = sqlx::query_as::<_, ModelSeries>(
        "SELECT * FROM modelSeries WHERE userId IS NULL OR userId = ? \
         ORDER BY manufacturer ASC, name ASC",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(json!({ "modelSeries": series })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateModelSeriesRequest {
    pub name: String,
    pub manufacturer: Option<String>,
}

pub async fn create_model_series(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreateModelSeriesRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }
    let manufacturer = body
        .manufacturer
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "BMW".to_string());

    // Idempotent create: an equal global or own entry is returned instead of
    // erroring, so the iOS inline "Eigene Baureihe" picker can retry safely.
    if let Some(existing) = sqlx::query_as::<_, ModelSeries>(
        "SELECT * FROM modelSeries WHERE manufacturer = ? AND name = ? \
         AND (userId IS NULL OR userId = ?)",
    )
    .bind(&manufacturer)
    .bind(&name)
    .bind(user.id)
    .fetch_optional(&pool)
    .await?
    {
        return Ok((StatusCode::OK, Json(json!({ "modelSeries": existing }))));
    }

    let id = sqlx::query("INSERT INTO modelSeries (name, manufacturer, userId) VALUES (?, ?, ?)")
        .bind(&name)
        .bind(&manufacturer)
        .bind(user.id)
        .execute(&pool)
        .await?
        .last_insert_rowid();

    let series = sqlx::query_as::<_, ModelSeries>("SELECT * FROM modelSeries WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok((StatusCode::CREATED, Json(json!({ "modelSeries": series }))))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateModelSeriesRequest {
    pub name: Option<String>,
    pub manufacturer: Option<String>,
}

pub async fn update_model_series(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(sid): Path<i64>,
    Json(body): Json<UpdateModelSeriesRequest>,
) -> AppResult<Json<Value>> {
    // Only own custom entries are editable; global and foreign rows are masked.
    let existing = sqlx::query_as::<_, ModelSeries>(
        "SELECT * FROM modelSeries WHERE id = ? AND userId = ?",
    )
    .bind(sid)
    .bind(user.id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Model series not found".to_string()))?;

    let name = body.name.unwrap_or(existing.name);
    let manufacturer = body.manufacturer.unwrap_or(existing.manufacturer);

    sqlx::query("UPDATE modelSeries SET name = ?, manufacturer = ? WHERE id = ? AND userId = ?")
        .bind(&name)
        .bind(&manufacturer)
        .bind(sid)
        .bind(user.id)
        .execute(&pool)
        .await?;

    let series = sqlx::query_as::<_, ModelSeries>("SELECT * FROM modelSeries WHERE id = ?")
        .bind(sid)
        .fetch_one(&pool)
        .await?;

    Ok(Json(json!({ "modelSeries": series })))
}

pub async fn delete_model_series(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(sid): Path<i64>,
) -> AppResult<Json<Value>> {
    let owned: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM modelSeries WHERE id = ? AND userId = ?")
        .bind(sid)
        .bind(user.id)
        .fetch_one(&pool)
        .await?
        .get("cnt");
    if owned == 0 {
        return Err(AppError::NotFound("Model series not found".to_string()));
    }

    // A lookup row disappears for good (hard delete), so refuse while anything
    // still points at it — otherwise parts/motorcycles would dangle.
    let part_refs: i64 =
        sqlx::query("SELECT COUNT(*) as cnt FROM partSeriesCompat WHERE seriesId = ?")
            .bind(sid)
            .fetch_one(&pool)
            .await?
            .get("cnt");
    let moto_refs: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM motorcycles WHERE seriesId = ?")
        .bind(sid)
        .fetch_one(&pool)
        .await?
        .get("cnt");
    if part_refs > 0 || moto_refs > 0 {
        return Err(AppError::BadRequest(
            "Model series is still referenced by parts or motorcycles".to_string(),
        ));
    }

    sqlx::query("DELETE FROM modelSeries WHERE id = ? AND userId = ?")
        .bind(sid)
        .bind(user.id)
        .execute(&pool)
        .await?;

    Ok(Json(json!({ "message": "Model series deleted" })))
}
