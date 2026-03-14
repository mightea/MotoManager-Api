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
};

fn row_to_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": r.get::<i64, _>("id"),
        "name": r.get::<String, _>("name"),
        "countryCode": r.get::<String, _>("countryCode"),
        "userId": r.get::<i64, _>("userId"),
    })
}

pub async fn list_locations(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query(
        "SELECT id, name, countryCode, userId FROM locations WHERE userId = ? ORDER BY name ASC",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    let locations: Vec<Value> = rows.iter().map(row_to_value).collect();
    Ok(Json(json!({ "locations": locations })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLocationRequest {
    pub name: String,
    pub country_code: Option<String>,
}

pub async fn create_location(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreateLocationRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    let country_code = body.country_code.unwrap_or_else(|| "CH".to_string());

    let id = sqlx::query(
        "INSERT INTO locations (name, countryCode, userId) VALUES (?, ?, ?)",
    )
    .bind(&body.name)
    .bind(&country_code)
    .bind(user.id)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    let row = sqlx::query(
        "SELECT id, name, countryCode, userId FROM locations WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "location": row_to_value(&row) })),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLocationRequest {
    pub name: Option<String>,
    pub country_code: Option<String>,
}

pub async fn update_location(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(lid): Path<i64>,
    Json(body): Json<UpdateLocationRequest>,
) -> AppResult<Json<Value>> {
    let existing = sqlx::query(
        "SELECT id, name, countryCode, userId FROM locations WHERE id = ? AND userId = ?",
    )
    .bind(lid)
    .bind(user.id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Location not found".to_string()))?;

    let name = body.name.unwrap_or_else(|| existing.get("name"));
    let country_code = body
        .country_code
        .unwrap_or_else(|| existing.get("countryCode"));

    sqlx::query("UPDATE locations SET name = ?, countryCode = ? WHERE id = ?")
        .bind(&name)
        .bind(&country_code)
        .bind(lid)
        .execute(&pool)
        .await?;

    let row = sqlx::query(
        "SELECT id, name, countryCode, userId FROM locations WHERE id = ?",
    )
    .bind(lid)
    .fetch_one(&pool)
    .await?;

    Ok(Json(json!({ "location": row_to_value(&row) })))
}

pub async fn delete_location(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(lid): Path<i64>,
) -> AppResult<Json<Value>> {
    let result = sqlx::query("DELETE FROM locations WHERE id = ? AND userId = ?")
        .bind(lid)
        .bind(user.id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Location not found".to_string()));
    }

    Ok(Json(json!({ "message": "Location deleted" })))
}
