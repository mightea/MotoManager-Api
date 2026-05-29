use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::motorcycles::verify_motorcycle_ownership,
    models::TirePressure,
};

/// Fetch the recommended tire pressures for a motorcycle.
/// Returns `{ tirePressure: null }` with HTTP 200 when nothing has been
/// recorded yet — the frontend treats null and a missing row the same
/// way, but 200-with-null is the cleaner contract.
pub async fn get_tire_pressure(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let pressure = sqlx::query_as::<_, TirePressure>(
        "SELECT * FROM tirePressures WHERE motorcycleId = ?",
    )
    .bind(motorcycle_id)
    .fetch_optional(&pool)
    .await?;

    Ok(Json(json!({ "tirePressure": pressure })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertTirePressureRequest {
    pub front_bar: f64,
    pub rear_bar: f64,
    pub sidecar_bar: Option<f64>,
    pub preferred_unit: String,
}

/// Insert or update the tire pressure row for a motorcycle. The frontend
/// always sends canonical bar values (psi entries are converted client
/// side); preferredUnit is stored verbatim so the form re-opens in the
/// unit the user originally typed.
pub async fn upsert_tire_pressure(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Json(body): Json<UpsertTirePressureRequest>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let preferred_unit = match body.preferred_unit.as_str() {
        "psi" => "psi",
        "bar" => "bar",
        _ => {
            return Err(AppError::BadRequest(
                "preferredUnit must be 'bar' or 'psi'".to_string(),
            ))
        }
    };

    if !body.front_bar.is_finite() || body.front_bar <= 0.0 {
        return Err(AppError::BadRequest(
            "frontBar must be a positive number".to_string(),
        ));
    }
    if !body.rear_bar.is_finite() || body.rear_bar <= 0.0 {
        return Err(AppError::BadRequest(
            "rearBar must be a positive number".to_string(),
        ));
    }
    if let Some(s) = body.sidecar_bar {
        if !s.is_finite() || s <= 0.0 {
            return Err(AppError::BadRequest(
                "sidecarBar must be a positive number when provided".to_string(),
            ));
        }
    }

    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO tirePressures \
           (motorcycleId, frontBar, rearBar, sidecarBar, preferredUnit, createdAt, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(motorcycleId) DO UPDATE SET \
           frontBar = excluded.frontBar, \
           rearBar = excluded.rearBar, \
           sidecarBar = excluded.sidecarBar, \
           preferredUnit = excluded.preferredUnit, \
           updatedAt = excluded.updatedAt",
    )
    .bind(motorcycle_id)
    .bind(body.front_bar)
    .bind(body.rear_bar)
    .bind(body.sidecar_bar)
    .bind(preferred_unit)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await?;

    let pressure = sqlx::query_as::<_, TirePressure>(
        "SELECT * FROM tirePressures WHERE motorcycleId = ?",
    )
    .bind(motorcycle_id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(json!({ "tirePressure": pressure })))
}

pub async fn delete_tire_pressure(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let result = sqlx::query("DELETE FROM tirePressures WHERE motorcycleId = ?")
        .bind(motorcycle_id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Tire pressure not found".to_string()));
    }

    Ok(Json(json!({ "success": true, "message": "Tire pressure deleted" })))
}
