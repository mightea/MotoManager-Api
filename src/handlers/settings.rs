use axum::{
    extract::State,
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
        "userId": row.get::<i64, _>("user_id"),
        "tireInterval": row.get::<i64, _>("tire_interval"),
        "batteryLithiumInterval": row.get::<i64, _>("battery_lithium_interval"),
        "batteryDefaultInterval": row.get::<i64, _>("battery_default_interval"),
        "engineOilInterval": row.get::<i64, _>("engine_oil_interval"),
        "gearboxOilInterval": row.get::<i64, _>("gearbox_oil_interval"),
        "finalDriveOilInterval": row.get::<i64, _>("final_drive_oil_interval"),
        "forkOilInterval": row.get::<i64, _>("fork_oil_interval"),
        "brakeFluidInterval": row.get::<i64, _>("brake_fluid_interval"),
        "coolantInterval": row.get::<i64, _>("coolant_interval"),
        "chainInterval": row.get::<i64, _>("chain_interval"),
        "tireKmInterval": row.get::<Option<i64>, _>("tire_km_interval"),
        "engineOilKmInterval": row.get::<Option<i64>, _>("engine_oil_km_interval"),
        "gearboxOilKmInterval": row.get::<Option<i64>, _>("gearbox_oil_km_interval"),
        "finalDriveOilKmInterval": row.get::<Option<i64>, _>("final_drive_oil_km_interval"),
        "forkOilKmInterval": row.get::<Option<i64>, _>("fork_oil_km_interval"),
        "brakeFluidKmInterval": row.get::<Option<i64>, _>("brake_fluid_km_interval"),
        "coolantKmInterval": row.get::<Option<i64>, _>("coolant_km_interval"),
        "chainKmInterval": row.get::<Option<i64>, _>("chain_km_interval"),
        "updatedAt": row.get::<Option<String>, _>("updated_at"),
    })
}

const SETTINGS_SELECT: &str =
    "id, user_id, tire_interval, battery_lithium_interval, battery_default_interval, \
     engine_oil_interval, gearbox_oil_interval, final_drive_oil_interval, fork_oil_interval, \
     brake_fluid_interval, coolant_interval, chain_interval, tire_km_interval, \
     engine_oil_km_interval, gearbox_oil_km_interval, final_drive_oil_km_interval, \
     fork_oil_km_interval, brake_fluid_km_interval, coolant_km_interval, chain_km_interval, \
     updated_at";

pub async fn get_settings(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let settings_row = sqlx::query(&format!(
        "SELECT {} FROM user_settings WHERE user_id = ?",
        SETTINGS_SELECT
    ))
    .bind(user.id)
    .fetch_optional(&pool)
    .await?;

    let settings = if let Some(row) = settings_row {
        settings_row_to_value(&row)
    } else {
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT OR IGNORE INTO user_settings (user_id, updated_at) VALUES (?, ?)")
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

    sqlx::query("INSERT OR IGNORE INTO user_settings (user_id, updated_at) VALUES (?, ?)")
        .bind(user.id)
        .bind(&now)
        .execute(&pool)
        .await?;

    let existing = sqlx::query(&format!(
        "SELECT {} FROM user_settings WHERE user_id = ?",
        SETTINGS_SELECT
    ))
    .bind(user.id)
    .fetch_one(&pool)
    .await?;

    let tire_interval = body
        .tire_interval
        .unwrap_or_else(|| existing.get("tire_interval"));
    let battery_lithium_interval = body
        .battery_lithium_interval
        .unwrap_or_else(|| existing.get("battery_lithium_interval"));
    let battery_default_interval = body
        .battery_default_interval
        .unwrap_or_else(|| existing.get("battery_default_interval"));
    let engine_oil_interval = body
        .engine_oil_interval
        .unwrap_or_else(|| existing.get("engine_oil_interval"));
    let gearbox_oil_interval = body
        .gearbox_oil_interval
        .unwrap_or_else(|| existing.get("gearbox_oil_interval"));
    let final_drive_oil_interval = body
        .final_drive_oil_interval
        .unwrap_or_else(|| existing.get("final_drive_oil_interval"));
    let fork_oil_interval = body
        .fork_oil_interval
        .unwrap_or_else(|| existing.get("fork_oil_interval"));
    let brake_fluid_interval = body
        .brake_fluid_interval
        .unwrap_or_else(|| existing.get("brake_fluid_interval"));
    let coolant_interval = body
        .coolant_interval
        .unwrap_or_else(|| existing.get("coolant_interval"));
    let chain_interval = body
        .chain_interval
        .unwrap_or_else(|| existing.get("chain_interval"));
    let tire_km_interval: Option<i64> = body
        .tire_km_interval
        .or_else(|| existing.get("tire_km_interval"));
    let engine_oil_km_interval: Option<i64> = body
        .engine_oil_km_interval
        .or_else(|| existing.get("engine_oil_km_interval"));
    let gearbox_oil_km_interval: Option<i64> = body
        .gearbox_oil_km_interval
        .or_else(|| existing.get("gearbox_oil_km_interval"));
    let final_drive_oil_km_interval: Option<i64> = body
        .final_drive_oil_km_interval
        .or_else(|| existing.get("final_drive_oil_km_interval"));
    let fork_oil_km_interval: Option<i64> = body
        .fork_oil_km_interval
        .or_else(|| existing.get("fork_oil_km_interval"));
    let brake_fluid_km_interval: Option<i64> = body
        .brake_fluid_km_interval
        .or_else(|| existing.get("brake_fluid_km_interval"));
    let coolant_km_interval: Option<i64> = body
        .coolant_km_interval
        .or_else(|| existing.get("coolant_km_interval"));
    let chain_km_interval: Option<i64> = body
        .chain_km_interval
        .or_else(|| existing.get("chain_km_interval"));

    sqlx::query(
        "UPDATE user_settings SET \
         tire_interval = ?, battery_lithium_interval = ?, battery_default_interval = ?, \
         engine_oil_interval = ?, gearbox_oil_interval = ?, final_drive_oil_interval = ?, \
         fork_oil_interval = ?, brake_fluid_interval = ?, coolant_interval = ?, chain_interval = ?, \
         tire_km_interval = ?, engine_oil_km_interval = ?, gearbox_oil_km_interval = ?, \
         final_drive_oil_km_interval = ?, fork_oil_km_interval = ?, brake_fluid_km_interval = ?, \
         coolant_km_interval = ?, chain_km_interval = ?, updated_at = ? \
         WHERE user_id = ?",
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
        "SELECT {} FROM user_settings WHERE user_id = ?",
        SETTINGS_SELECT
    ))
    .bind(user.id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(json!({ "settings": settings_row_to_value(&row) })))
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

    sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?")
        .bind(&new_hash)
        .bind(&now)
        .bind(user.id)
        .execute(&pool)
        .await?;

    Ok(Json(json!({ "message": "Password changed successfully" })))
}

