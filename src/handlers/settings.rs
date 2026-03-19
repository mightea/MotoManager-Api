use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    auth::{
        password::{hash_password, verify_password},
        AuthUser,
    },
    error::{AppError, AppResult},
    models::{Authenticator, PublicUser, UserSettings},
};

pub async fn get_settings(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let settings = sqlx::query_as::<_, UserSettings>("SELECT * FROM userSettings WHERE userId = ?")
        .bind(user.id)
        .fetch_optional(&pool)
        .await?;

    let settings = if let Some(s) = settings {
        s
    } else {
        let now = Utc::now().to_rfc3339();
        sqlx::query!(
            "INSERT OR IGNORE INTO userSettings (userId, updatedAt) VALUES (?, ?)",
            user.id,
            now
        )
        .execute(&pool)
        .await?;

        sqlx::query_as::<_, UserSettings>("SELECT * FROM userSettings WHERE userId = ?")
            .bind(user.id)
            .fetch_one(&pool)
            .await?
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

    sqlx::query!(
        "INSERT OR IGNORE INTO userSettings (userId, updatedAt) VALUES (?, ?)",
        user.id,
        now
    )
    .execute(&pool)
    .await?;

    let existing = sqlx::query_as::<_, UserSettings>("SELECT * FROM userSettings WHERE userId = ?")
        .bind(user.id)
        .fetch_one(&pool)
        .await?;

    let tire_interval = body.tire_interval.unwrap_or(existing.tire_interval);
    let battery_lithium_interval = body
        .battery_lithium_interval
        .unwrap_or(existing.battery_lithium_interval);
    let battery_default_interval = body
        .battery_default_interval
        .unwrap_or(existing.battery_default_interval);
    let engine_oil_interval = body
        .engine_oil_interval
        .unwrap_or(existing.engine_oil_interval);
    let gearbox_oil_interval = body
        .gearbox_oil_interval
        .unwrap_or(existing.gearbox_oil_interval);
    let final_drive_oil_interval = body
        .final_drive_oil_interval
        .unwrap_or(existing.final_drive_oil_interval);
    let fork_oil_interval = body.fork_oil_interval.unwrap_or(existing.fork_oil_interval);
    let brake_fluid_interval = body
        .brake_fluid_interval
        .unwrap_or(existing.brake_fluid_interval);
    let coolant_interval = body.coolant_interval.unwrap_or(existing.coolant_interval);
    let chain_interval = body.chain_interval.unwrap_or(existing.chain_interval);
    let tire_km_interval = body.tire_km_interval.or(existing.tire_km_interval);
    let engine_oil_km_interval = body
        .engine_oil_km_interval
        .or(existing.engine_oil_km_interval);
    let gearbox_oil_km_interval = body
        .gearbox_oil_km_interval
        .or(existing.gearbox_oil_km_interval);
    let final_drive_oil_km_interval = body
        .final_drive_oil_km_interval
        .or(existing.final_drive_oil_km_interval);
    let fork_oil_km_interval = body.fork_oil_km_interval.or(existing.fork_oil_km_interval);
    let brake_fluid_km_interval = body
        .brake_fluid_km_interval
        .or(existing.brake_fluid_km_interval);
    let coolant_km_interval = body.coolant_km_interval.or(existing.coolant_km_interval);
    let chain_km_interval = body.chain_km_interval.or(existing.chain_km_interval);

    sqlx::query!(
        "UPDATE userSettings SET \
         tireInterval = ?, batteryLithiumInterval = ?, batteryDefaultInterval = ?, \
         engineOilInterval = ?, gearboxOilInterval = ?, finalDriveOilInterval = ?, \
         forkOilInterval = ?, brakeFluidInterval = ?, coolantInterval = ?, chainInterval = ?, \
         tireKmInterval = ?, engineOilKmInterval = ?, gearboxOilKmInterval = ?, \
         finalDriveOilKmInterval = ?, forkOilKmInterval = ?, brakeFluidKmInterval = ?, \
         coolantKmInterval = ?, chainKmInterval = ?, updatedAt = ? \
         WHERE userId = ?",
        tire_interval,
        battery_lithium_interval,
        battery_default_interval,
        engine_oil_interval,
        gearbox_oil_interval,
        final_drive_oil_interval,
        fork_oil_interval,
        brake_fluid_interval,
        coolant_interval,
        chain_interval,
        tire_km_interval,
        engine_oil_km_interval,
        gearbox_oil_km_interval,
        final_drive_oil_km_interval,
        fork_oil_km_interval,
        brake_fluid_km_interval,
        coolant_km_interval,
        chain_km_interval,
        now,
        user.id
    )
    .execute(&pool)
    .await?;

    let settings = sqlx::query_as::<_, UserSettings>("SELECT * FROM userSettings WHERE userId = ?")
        .bind(user.id)
        .fetch_one(&pool)
        .await?;

    Ok(Json(json!({ "settings": settings })))
}

pub async fn get_authenticators(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let authenticators = sqlx::query_as::<_, Authenticator>(
        "SELECT * FROM authenticators WHERE userId = ? ORDER BY createdAt DESC",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(json!(authenticators)))
}

pub async fn delete_authenticator(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let result = sqlx::query!(
        "DELETE FROM authenticators WHERE id = ? AND userId = ?",
        id,
        user.id
    )
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

    sqlx::query!(
        "UPDATE users SET passwordHash = ?, updatedAt = ? WHERE id = ?",
        new_hash,
        now,
        user.id
    )
    .execute(&pool)
    .await?;

    Ok(Json(json!({ "message": "Password changed successfully" })))
}
