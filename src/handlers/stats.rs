use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::{
    auth::AuthUser,
    config::Config,
    error::AppResult,
    handlers::motorcycles::maintenance_row_to_value,
};

pub async fn get_stats(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    // Basic counts for global stats
    let users_count: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM users").fetch_one(&pool).await?.get("cnt");
    let motorcycles_count: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM motorcycles").fetch_one(&pool).await?.get("cnt");
    let docs_count: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM documents").fetch_one(&pool).await?.get("cnt");

    // Full data for dashboard (filtered by user)
    let motorcycles = sqlx::query("SELECT id, make, model, modelYear, userId, vin, engineNumber, vehicleNr, numberPlate, image, isVeteran, isArchived, firstRegistration, initialOdo, manualOdo, purchaseDate, purchasePrice, normalizedPurchasePrice, currencyCode, fuelTankSize FROM motorcycles WHERE userId = ?")
        .bind(user.id)
        .fetch_all(&pool)
        .await?;
    
    let mut motorcycles_json = Vec::new();
    for r in &motorcycles {
        let moto_id: i64 = r.get("id");
        let open_issues: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM issues WHERE motorcycleId = ? AND status != 'done'")
            .bind(moto_id)
            .fetch_one(&pool)
            .await?
            .get("cnt");
        let maintenance_count: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM maintenanceRecords WHERE motorcycleId = ?")
            .bind(moto_id)
            .fetch_one(&pool)
            .await?
            .get("cnt");
        let latest_odo: Option<i64> = sqlx::query("SELECT MAX(odo) as max_odo FROM maintenanceRecords WHERE motorcycleId = ?")
            .bind(moto_id)
            .fetch_one(&pool)
            .await?
            .get("max_odo");

        motorcycles_json.push(json!({
            "id": r.get::<i64, _>("id"),
            "make": r.get::<String, _>("make"),
            "model": r.get::<String, _>("model"),
            "fabricationDate": r.get::<Option<String>, _>("modelYear"),
            "userId": r.get::<i64, _>("userId"),
            "vin": r.get::<Option<String>, _>("vin"),
            "engineNumber": r.get::<Option<String>, _>("engineNumber"),
            "vehicleNr": r.get::<Option<String>, _>("vehicleNr"),
            "numberPlate": r.get::<Option<String>, _>("numberPlate"),
            "image": r.get::<Option<String>, _>("image"),
            "isVeteran": r.get::<bool, _>("isVeteran"),
            "isArchived": r.get::<bool, _>("isArchived"),
            "firstRegistration": r.get::<Option<String>, _>("firstRegistration"),
            "initialOdo": r.get::<i64, _>("initialOdo"),
            "manualOdo": r.get::<Option<i64>, _>("manualOdo"),
            "purchaseDate": r.get::<Option<String>, _>("purchaseDate"),
            "purchasePrice": r.get::<Option<f64>, _>("purchasePrice"),
            "normalizedPurchasePrice": r.get::<Option<f64>, _>("normalizedPurchasePrice"),
            "currencyCode": r.get::<Option<String>, _>("currencyCode"),
            "fuelTankSize": r.get::<Option<f64>, _>("fuelTankSize"),
            "openIssues": open_issues,
            "maintenanceCount": maintenance_count,
            "latestOdo": latest_odo,
        }));
    }

    let issues = sqlx::query("SELECT id, motorcycleId, odo, description, priority, status, date FROM issues WHERE motorcycleId IN (SELECT id FROM motorcycles WHERE userId = ?)")
        .bind(user.id)
        .fetch_all(&pool)
        .await?;
    let issues_json: Vec<Value> = issues.iter().map(|r| json!({
        "id": r.get::<i64, _>("id"),
        "motorcycleId": r.get::<i64, _>("motorcycleId"),
        "odo": r.get::<i64, _>("odo"),
        "description": r.get::<Option<String>, _>("description"),
        "priority": r.get::<String, _>("priority"),
        "status": r.get::<String, _>("status"),
        "date": r.get::<Option<String>, _>("date"),
    })).collect();

    let maintenance = sqlx::query("SELECT * FROM maintenanceRecords WHERE motorcycleId IN (SELECT id FROM motorcycles WHERE userId = ?)")
        .bind(user.id)
        .fetch_all(&pool)
        .await?;
    let maintenance_json: Vec<Value> = maintenance.iter().map(maintenance_row_to_value).collect();

    let location_history = sqlx::query("SELECT id, motorcycleId, locationId, odometer, date FROM locationRecords WHERE motorcycleId IN (SELECT id FROM motorcycles WHERE userId = ?)")
        .bind(user.id)
        .fetch_all(&pool)
        .await?;
    let location_history_json: Vec<Value> = location_history.iter().map(|r| json!({
        "id": r.get::<i64, _>("id"),
        "motorcycleId": r.get::<i64, _>("motorcycleId"),
        "locationId": r.get::<i64, _>("locationId"),
        "odometer": r.get::<Option<i64>, _>("odometer"),
        "date": r.get::<String, _>("date"),
    })).collect();

    let locations = sqlx::query("SELECT id, name, countryCode, userId FROM locations WHERE userId = ?")
        .bind(user.id)
        .fetch_all(&pool)
        .await?;
    let locations_json: Vec<Value> = locations.iter().map(|r| json!({
        "id": r.get::<i64, _>("id"),
        "name": r.get::<String, _>("name"),
        "countryCode": r.get::<String, _>("countryCode"),
        "userId": r.get::<i64, _>("userId"),
    })).collect();

    let settings_row = sqlx::query("SELECT * FROM userSettings WHERE userId = ?").bind(user.id).fetch_optional(&pool).await?;
    let settings_json = settings_row.map(|r| json!({
        "id": r.get::<i64, _>("id"),
        "userId": r.get::<i64, _>("userId"),
        "tireInterval": r.get::<i64, _>("tireInterval"),
        "batteryLithiumInterval": r.get::<i64, _>("batteryLithiumInterval"),
        "batteryDefaultInterval": r.get::<i64, _>("batteryDefaultInterval"),
        "engineOilInterval": r.get::<i64, _>("engineOilInterval"),
        "gearboxOilInterval": r.get::<i64, _>("gearboxOilInterval"),
        "finalDriveOilInterval": r.get::<i64, _>("finalDriveOilInterval"),
        "forkOilInterval": r.get::<i64, _>("forkOilInterval"),
        "brakeFluidInterval": r.get::<i64, _>("brakeFluidInterval"),
        "coolantInterval": r.get::<i64, _>("coolantInterval"),
        "chainInterval": r.get::<i64, _>("chainInterval"),
        "tireKmInterval": r.get::<Option<i64>, _>("tireKmInterval"),
        "engineOilKmInterval": r.get::<Option<i64>, _>("engineOilKmInterval"),
        "gearboxOilKmInterval": r.get::<Option<i64>, _>("gearboxOilKmInterval"),
        "finalDriveOilKmInterval": r.get::<Option<i64>, _>("finalDriveOilKmInterval"),
        "forkOilKmInterval": r.get::<Option<i64>, _>("forkOilKmInterval"),
        "brakeFluidKmInterval": r.get::<Option<i64>, _>("brakeFluidKmInterval"),
        "coolantKmInterval": r.get::<Option<i64>, _>("coolantKmInterval"),
        "chainKmInterval": r.get::<Option<i64>, _>("chainKmInterval"),
        "updatedAt": r.get::<String, _>("updatedAt"),
    }));

    Ok(Json(json!({
        "stats": {
            "users": users_count,
            "motorcycles": motorcycles_count,
            "documents": docs_count,
        },
        "motorcycles": motorcycles_json,
        "issues": issues_json,
        "maintenance": maintenance_json,
        "locationHistory": location_history_json,
        "locations": locations_json,
        "settings": settings_json,
        "version": config.app_version,
    })))
}
