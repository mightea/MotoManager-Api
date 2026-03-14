use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::{
    auth::AuthUser,
    config::Config,
    error::AppResult,
};

pub async fn get_stats(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(_user): AuthUser,
) -> AppResult<Json<Value>> {
    let users: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM users")
        .fetch_one(&pool)
        .await?
        .get("cnt");

    let motorcycles: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM motorcycles")
        .fetch_one(&pool)
        .await?
        .get("cnt");

    let maintenance_records: i64 =
        sqlx::query("SELECT COUNT(*) as cnt FROM maintenanceRecords")
            .fetch_one(&pool)
            .await?
            .get("cnt");

    let issues: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM issues")
        .fetch_one(&pool)
        .await?
        .get("cnt");

    let documents: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM documents")
        .fetch_one(&pool)
        .await?
        .get("cnt");

    let locations: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM locations")
        .fetch_one(&pool)
        .await?
        .get("cnt");

    let torque_specs: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM torqueSpecs")
        .fetch_one(&pool)
        .await?
        .get("cnt");

    let currencies: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM currencies")
        .fetch_one(&pool)
        .await?
        .get("cnt");

    Ok(Json(json!({
        "users": users,
        "motorcycles": motorcycles,
        "maintenanceRecords": maintenance_records,
        "issues": issues,
        "documents": documents,
        "locations": locations,
        "torqueSpecs": torque_specs,
        "currencies": currencies,
        "version": config.app_version,
    })))
}
