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

    let pressure =
        sqlx::query_as::<_, TirePressure>("SELECT * FROM tirePressures WHERE motorcycleId = ?")
            .bind(motorcycle_id)
            .fetch_optional(&pool)
            .await?;

    Ok(Json(json!({ "tirePressure": pressure })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertTirePressureRequest {
    // One optional front/rear pair per riding configuration; at least one
    // complete pair is required (migrations 027/029).
    #[serde(default)]
    pub front_bar: Option<f64>,
    #[serde(default)]
    pub rear_bar: Option<f64>,
    #[serde(default)]
    pub front_passenger_bar: Option<f64>,
    #[serde(default)]
    pub rear_passenger_bar: Option<f64>,
    #[serde(default)]
    pub front_offroad_bar: Option<f64>,
    #[serde(default)]
    pub rear_offroad_bar: Option<f64>,
    // Sidecar wheel — one optional value per variant (migration 028);
    // sidecar_bar is the solo value.
    pub sidecar_bar: Option<f64>,
    #[serde(default)]
    pub sidecar_passenger_bar: Option<f64>,
    #[serde(default)]
    pub sidecar_offroad_bar: Option<f64>,
    pub preferred_unit: String,
}

/// A pressure must be a positive, finite bar value — shared by every field.
fn validate_pressure(name: &str, value: Option<f64>) -> AppResult<()> {
    if let Some(v) = value {
        if !v.is_finite() || v <= 0.0 {
            return Err(AppError::BadRequest(format!(
                "{name} must be a positive number"
            )));
        }
    }
    Ok(())
}

/// One riding configuration is either absent (all three fields null) or
/// carries a complete front/rear pair; the sidecar value rides along with a
/// complete pair only. Returns whether the configuration is present.
fn validate_config(
    name: &str,
    front: Option<f64>,
    rear: Option<f64>,
    sidecar: Option<f64>,
) -> AppResult<bool> {
    match (front, rear) {
        (Some(_), Some(_)) => Ok(true),
        (None, None) if sidecar.is_none() => Ok(false),
        _ => Err(AppError::BadRequest(format!(
            "{name} requires both front and rear pressures"
        ))),
    }
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

    validate_pressure("frontBar", body.front_bar)?;
    validate_pressure("rearBar", body.rear_bar)?;
    validate_pressure("frontPassengerBar", body.front_passenger_bar)?;
    validate_pressure("rearPassengerBar", body.rear_passenger_bar)?;
    validate_pressure("frontOffroadBar", body.front_offroad_bar)?;
    validate_pressure("rearOffroadBar", body.rear_offroad_bar)?;
    validate_pressure("sidecarBar", body.sidecar_bar)?;
    validate_pressure("sidecarPassengerBar", body.sidecar_passenger_bar)?;
    validate_pressure("sidecarOffroadBar", body.sidecar_offroad_bar)?;

    let has_solo = validate_config("solo", body.front_bar, body.rear_bar, body.sidecar_bar)?;
    let has_passenger = validate_config(
        "passenger",
        body.front_passenger_bar,
        body.rear_passenger_bar,
        body.sidecar_passenger_bar,
    )?;
    let has_offroad = validate_config(
        "offroad",
        body.front_offroad_bar,
        body.rear_offroad_bar,
        body.sidecar_offroad_bar,
    )?;
    if !has_solo && !has_passenger && !has_offroad {
        return Err(AppError::BadRequest(
            "at least one configuration is required — use DELETE to remove the record".to_string(),
        ));
    }

    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO tirePressures \
           (motorcycleId, frontBar, rearBar, \
            frontPassengerBar, rearPassengerBar, frontOffroadBar, rearOffroadBar, \
            sidecarBar, sidecarPassengerBar, sidecarOffroadBar, \
            preferredUnit, createdAt, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(motorcycleId) DO UPDATE SET \
           frontBar = excluded.frontBar, \
           rearBar = excluded.rearBar, \
           frontPassengerBar = excluded.frontPassengerBar, \
           rearPassengerBar = excluded.rearPassengerBar, \
           frontOffroadBar = excluded.frontOffroadBar, \
           rearOffroadBar = excluded.rearOffroadBar, \
           sidecarBar = excluded.sidecarBar, \
           sidecarPassengerBar = excluded.sidecarPassengerBar, \
           sidecarOffroadBar = excluded.sidecarOffroadBar, \
           preferredUnit = excluded.preferredUnit, \
           updatedAt = excluded.updatedAt",
    )
    .bind(motorcycle_id)
    .bind(body.front_bar)
    .bind(body.rear_bar)
    .bind(body.front_passenger_bar)
    .bind(body.rear_passenger_bar)
    .bind(body.front_offroad_bar)
    .bind(body.rear_offroad_bar)
    .bind(body.sidecar_bar)
    .bind(body.sidecar_passenger_bar)
    .bind(body.sidecar_offroad_bar)
    .bind(preferred_unit)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await?;

    let pressure =
        sqlx::query_as::<_, TirePressure>("SELECT * FROM tirePressures WHERE motorcycleId = ?")
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

    Ok(Json(
        json!({ "success": true, "message": "Tire pressure deleted" }),
    ))
}
