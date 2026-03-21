use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::HashMap;

use crate::{
    auth::AuthUser,
    config::Config,
    error::AppResult,
    models::{Issue, Location, LocationRecord, MaintenanceRecord, Motorcycle, UserSettings},
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
    image.map(|i| {
        format!(
            "/images/{}",
            i.replace("/data/images/", "").replace("data/images/", "")
        )
    })
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

    let locations = sqlx::query_as::<_, Location>("SELECT * FROM locations WHERE userId = ?")
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

    let _settings =
        sqlx::query_as::<_, UserSettings>("SELECT * FROM userSettings WHERE userId = ?")
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
        if moto.is_veteran {
            veteran_count += 1;
        }

        let moto_maintenance: Vec<&MaintenanceRecord> = maintenance
            .iter()
            .filter(|m| m.motorcycle_id == moto_id)
            .collect();
        let moto_issues: Vec<&Issue> = issues
            .iter()
            .filter(|i| i.motorcycle_id == moto_id)
            .collect();
        let moto_loc_records: Vec<&LocationRecord> = location_records
            .iter()
            .filter(|r| r.motorcycle_id == moto_id)
            .collect();

        // Latest Odometer
        let max_maintenance_odo = moto_maintenance.iter().map(|m| m.odo).max().unwrap_or(0);
        let max_issues_odo = moto_issues.iter().map(|i| i.odo).max().unwrap_or(0);
        let max_location_odo = moto_loc_records
            .iter()
            .filter_map(|r| r.odometer)
            .max()
            .unwrap_or(0);
        let manual_odo = moto.manual_odo.unwrap_or(0);
        let current_odo = initial_odo
            .max(max_maintenance_odo)
            .max(max_issues_odo)
            .max(max_location_odo)
            .max(manual_odo);

        // Odometer at start of year
        let last_year_odo = moto_maintenance
            .iter()
            .filter(|m| {
                parse_date(&m.date)
                    .map(|d| d.year() < current_year)
                    .unwrap_or(false)
            })
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
        let last_maintenance_date = moto_maintenance.first().map(|m| m.date.clone());
        let last_issues_date = moto_issues.iter().map(|i| i.date.clone()).max();
        let last_location_date = moto_loc_records.first().map(|r| r.date.clone());
        let last_activity = last_maintenance_date
            .max(last_issues_date)
            .max(last_location_date);

        // Current Location
        let latest_loc_record = moto_loc_records.first();
        let latest_m_with_loc = moto_maintenance.iter().find(|m| m.location_id.is_some());

        let current_location_id = match (latest_loc_record, latest_m_with_loc) {
            (Some(lr), Some(mr)) => {
                if lr.date >= mr.date {
                    Some(lr.location_id)
                } else {
                    mr.location_id
                }
            }
            (Some(lr), None) => Some(lr.location_id),
            (None, Some(mr)) => mr.location_id,
            (None, None) => None,
        };
        let current_location = current_location_id.and_then(|id| location_map.get(&id));

        // Cost this year
        let cost_this_year: f64 = moto_maintenance
            .iter()
            .filter(|m| {
                parse_date(&m.date)
                    .map(|d| d.year() == current_year)
                    .unwrap_or(false)
            })
            .map(|m| m.normalized_cost.or(m.cost).unwrap_or(0.0))
            .sum();
        total_cost_this_year += cost_this_year;

        // Next Inspection (Swiss Law: 4-3-2-2, Veterans: 6-6-6)
        let last_inspection_record = moto_maintenance
            .iter()
            .find(|m| m.record_type == "inspection");

        let next_inspection = if let Some(last) = last_inspection_record {
            if let Some(last_date) = parse_date(&last.date) {
                let interval = if moto.is_veteran {
                    6
                } else if let Some(reg_date_str) = &moto.first_registration {
                    if let Some(reg_date) = parse_date(reg_date_str) {
                        let years_since_reg =
                            (last_date.signed_duration_since(reg_date).num_days() as f64) / 365.25;
                        if years_since_reg < 5.0 {
                            // First inspection was at 4 years, next is +3
                            3
                        } else {
                            // Second inspection was at 7 years, next is +2
                            2
                        }
                    } else {
                        2
                    }
                } else {
                    2
                };

                let next_date = NaiveDate::from_ymd_opt(
                    last_date.year() + interval,
                    last_date.month(),
                    last_date.day(),
                );
                next_date.map(|d| {
                    let diff = d.signed_duration_since(today).num_days();
                    let is_overdue = diff < 0;

                    let relative_label = if diff == 0 {
                        "Heute fällig".to_string()
                    } else if is_overdue {
                        let abs_days = diff.abs();
                        if abs_days < 14 {
                            format!("seit {} Tagen überfällig", abs_days)
                        } else if abs_days < 60 {
                            format!("seit {} Wochen überfällig", abs_days / 7)
                        } else if abs_days < 730 {
                            format!("seit {} Monaten überfällig", abs_days / 30)
                        } else {
                            format!("seit {} Jahren überfällig", abs_days / 365)
                        }
                    } else {
                        if diff < 14 {
                            format!("in {} Tagen", diff)
                        } else if diff < 60 {
                            format!("in {} Wochen", diff / 7)
                        } else if diff < 730 {
                            format!("in {} Monaten", diff / 30)
                        } else {
                            format!("in {} Jahren", diff / 365)
                        }
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
        } else {
            None
        };

        // Overdue Maintenance Calculation
        let mut overdue_items = Vec::new();
        if let Some(s) = &_settings {
            let moto_maintenance_records: Vec<&MaintenanceRecord> = moto_maintenance.to_vec();

            // Helper to check if a specific type is overdue
            let is_overdue_fn = |record_type: &str,
                                 subtype: Option<&str>,
                                 years: i64,
                                 kms: Option<i64>|
             -> (bool, String) {
                let latest = moto_maintenance_records.iter().find(|r| {
                    r.record_type == record_type
                        && (subtype.is_none()
                            || r.fluid_type.as_deref() == subtype
                            || r.tire_position.as_deref() == subtype)
                });

                if let Some(r) = latest {
                    if let Some(last_date) = parse_date(&r.date) {
                        let next_date = NaiveDate::from_ymd_opt(
                            last_date.year() + (years as i32),
                            last_date.month(),
                            last_date.day(),
                        );
                        if let Some(d) = next_date {
                            if d < today {
                                return (true, format!("{} (Zeit)", record_type));
                            }
                        }
                    }
                    if let Some(interval_kms) = kms {
                        if current_odo - r.odo >= interval_kms {
                            return (true, format!("{} (Kilometer)", record_type));
                        }
                    }
                }
                (false, String::new())
            };

            // 1. Tires
            let (ov, _) = is_overdue_fn("tire", Some("front"), s.tire_interval, s.tire_km_interval);
            if ov {
                overdue_items.push("Vorderreifen".to_string());
            }
            let (ov, _) = is_overdue_fn("tire", Some("rear"), s.tire_interval, s.tire_km_interval);
            if ov {
                overdue_items.push("Hinterreifen".to_string());
            }

            // 2. Battery
            let latest_battery = moto_maintenance_records
                .iter()
                .find(|r| r.record_type == "battery");
            let battery_years = if let Some(b) = latest_battery {
                if b.battery_type.as_deref() == Some("lithium-ion") {
                    s.battery_lithium_interval
                } else {
                    s.battery_default_interval
                }
            } else {
                s.battery_default_interval
            };
            let (ov, _) = is_overdue_fn("battery", None, battery_years, None);
            if ov {
                overdue_items.push("Batterie".to_string());
            }

            // 3. Fluids
            let fluids = [
                (
                    "engineoil",
                    "Motoröl",
                    s.engine_oil_interval,
                    s.engine_oil_km_interval,
                ),
                (
                    "gearboxoil",
                    "Getriebeöl",
                    s.gearbox_oil_interval,
                    s.gearbox_oil_km_interval,
                ),
                (
                    "finaldriveoil",
                    "Kardanöl",
                    s.final_drive_oil_interval,
                    s.final_drive_oil_km_interval,
                ),
                (
                    "forkoil",
                    "Gabelöl",
                    s.fork_oil_interval,
                    s.fork_oil_km_interval,
                ),
                (
                    "brakefluid",
                    "Bremsflüssigkeit",
                    s.brake_fluid_interval,
                    s.brake_fluid_km_interval,
                ),
                (
                    "coolant",
                    "Kühlflüssigkeit",
                    s.coolant_interval,
                    s.coolant_km_interval,
                ),
            ];
            for (f_type, f_label, f_years, f_kms) in fluids {
                let (ov, _) = is_overdue_fn("fluid", Some(f_type), f_years, f_kms);
                if ov {
                    overdue_items.push(f_label.to_string());
                }
            }

            // 4. Chain
            let (ov, _) = is_overdue_fn("chain", None, s.chain_interval, s.chain_km_interval);
            if ov {
                overdue_items.push("Kette".to_string());
            }
        }

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
            "hasOverdueMaintenance": !overdue_items.is_empty(),
            "overdueMaintenanceItems": overdue_items
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
