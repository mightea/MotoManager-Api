use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use chrono::{Datelike, Utc, NaiveDate, DateTime};

use crate::{
    auth::AuthUser,
    config::Config,
    error::AppResult,
    handlers::motorcycles::maintenance_row_to_value,
};

fn parse_year(date_str: &str) -> Option<i32> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
        return Some(dt.year());
    }
    if let Ok(d) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        return Some(d.year());
    }
    // Fallback: first 4 digits
    if date_str.len() >= 4 {
        date_str[0..4].parse::<i32>().ok()
    } else {
        None
    }
}

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

    // 3. Perform Aggregations
    let current_year = Utc::now().year();
    let mut yearly_map: HashMap<i32, Value> = HashMap::new();
    let mut total_km_overall = 0i64;
    let mut total_km_this_year = 0i64;
    let mut total_cost_overall = 0.0f64;
    let mut total_cost_this_year = 0.0f64;
    let mut veteran_count = 0i64;
    let mut total_active_issues = 0i64;

    // Pre-process yearly structure
    let mut start_year = current_year;
    for r in &motorcycles {
        if let Some(date_str) = r.get::<Option<String>, _>("purchaseDate") {
            if let Some(y) = parse_year(&date_str) {
                if y < start_year { start_year = y; }
            }
        }
    }
    for y in start_year..=current_year {
        yearly_map.insert(y, json!({
            "year": y,
            "distance": 0,
            "cost": 0.0,
            "motorcycleCount": 0,
            "motorcycles": [],
            "records": []
        }));
    }

    // Process each motorcycle
    let mut motorcycles_json = Vec::new();
    for r in &motorcycles {
        let moto_id: i64 = r.get("id");
        let initial_odo: i64 = r.get("initialOdo");
        let is_veteran: bool = r.get("isVeteran");
        if is_veteran { veteran_count += 1; }
        
        let purchase_year = r.get::<Option<String>, _>("purchaseDate")
            .and_then(|d| parse_year(&d))
            .unwrap_or(start_year);

        // Map max ODO per year for this bike
        let mut odo_by_year: HashMap<i32, i64> = HashMap::new();
        odo_by_year.insert(purchase_year - 1, initial_odo); // Baseline

        for m in maintenance.iter().filter(|m| m.get::<i64, _>("motorcycleId") == moto_id) {
            let odo: i64 = m.get("odo");
            if let Some(y) = parse_year(&m.get::<String, _>("date")) {
                let current = odo_by_year.get(&y).cloned().unwrap_or(0);
                if odo > current { odo_by_year.insert(y, odo); }
            }
        }

        // Calculate yearly metrics for this bike
        let mut last_odo = initial_odo;
        let mut bike_max_odo = initial_odo;

        for y in start_year..=current_year {
            if y >= purchase_year {
                let yearly_max = odo_by_year.get(&y).cloned().unwrap_or(last_odo);
                let distance = yearly_max - last_odo;
                
                let yearly_cost = maintenance.iter()
                    .filter(|m| m.get::<i64, _>("motorcycleId") == moto_id)
                    .filter(|m| parse_year(&m.get::<String, _>("date")) == Some(y))
                    .map(|m| m.get::<Option<f64>, _>("normalizedCost").or_else(|| m.get::<Option<f64>, _>("cost")).unwrap_or(0.0))
                    .sum::<f64>();

                if let Some(y_stats) = yearly_map.get_mut(&y) {
                    if let Some(obj) = y_stats.as_object_mut() {
                        obj["motorcycleCount"] = json!(obj["motorcycleCount"].as_i64().unwrap_or(0) + 1);
                        obj["distance"] = json!(obj["distance"].as_i64().unwrap_or(0) + distance);
                        obj["cost"] = json!(obj["cost"].as_f64().unwrap_or(0.0) + yearly_cost);
                        
                        let moto_list = obj["motorcycles"].as_array_mut().unwrap();
                        moto_list.push(json!({
                            "id": moto_id,
                            "make": r.get::<String, _>("make"),
                            "model": r.get::<String, _>("model"),
                            "distance": distance,
                            "cost": yearly_cost
                        }));
                    }
                }

                if y == current_year { total_km_this_year += distance; }
                last_odo = yearly_max;
                if yearly_max > bike_max_odo { bike_max_odo = yearly_max; }
            }
        }

        total_km_overall += bike_max_odo - initial_odo;

        motorcycles_json.push(json!({
            "id": moto_id,
            "make": r.get::<String, _>("make"),
            "model": r.get::<String, _>("model"),
            "fabricationDate": r.get::<Option<String>, _>("modelYear"),
            "userId": r.get::<i64, _>("userId"),
            "image": r.get::<Option<String>, _>("image").map(|i| format!("/images/{}", i.replace("/data/images/", "").replace("data/images/", ""))),
            "isVeteran": is_veteran,
            "isArchived": r.get::<bool, _>("isArchived"),
            "initialOdo": initial_odo,
            "odometer": bike_max_odo,
            "odometerThisYear": odo_by_year.get(&current_year).map(|&v| v - odo_by_year.get(&(current_year - 1)).cloned().unwrap_or(initial_odo)).unwrap_or(0),
        }));
    }

    // Process costs and yearly fleet stats (attach records)
    for m in &maintenance {
        if let Some(y) = parse_year(&m.get::<String, _>("date")) {
            let cost = m.get::<Option<f64>, _>("normalizedCost").or_else(|| m.get::<Option<f64>, _>("cost")).unwrap_or(0.0);
            total_cost_overall += cost;
            if y == current_year { total_cost_this_year += cost; }

            if let Some(y_stats) = yearly_map.get_mut(&y) {
                if let Some(obj) = y_stats.as_object_mut() {
                    let records = obj["records"].as_array_mut().unwrap();
                    records.push(maintenance_row_to_value(m));
                }
            }
        }
    }

    for i in &issues {
        let status: String = i.get("status");
        if status != "done" { total_active_issues += 1; }
    }

    let mut yearly_vec: Vec<Value> = yearly_map.into_values().collect();
    yearly_vec.sort_by_key(|v| v["year"].as_i64().unwrap_or(0) * -1);

    // Max values for charts
    let max_yearly_distance = yearly_vec.iter().map(|v| v["distance"].as_i64().unwrap_or(0)).max().unwrap_or(0);
    let max_yearly_cost = yearly_vec.iter().map(|v| v["cost"].as_f64().unwrap_or(0.0)).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);
    let max_yearly_count = yearly_vec.iter().map(|v| v["motorcycleCount"].as_i64().unwrap_or(0)).max().unwrap_or(0);

    // Top Rider
    let top_rider = yearly_vec.first()
        .and_then(|y| y["motorcycles"].as_array())
        .and_then(|motos| motos.iter().max_by_key(|m| m["distance"].as_i64().unwrap_or(0)))
        .cloned();

    let stats_data = json!({
        "year": current_year,
        "totalMotorcycles": motorcycles.len(),
        "totalKmThisYear": total_km_this_year,
        "totalKmOverall": total_km_overall,
        "totalActiveIssues": total_active_issues,
        "totalMaintenanceCostThisYear": total_cost_this_year,
        "veteranCount": veteran_count,
        "topRider": top_rider,
        "yearly": yearly_vec,
        "overall": {
            "totalDistance": total_km_overall,
            "totalCost": total_cost_overall,
            "maxYearlyDistance": max_yearly_distance,
            "maxYearlyCost": max_yearly_cost,
            "maxYearlyCount": max_yearly_count,
        },
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
    });

    Ok(Json(json!({
        "stats": stats_data,
        "fleetStats": stats_data,
        "motorcycles": motorcycles_json,
        "version": config.app_version,
    })))
}
