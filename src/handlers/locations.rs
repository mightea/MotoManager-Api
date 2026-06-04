use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::{Location, LocationType},
};

/// Reject coordinates that are out of WGS84 range or that arrive as a half-pair.
/// Returning Ok((None, None)) when both are absent is fine (coordinates are optional).
fn validate_coords(
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> AppResult<(Option<f64>, Option<f64>)> {
    match (latitude, longitude) {
        (None, None) => Ok((None, None)),
        (Some(lat), Some(lon)) => {
            if !(-90.0..=90.0).contains(&lat) {
                return Err(AppError::BadRequest(
                    "latitude must be between -90 and 90".to_string(),
                ));
            }
            if !(-180.0..=180.0).contains(&lon) {
                return Err(AppError::BadRequest(
                    "longitude must be between -180 and 180".to_string(),
                ));
            }
            Ok((Some(lat), Some(lon)))
        }
        _ => Err(AppError::BadRequest(
            "latitude and longitude must be provided together".to_string(),
        )),
    }
}

/// Verifies a location belongs to the given user. Used by handlers in other modules
/// (e.g. maintenance) that accept a locationId from the client.
pub async fn verify_location_ownership(
    pool: &SqlitePool,
    location_id: i64,
    user_id: i64,
) -> AppResult<()> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM locations WHERE id = ? AND userId = ?")
            .bind(location_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;

    if row.is_none() {
        return Err(AppError::NotFound("Location not found".to_string()));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationFilter {
    /// Comma-separated list of LocationType values in camelCase, e.g. `storage,maintenanceShop`.
    pub types: Option<String>,
}

pub async fn list_locations(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Query(filter): Query<LocationFilter>,
) -> AppResult<Json<Value>> {
    let mut query_str = "SELECT * FROM locations WHERE userId = ?".to_string();
    let mut types_list: Vec<LocationType> = Vec::new();

    if let Some(types) = filter.types {
        for raw in types.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let parsed: LocationType = serde_json::from_value(json!(raw))
                .map_err(|_| AppError::BadRequest(format!("Unknown location type: {}", raw)))?;
            types_list.push(parsed);
        }
        if !types_list.is_empty() {
            let placeholders: Vec<&str> = vec!["?"; types_list.len()];
            query_str.push_str(&format!(" AND type IN ({})", placeholders.join(", ")));
        }
    }

    query_str.push_str(" ORDER BY name ASC");

    let mut query = sqlx::query_as::<_, Location>(sqlx::AssertSqlSafe(query_str)).bind(user.id);
    for t in types_list {
        query = query.bind(t);
    }
    let locations = query.fetch_all(&pool).await?;

    Ok(Json(json!({ "locations": locations })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLocationRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub location_type: LocationType,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

pub async fn create_location(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreateLocationRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }
    let (latitude, longitude) = validate_coords(body.latitude, body.longitude)?;
    let now = Utc::now().to_rfc3339();

    let id = sqlx::query(
        "INSERT INTO locations (name, type, latitude, longitude, userId, createdAt) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(body.name.trim())
    .bind(body.location_type)
    .bind(latitude)
    .bind(longitude)
    .bind(user.id)
    .bind(&now)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    let location = sqlx::query_as::<_, Location>("SELECT * FROM locations WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok((StatusCode::CREATED, Json(json!({ "location": location }))))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLocationRequest {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub location_type: Option<LocationType>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

pub async fn update_location(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(lid): Path<i64>,
    Json(body): Json<UpdateLocationRequest>,
) -> AppResult<Json<Value>> {
    let existing =
        sqlx::query_as::<_, Location>("SELECT * FROM locations WHERE id = ? AND userId = ?")
            .bind(lid)
            .bind(user.id)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Location not found".to_string()))?;

    let name = body
        .name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or(existing.name);
    let location_type = body.location_type.unwrap_or(existing.location_type);
    // Coords have to be validated as a pair. If the client sends neither, keep what we had.
    let (latitude, longitude) = match (body.latitude, body.longitude) {
        (None, None) => (existing.latitude, existing.longitude),
        (lat, lon) => validate_coords(lat, lon)?,
    };
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE locations SET name = ?, type = ?, latitude = ?, longitude = ?, \
         updatedAt = ? WHERE id = ?",
    )
    .bind(&name)
    .bind(location_type)
    .bind(latitude)
    .bind(longitude)
    .bind(&now)
    .bind(lid)
    .execute(&pool)
    .await?;

    let location = sqlx::query_as::<_, Location>("SELECT * FROM locations WHERE id = ?")
        .bind(lid)
        .fetch_one(&pool)
        .await?;

    Ok(Json(json!({ "location": location })))
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
