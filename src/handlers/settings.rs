use axum::{
    extract::{State, Path},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::{
    auth::{
        password::{hash_password, verify_password},
        AuthUser,
    },
    error::{AppError, AppResult},
    models::PublicUser,
};

fn settings_row_to_value(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<i64, _>("id"),
        "userId": row.get::<i64, _>("userId"),
        "tireInterval": row.get::<i64, _>("tireInterval"),
        "batteryLithiumInterval": row.get::<i64, _>("batteryLithiumInterval"),
        "batteryDefaultInterval": row.get::<i64, _>("batteryDefaultInterval"),
        "engineOilInterval": row.get::<i64, _>("engineOilInterval"),
        "gearboxOilInterval": row.get::<i64, _>("gearboxOilInterval"),
        "finalDriveOilInterval": row.get::<i64, _>("finalDriveOilInterval"),
        "forkOilInterval": row.get::<i64, _>("forkOilInterval"),
        "brakeFluidInterval": row.get::<i64, _>("brakeFluidInterval"),
        "coolantInterval": row.get::<i64, _>("coolantInterval"),
        "chainInterval": row.get::<i64, _>("chainInterval"),
        "tireKmInterval": row.get::<Option<i64>, _>("tireKmInterval"),
        "engineOilKmInterval": row.get::<Option<i64>, _>("engineOilKmInterval"),
        "gearboxOilKmInterval": row.get::<Option<i64>, _>("gearboxOilKmInterval"),
        "finalDriveOilKmInterval": row.get::<Option<i64>, _>("finalDriveOilKmInterval"),
        "forkOilKmInterval": row.get::<Option<i64>, _>("forkOilKmInterval"),
        "brakeFluidKmInterval": row.get::<Option<i64>, _>("brakeFluidKmInterval"),
        "coolantKmInterval": row.get::<Option<i64>, _>("coolantKmInterval"),
        "chainKmInterval": row.get::<Option<i64>, _>("chainKmInterval"),
        "updatedAt": row.get::<Option<String>, _>("updatedAt"),
    })
}

const SETTINGS_SELECT: &str =
    "id, userId, tireInterval, batteryLithiumInterval, batteryDefaultInterval, \
     engineOilInterval, gearboxOilInterval, finalDriveOilInterval, forkOilInterval, \
     brakeFluidInterval, coolantInterval, chainInterval, tireKmInterval, \
     engineOilKmInterval, gearboxOilKmInterval, finalDriveOilKmInterval, \
     forkOilKmInterval, brakeFluidKmInterval, coolantKmInterval, chainKmInterval, \
     updatedAt";

pub async fn get_settings(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let settings_row = sqlx::query(&format!(
        "SELECT {} FROM userSettings WHERE userId = ?",
        SETTINGS_SELECT
    ))
    .bind(user.id)
    .fetch_optional(&pool)
    .await?;

    let settings = if let Some(row) = settings_row {
        settings_row_to_value(&row)
    } else {
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT OR IGNORE INTO userSettings (userId, updatedAt) VALUES (?, ?)")
            .bind(user.id)
            .bind(&now)
            .execute(&pool)
            .await?;
        json!({
            "userId": user.id,
            "tireInterval": 8,
            "batteryLithiumInterval": 10,
            "batteryDefaultInterval": 6,
            "engineOilInterval": 2,
            "gearboxOilInterval": 2,
            "finalDriveOilInterval": 2,
            "forkOilInterval": 4,
            "brakeFluidInterval": 4,
            "coolantInterval": 4,
            "chainInterval": 1,
            "tireKmInterval": null,
            "engineOilKmInterval": null,
            "gearboxOilKmInterval": null,
            "finalDriveOilKmInterval": null,
            "forkOilKmInterval": null,
            "brakeFluidKmInterval": null,
            "coolantKmInterval": null,
            "chainKmInterval": null,
            "updatedAt": now,
        })
    };

    Ok(Json(json!({
        "settings": settings,
        "user": PublicUser::from(user),
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsRequest {
    pub tire_interval: Option<i64>,
    pub battery_lithium_interval: Option<i64>,
    pub battery_default_interval: Option<i64>,
    pub engine_oil_interval: Option<i64>,
    pub gearbox_oil_interval: Option<i64>,
    pub final_drive_oil_interval: Option<i64>,
    pub fork_oil_interval: Option<i64>,
    pub brake_fluid_interval: Option<i64>,
    pub coolant_interval: Option<i64>,
    pub chain_interval: Option<i64>,
    pub tire_km_interval: Option<i64>,
    pub engine_oil_km_interval: Option<i64>,
    pub gearbox_oil_km_interval: Option<i64>,
    pub final_drive_oil_km_interval: Option<i64>,
    pub fork_oil_km_interval: Option<i64>,
    pub brake_fluid_km_interval: Option<i64>,
    pub coolant_km_interval: Option<i64>,
    pub chain_km_interval: Option<i64>,
}

pub async fn update_settings(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Json(body): Json<UpdateSettingsRequest>,
) -> AppResult<Json<Value>> {
    let now = Utc::now().to_rfc3339();

    sqlx::query("INSERT OR IGNORE INTO userSettings (userId, updatedAt) VALUES (?, ?)")
        .bind(user.id)
        .bind(&now)
        .execute(&pool)
        .await?;

    let existing = sqlx::query(&format!(
        "SELECT {} FROM userSettings WHERE userId = ?",
        SETTINGS_SELECT
    ))
    .bind(user.id)
    .fetch_one(&pool)
    .await?;

    let tire_interval = body
        .tire_interval
        .unwrap_or_else(|| existing.get("tireInterval"));
    let battery_lithium_interval = body
        .battery_lithium_interval
        .unwrap_or_else(|| existing.get("batteryLithiumInterval"));
    let battery_default_interval = body
        .battery_default_interval
        .unwrap_or_else(|| existing.get("batteryDefaultInterval"));
    let engine_oil_interval = body
        .engine_oil_interval
        .unwrap_or_else(|| existing.get("engineOilInterval"));
    let gearbox_oil_interval = body
        .gearbox_oil_interval
        .unwrap_or_else(|| existing.get("gearboxOilInterval"));
    let final_drive_oil_interval = body
        .final_drive_oil_interval
        .unwrap_or_else(|| existing.get("finalDriveOilInterval"));
    let fork_oil_interval = body
        .fork_oil_interval
        .unwrap_or_else(|| existing.get("forkOilInterval"));
    let brake_fluid_interval = body
        .brake_fluid_interval
        .unwrap_or_else(|| existing.get("brakeFluidInterval"));
    let coolant_interval = body
        .coolant_interval
        .unwrap_or_else(|| existing.get("coolantInterval"));
    let chain_interval = body
        .chain_interval
        .unwrap_or_else(|| existing.get("chainInterval"));
    let tire_km_interval: Option<i64> = body
        .tire_km_interval
        .or_else(|| existing.get("tireKmInterval"));
    let engine_oil_km_interval: Option<i64> = body
        .engine_oil_km_interval
        .or_else(|| existing.get("engineOilKmInterval"));
    let gearbox_oil_km_interval: Option<i64> = body
        .gearbox_oil_km_interval
        .or_else(|| existing.get("gearboxOilKmInterval"));
    let final_drive_oil_km_interval: Option<i64> = body
        .final_drive_oil_km_interval
        .or_else(|| existing.get("finalDriveOilKmInterval"));
    let fork_oil_km_interval: Option<i64> = body
        .fork_oil_km_interval
        .or_else(|| existing.get("forkOilKmInterval"));
    let brake_fluid_km_interval: Option<i64> = body
        .brake_fluid_km_interval
        .or_else(|| existing.get("brakeFluidKmInterval"));
    let coolant_km_interval: Option<i64> = body
        .coolant_km_interval
        .or_else(|| existing.get("coolantKmInterval"));
    let chain_km_interval: Option<i64> = body
        .chain_km_interval
        .or_else(|| existing.get("chainKmInterval"));

    sqlx::query(
        "UPDATE userSettings SET \
         tireInterval = ?, batteryLithiumInterval = ?, batteryDefaultInterval = ?, \
         engineOilInterval = ?, gearboxOilInterval = ?, finalDriveOilInterval = ?, \
         forkOilInterval = ?, brakeFluidInterval = ?, coolantInterval = ?, chainInterval = ?, \
         tireKmInterval = ?, engineOilKmInterval = ?, gearboxOilKmInterval = ?, \
         finalDriveOilKmInterval = ?, forkOilKmInterval = ?, brakeFluidKmInterval = ?, \
         coolantKmInterval = ?, chainKmInterval = ?, updatedAt = ? \
         WHERE userId = ?",
    )
    .bind(tire_interval)
    .bind(battery_lithium_interval)
    .bind(battery_default_interval)
    .bind(engine_oil_interval)
    .bind(gearbox_oil_interval)
    .bind(final_drive_oil_interval)
    .bind(fork_oil_interval)
    .bind(brake_fluid_interval)
    .bind(coolant_interval)
    .bind(chain_interval)
    .bind(tire_km_interval)
    .bind(engine_oil_km_interval)
    .bind(gearbox_oil_km_interval)
    .bind(final_drive_oil_km_interval)
    .bind(fork_oil_km_interval)
    .bind(brake_fluid_km_interval)
    .bind(coolant_km_interval)
    .bind(chain_km_interval)
    .bind(&now)
    .bind(user.id)
    .execute(&pool)
    .await?;

    let row = sqlx::query(&format!(
        "SELECT {} FROM userSettings WHERE userId = ?",
        SETTINGS_SELECT
    ))
    .bind(user.id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(json!({ "settings": settings_row_to_value(&row) })))
}

pub async fn get_authenticators(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query("SELECT id, userId, deviceType, createdAt FROM authenticators WHERE userId = ? ORDER BY createdAt DESC")
        .bind(user.id)
        .fetch_all(&pool)
        .await?;

    let authenticators: Vec<Value> = rows.iter().map(|r| json!({
        "id": r.get::<String, _>("id"),
        "userId": r.get::<i64, _>("userId"),
        "deviceType": r.get::<String, _>("deviceType"),
        "createdAt": r.get::<String, _>("createdAt"),
    })).collect();

    Ok(Json(json!(authenticators)))
}

pub async fn delete_authenticator(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let result = sqlx::query("DELETE FROM authenticators WHERE id = ? AND userId = ?")
        .bind(&id)
        .bind(user.id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Authenticator not found".to_string()));
    }

    Ok(Json(json!({ "message": "Authenticator deleted" })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
    pub confirm_password: String,
}

pub async fn change_password(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Json(body): Json<ChangePasswordRequest>,
) -> AppResult<Json<Value>> {
    if body.new_password != body.confirm_password {
        return Err(AppError::BadRequest("Passwords do not match".to_string()));
    }
    if body.new_password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    let valid = verify_password(&body.current_password, &user.password_hash)?;
    if !valid {
        return Err(AppError::BadRequest(
            "Current password is incorrect".to_string(),
        ));
    }

    let new_hash = hash_password(&body.new_password)?;
    let now = Utc::now().to_rfc3339();

    sqlx::query("UPDATE users SET passwordHash = ?, updatedAt = ? WHERE id = ?")
        .bind(&new_hash)
        .bind(&now)
        .bind(user.id)
        .execute(&pool)
        .await?;

    Ok(Json(json!({ "message": "Password changed successfully" })))
}
