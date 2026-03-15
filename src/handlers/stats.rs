use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use chrono::Datelike;

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
    // 1. Global / Instance Counts
    let users_count: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM users").fetch_one(&pool).await?.get("cnt");
    let motorcycles_count_global: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM motorcycles").fetch_one(&pool).await?.get("cnt");
    let archived_count_global: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM motorcycles WHERE isArchived = 1").fetch_one(&pool).await?.get("cnt");
    let docs_count_global: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM documents").fetch_one(&pool).await?.get("cnt");
    let doc_assignments_count_global: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM documentMotorcycles").fetch_one(&pool).await?.get("cnt");
    let maintenance_count_total_global: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM maintenanceRecords").fetch_one(&pool).await?.get("cnt");
    let issues_count_total_global: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM issues").fetch_one(&pool).await?.get("cnt");
    let open_issues_count_total_global: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM issues WHERE status != 'done'").fetch_one(&pool).await?.get("cnt");
    let locations_count_total_global: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM locations").fetch_one(&pool).await?.get("cnt");
    let location_history_count_total_global: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM locationRecords").fetch_one(&pool).await?.get("cnt");
    let torque_specs_count_total_global: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM torqueSpecs").fetch_one(&pool).await?.get("cnt");

    // 2. Fetch all user data for computation
    let motorcycles = sqlx::query("SELECT * FROM motorcycles WHERE userId = ?")
        .bind(user.id)
        .fetch_all(&pool)
        .await?;
    
    let maintenance = sqlx::query("SELECT * FROM maintenanceRecords WHERE motorcycleId IN (SELECT id FROM motorcycles WHERE userId = ?)")
        .bind(user.id)
        .fetch_all(&pool)
        .await?;

    let issues = sqlx::query("SELECT * FROM issues WHERE motorcycleId IN (SELECT id FROM motorcycles WHERE userId = ?)")
        .bind(user.id)
        .fetch_all(&pool)
        .await?;

    let location_history = sqlx::query("SELECT * FROM locationRecords WHERE motorcycleId IN (SELECT id FROM motorcycles WHERE userId = ?)")
        .bind(user.id)
        .fetch_all(&pool)
        .await?;

    let locations = sqlx::query("SELECT * FROM locations WHERE userId = ?")
        .bind(user.id)
        .fetch_all(&pool)
        .await?;

    let settings_row = sqlx::query("SELECT * FROM userSettings WHERE userId = ?").bind(user.id).fetch_optional(&pool).await?;

    // 3. Perform Aggregations
    let current_year = Utc::now().year();
    let mut yearly_stats: HashMap<i32, Value> = HashMap::new();
    let mut total_km_overall = 0i64;
    let mut total_km_this_year = 0i64;
    let mut total_cost_this_year = 0.0f64;
    let mut veteran_count = 0i64;
    let mut total_active_issues = 0i64;
    let mut moto_yearly_distance: HashMap<i64, i64> = HashMap::new();

    // Process each motorcycle
    let mut motorcycles_json = Vec::new();
    for r in &motorcycles {
        let moto_id: i64 = r.get("id");
        let initial_odo: i64 = r.get("initialOdo");
        let is_veteran: bool = r.get("isVeteran");
        if is_veteran { veteran_count += 1; }
        
        // Find max ODO from all sources
        let mut max_odo = initial_odo;
        let mut max_odo_prev_year = initial_odo;

        for m in maintenance.iter().filter(|m| m.get::<i64, _>("motorcycleId") == moto_id) {
            let odo: i64 = m.get("odo");
            if odo > max_odo { max_odo = odo; }
            
            if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&m.get::<String, _>("date")) {
                if date.year() < current_year && odo > max_odo_prev_year {
                    max_odo_prev_year = odo;
                }
            }
        }

        for i in issues.iter().filter(|i| i.get::<i64, _>("motorcycleId") == moto_id) {
            let odo: i64 = i.get("odo");
            let status: String = i.get("status");
            if status != "done" { total_active_issues += 1; }

            if odo > max_odo { max_odo = odo; }
            if let Some(date_str) = i.get::<Option<String>, _>("date") {
                if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&date_str) {
                    if date.year() < current_year && odo > max_odo_prev_year {
                        max_odo_prev_year = odo;
                    }
                }
            }
        }

        let distance_this_year = max_odo - max_odo_prev_year;
        total_km_this_year += distance_this_year;
        total_km_overall += max_odo - initial_odo;
        moto_yearly_distance.insert(moto_id, distance_this_year);

        motorcycles_json.push(json!({
            "id": moto_id,
            "make": r.get::<String, _>("make"),
            "model": r.get::<String, _>("model"),
            "fabricationDate": r.get::<Option<String>, _>("modelYear"),
            "userId": r.get::<i64, _>("userId"),
            "image": r.get::<Option<String>, _>("image"),
            "isVeteran": is_veteran,
            "isArchived": r.get::<bool, _>("isArchived"),
            "initialOdo": initial_odo,
            "odometer": max_odo,
            "odometerThisYear": distance_this_year,
        }));
    }

    // Process costs and yearly fleet stats
    for m in &maintenance {
        if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&m.get::<String, _>("date")) {
            let year = date.year();
            let cost = m.get::<Option<f64>, _>("normalizedCost").or_else(|| m.get::<Option<f64>, _>("cost")).unwrap_or(0.0);
            
            if year == current_year {
                total_cost_this_year += cost;
            }

            let entry = yearly_stats.entry(year).or_insert(json!({
                "year": year,
                "distance": 0,
                "cost": 0.0,
                "motorcycleCount": 0,
            }));
            
            if let Some(obj) = entry.as_object_mut() {
                let current_cost = obj.get("cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
                obj.insert("cost".to_string(), json!(current_cost + cost));
            }
        }
    }

    // Top Rider calculation
    let top_rider = motorcycles.iter()
        .filter(|r| moto_yearly_distance.get(&r.get::<i64, _>("id")).cloned().unwrap_or(0) > 0)
        .max_by_key(|r| moto_yearly_distance.get(&r.get::<i64, _>("id")).cloned().unwrap_or(0))
        .map(|r| {
            let id: i64 = r.get("id");
            json!({
                "id": id,
                "make": r.get::<String, _>("make"),
                "model": r.get::<String, _>("model"),
                "odometerThisYear": moto_yearly_distance.get(&id).unwrap_or(&0),
            })
        });

    let mut yearly_vec: Vec<Value> = yearly_stats.into_values().collect();
    yearly_vec.sort_by_key(|v| v["year"].as_i64().unwrap_or(0) * -1);

    let avg_moto_per_user = if users_count > 0 { motorcycles_count_global as f64 / users_count as f64 } else { 0.0 };
    let avg_docs_per_user = if users_count > 0 { docs_count_global as f64 / users_count as f64 } else { 0.0 };

    Ok(Json(json!({
        "stats": {
            "year": current_year,
            "totalMotorcycles": motorcycles.len(),
            "totalKmThisYear": total_km_this_year,
            "totalKmOverall": total_km_overall,
            "totalActiveIssues": total_active_issues,
            "totalMaintenanceCostThisYear": total_cost_this_year,
            "veteranCount": veteran_count,
            "topRider": top_rider,
            "yearly": yearly_vec,
            // Instance-wide stats for admin view
            "global": {
                "users": users_count,
                "motorcycles": motorcycles_count_global,
                "archivedMotorcycles": archived_count_global,
                "documents": docs_count_global,
                "documentAssignments": doc_assignments_count_global,
                "maintenance": maintenance_count_total_global,
                "issues": issues_count_total_global,
                "openIssues": open_issues_count_total_global,
                "locations": locations_count_total_global,
                "locationHistory": location_history_count_total_global,
                "torqueSpecs": torque_specs_count_total_global,
            }
        },
        "avgMotoPerUser": avg_moto_per_user,
        "avgDocsPerUser": avg_docs_per_user,
        "motorcycles": motorcycles_json,
        "issues": issues.iter().map(|r| json!({
            "id": r.get::<i64, _>("id"),
            "motorcycleId": r.get::<i64, _>("motorcycleId"),
            "status": r.get::<String, _>("status"),
            "odo": r.get::<i64, _>("odo"),
            "date": r.get::<Option<String>, _>("date"),
        })).collect::<Vec<Value>>(),
        "maintenance": maintenance.iter().map(maintenance_row_to_value).collect::<Vec<Value>>(),
        "locationHistory": location_history.iter().map(|r| json!({
            "motorcycleId": r.get::<i64, _>("motorcycleId"),
            "odometer": r.get::<Option<i64>, _>("odometer"),
            "date": r.get::<String, _>("date"),
        })).collect::<Vec<Value>>(),
        "locations": locations.iter().map(|r| json!({
            "id": r.get::<i64, _>("id"),
            "name": r.get::<String, _>("name"),
            "countryCode": r.get::<String, _>("countryCode"),
        })).collect::<Vec<Value>>(),
        "settings": settings_row.map(|r| json!({
            "userId": r.get::<i64, _>("userId"),
        })),
        "version": config.app_version,
    })))
}
use chrono::Utc;
