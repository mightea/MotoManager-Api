//! `GET /api/map`: trip analytics for the webapp's map page (per-location
//! aggregates, legs between fuel stops, tours, yearly range stats, riding
//! calendar). All computation lives in `crate::map_analysis`; this module only
//! loads the user's rows and shapes them into `MapInput`.

use axum::{extract::State, Json};
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    auth::AuthUser,
    error::AppResult,
    map_analysis::{
        analyze, FuelRecord, MaintenanceVisit, MapInput, MapPayload, MotorcycleRef, ParkedVisit,
    },
    models::Location,
};

#[derive(sqlx::FromRow)]
struct MotorcycleRow {
    id: i64,
    make: String,
    model: String,
    status: String,
}

#[derive(sqlx::FromRow)]
struct FuelRow {
    id: i64,
    #[sqlx(rename = "motorcycleId")]
    motorcycle_id: i64,
    date: String,
    odo: i64,
    #[sqlx(rename = "locationId")]
    location_id: Option<i64>,
    #[sqlx(rename = "fuelAmount")]
    fuel_amount: Option<f64>,
    cost: Option<f64>,
    #[sqlx(rename = "normalizedCost")]
    normalized_cost: Option<f64>,
    #[sqlx(rename = "fuelConsumption")]
    fuel_consumption: Option<f64>,
}

#[derive(sqlx::FromRow)]
struct VisitRow {
    id: i64,
    #[sqlx(rename = "motorcycleId")]
    motorcycle_id: i64,
    date: String,
    #[sqlx(rename = "locationId")]
    location_id: i64,
    #[sqlx(rename = "type")]
    record_type: String,
    cost: Option<f64>,
    #[sqlx(rename = "normalizedCost")]
    normalized_cost: Option<f64>,
    #[sqlx(rename = "partsCost")]
    parts_cost: Option<f64>,
}

#[derive(sqlx::FromRow)]
struct ParkedRow {
    id: i64,
    #[sqlx(rename = "motorcycleId")]
    motorcycle_id: i64,
    date: String,
    #[sqlx(rename = "locationId")]
    location_id: i64,
}

/// Load everything the analysis needs for one user and run it. Shared by the
/// REST handler and the MCP `get_riding_summary` tool.
pub async fn compute_map(pool: &SqlitePool, user_id: i64) -> AppResult<MapPayload> {
    let motorcycles = sqlx::query_as::<_, MotorcycleRow>(
        "SELECT id, make, model, status FROM motorcycles WHERE userId = ? ORDER BY id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let locations = sqlx::query_as::<_, Location>("SELECT * FROM locations WHERE userId = ?")
        .bind(user_id)
        .fetch_all(pool)
        .await?;

    let fuel_rows = sqlx::query_as::<_, FuelRow>(
        "SELECT id, motorcycleId, date, odo, locationId, fuelAmount, cost, normalizedCost, \
         fuelConsumption FROM maintenanceRecords \
         WHERE type = 'fuel' AND deletedAt IS NULL \
         AND motorcycleId IN (SELECT id FROM motorcycles WHERE userId = ?)",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let visit_rows = sqlx::query_as::<_, VisitRow>(
        "SELECT id, motorcycleId, date, locationId, type, cost, normalizedCost, partsCost \
         FROM maintenanceRecords \
         WHERE type != 'fuel' AND locationId IS NOT NULL AND deletedAt IS NULL \
         AND motorcycleId IN (SELECT id FROM motorcycles WHERE userId = ?)",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let parked_rows = sqlx::query_as::<_, ParkedRow>(
        "SELECT id, motorcycleId, date, locationId FROM locationRecords \
         WHERE motorcycleId IN (SELECT id FROM motorcycles WHERE userId = ?)",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut maintenance_visits = Vec::new();
    let mut parked_visits: Vec<ParkedVisit> = parked_rows
        .into_iter()
        .map(|r| ParkedVisit {
            id: r.id,
            motorcycle_id: r.motorcycle_id,
            date: r.date,
            location_id: r.location_id,
        })
        .collect();
    for row in visit_rows {
        // A maintenance record of type `location` is a "the bike is here"
        // marker, not a workshop visit (same reading as the home dashboard).
        if row.record_type == "location" {
            parked_visits.push(ParkedVisit {
                // Negative ids keep these distinct from locationRecords ids in
                // the home-selection tie-break without colliding.
                id: -row.id,
                motorcycle_id: row.motorcycle_id,
                date: row.date,
                location_id: row.location_id,
            });
        } else {
            maintenance_visits.push(MaintenanceVisit {
                motorcycle_id: row.motorcycle_id,
                date: row.date,
                location_id: row.location_id,
                cost: row.normalized_cost.or(row.cost).unwrap_or(0.0)
                    + row.parts_cost.unwrap_or(0.0),
            });
        }
    }

    let input = MapInput {
        motorcycles: motorcycles
            .into_iter()
            .map(|m| MotorcycleRef {
                id: m.id,
                make: m.make,
                model: m.model,
                status: m.status,
            })
            .collect(),
        locations,
        fuel_records: fuel_rows
            .into_iter()
            .map(|r| FuelRecord {
                id: r.id,
                motorcycle_id: r.motorcycle_id,
                date: r.date,
                odo: r.odo,
                location_id: r.location_id,
                liters: r.fuel_amount,
                cost: r.normalized_cost.or(r.cost),
                consumption: r.fuel_consumption,
            })
            .collect(),
        maintenance_visits,
        parked_visits,
    };
    Ok(analyze(input))
}

pub async fn get_map(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let payload = compute_map(&pool, user.id).await?;
    Ok(Json(json!(payload)))
}
