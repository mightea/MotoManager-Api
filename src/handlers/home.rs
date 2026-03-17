use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use chrono::{Datelike, Utc, NaiveDate, DateTime};
use std::collections::HashMap;

use crate::{
    auth::AuthUser,
    config::Config,
    error::AppResult,
    models::{Motorcycle, MaintenanceRecord, Issue, Location, LocationRecord, UserSettings},
};

fn parse_date(date_str: &str) -> Option<NaiveDate> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
        return Some(dt.date_naive());
    }
    if let Ok(d) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        return Some(d);
    }
    // Fallback: first 10 digits
    if date_str.len() >= 10 {
        NaiveDate::parse_from_str(&date_str[0..10], "%Y-%m-%d").ok()
    } else {
        None
    }
}

fn format_image_url(image: Option<String>) -> Option<String> {
    image.map(|i| format!("/images/{}", i.replace("/data/images/", "").replace("data/images/", "")))
}

pub async fn get_home_data(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let current_year = Utc::now().year();
    let today = Utc::now().date_naive();

    // 1. Fetch all motorcycles
    let motorcycles = sqlx::query_as::<_, Motorcycle>("SELECT * FROM motorcycles WHERE userId = ?")
        .bind(user.id)
        .fetch_all(&pool)
        .await?;

    // 2. Fetch all related data in bulk to avoid N+1
    let maintenance = sqlx::query_as::<_, MaintenanceRecord>(
        "SELECT * FROM maintenanceRecords WHERE motorcycleId IN (SELECT id FROM motorcycles WHERE userId = ?) ORDER BY date DESC, odo DESC",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    let issues = sqlx::query_as::<_, Issue>(
        "SELECT * FROM issues WHERE motorcycleId IN (SELECT id FROM motorcycles WHERE userId = ?)",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    let locations = sqlx::query_as::<_, Location>(
        "SELECT * FROM locations WHERE userId = ?",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;
    let location_map: HashMap<i64, Location> = locations.into_iter().map(|l| (l.id, l)).collect();

    let location_records = sqlx::query_as::<_, LocationRecord>(
        "SELECT * FROM locationRecords WHERE motorcycleId IN (SELECT id FROM motorcycles WHERE userId = ?) ORDER BY date DESC, id DESC",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    let _settings = sqlx::query_as::<_, UserSettings>(
        "SELECT * FROM userSettings WHERE userId = ?",
    )
    .bind(user.id)
    .fetch_optional(&pool)
    .await?;

    // 3. Process each motorcycle
    let mut motorcycles_json = Vec::new();
    let mut total_km_this_year = 0i64;
    let mut total_km_overall = 0i64;
    let mut total_active_issues = 0i64;
    let mut total_cost_this_year = 0.0f64;
    let mut veteran_count = 0i64;

    for moto in &motorcycles {
        let moto_id = moto.id;
        let initial_odo = moto.initial_odo;
        if moto.is_veteran { veteran_count += 1; }

        let moto_maintenance: Vec<&MaintenanceRecord> = maintenance.iter().filter(|m| m.motorcycle_id == moto_id).collect();
        let moto_issues: Vec<&Issue> = issues.iter().filter(|i| i.motorcycle_id == moto_id).collect();
        let moto_loc_records: Vec<&LocationRecord> = location_records.iter().filter(|r| r.motorcycle_id == moto_id).collect();

        // Latest Odometer
        let max_m_odo = moto_maintenance.iter().map(|m| m.odo).max().unwrap_or(0);
        let max_i_odo = moto_issues.iter().map(|i| i.odo).max().unwrap_or(0);
        let max_l_odo = moto_loc_records.iter().filter_map(|r| r.odometer).max().unwrap_or(0);
        let manual_odo = moto.manual_odo.unwrap_or(0);
        let current_odo = initial_odo.max(max_m_odo).max(max_i_odo).max(max_l_odo).max(manual_odo);

        // Odometer at start of year
        let last_year_odo = moto_maintenance.iter()
            .filter(|m| parse_date(&m.date).map(|d| d.year() < current_year).unwrap_or(false))
            .map(|m| m.odo)
            .max()
            .unwrap_or(initial_odo);
        let odo_this_year = current_odo - last_year_odo;
        
        total_km_this_year += odo_this_year;
        total_km_overall += current_odo - initial_odo;

        // Issues
        let open_issues_count = moto_issues.iter().filter(|i| i.status != "done").count() as i64;
        total_active_issues += open_issues_count;

        // Last Activity
        let last_m_date = moto_maintenance.first().map(|m| m.date.clone());
        let last_i_date = moto_issues.iter().map(|i| i.date.clone()).max();
        let last_l_date = moto_loc_records.first().map(|r| r.date.clone());
        let last_activity = last_m_date.max(last_i_date).max(last_l_date);

        // Current Location
        let latest_loc_record = moto_loc_records.first();
        let current_location_id = latest_loc_record.map(|r| r.location_id);
        let current_location = current_location_id.and_then(|id| location_map.get(&id));

        // Cost this year
        let cost_this_year: f64 = moto_maintenance.iter()
            .filter(|m| parse_date(&m.date).map(|d| d.year() == current_year).unwrap_or(false))
            .map(|m| m.normalized_cost.or(m.cost).unwrap_or(0.0))
            .sum();
        total_cost_this_year += cost_this_year;

        // Next Inspection (simplistic version)
        // In CH, it's 4-3-2-2 years from first registration, or 2 years from last inspection.
        // For simplicity, we'll just check if there was an inspection record.
        let last_inspection = moto_maintenance.iter()
            .filter(|m| m.record_type == "inspection")
            .next();
        
        let next_inspection = if let Some(last) = last_inspection {
            if let Some(last_date) = parse_date(&last.date) {
                let next_date = NaiveDate::from_ymd_opt(last_date.year() + 2, last_date.month(), last_date.day());
                next_date.map(|d| {
                    let is_overdue = d < today;
                    let diff = d.signed_duration_since(today).num_days();
                    let relative_label = if is_overdue {
                        "Überfällig".to_string()
                    } else if diff < 30 {
                        format!("In {} Tagen", diff)
                    } else if diff < 365 {
                        format!("In {} Monaten", diff / 30)
                    } else {
                        format!("In {} Jahren", diff / 365)
                    };

                    json!({
                        "dueDateISO": d.to_string(),
                        "isOverdue": is_overdue,
                        "relativeLabel": relative_label
                    })
                })
            } else {
                None
            }
        } else if let Some(reg_date_str) = &moto.first_registration {
             if let Some(reg_date) = parse_date(reg_date_str) {
                // Simplification: 4 years after first registration if no inspection yet
                let next_date = NaiveDate::from_ymd_opt(reg_date.year() + 4, reg_date.month(), reg_date.day());
                next_date.map(|d| {
                    let is_overdue = d < today;
                    json!({
                        "dueDateISO": d.to_string(),
                        "isOverdue": is_overdue,
                        "relativeLabel": if is_overdue { "Überfällig".to_string() } else { "Anstehend".to_string() }
                    })
                })
             } else {
                 None
             }
        } else {
            None
        };

        motorcycles_json.push(json!({
            "id": moto.id,
            "make": moto.make,
            "model": moto.model,
            "fabricationDate": moto.model_year,
            "userId": moto.user_id,
            "image": format_image_url(moto.image.clone()),
            "isVeteran": moto.is_veteran,
            "isArchived": moto.is_archived,
            "initialOdo": initial_odo,
            "odometer": current_odo,
            "odometerThisYear": odo_this_year,
            "numberOfIssues": open_issues_count,
            "lastActivity": last_activity,
            "nextInspection": next_inspection,
            "currentLocationId": current_location_id,
            "currentLocationName": current_location.map(|l| l.name.clone()),
            "currentLocationCountryCode": current_location.map(|l| l.country_code.clone()),
            "hasOverdueMaintenance": false, // Placeholder for now
            "overdueMaintenanceItems": []   // Placeholder for now
        }));
    }

    let stats_data = json!({
        "year": current_year,
        "totalMotorcycles": motorcycles.len(),
        "totalKmThisYear": total_km_this_year,
        "totalKmOverall": total_km_overall,
        "totalActiveIssues": total_active_issues,
        "totalMaintenanceCostThisYear": total_cost_this_year,
        "veteranCount": veteran_count,
    });

    Ok(Json(json!({
        "stats": stats_data,
        "motorcycles": motorcycles_json,
        "version": config.app_version,
    })))
}
