use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::motorcycles::verify_motorcycle_ownership,
    models::MaintenanceRecord,
};

async fn recalculate_fuel_consumption(
    pool: &SqlitePool,
    record_id: i64,
    motorcycle_id: i64,
    current_odo: i64,
    fuel_amount: f64,
    provided_trip_distance: Option<f64>,
) -> AppResult<()> {
    let prev_row = sqlx::query!(
        "SELECT odo FROM maintenanceRecords \
         WHERE motorcycleId = ? AND type = 'fuel' AND odo < ? AND id != ? \
         ORDER BY odo DESC LIMIT 1",
        motorcycle_id,
        current_odo,
        record_id
    )
    .fetch_optional(pool)
    .await?;

    let trip_distance = if let Some(d) = provided_trip_distance {
        d
    } else if let Some(prev) = prev_row {
        (current_odo - prev.odo) as f64
    } else {
        return Ok(());
    };

    if trip_distance <= 0.0 {
        return Ok(());
    }

    let fuel_consumption = (fuel_amount / trip_distance) * 100.0;

    sqlx::query!(
        "UPDATE maintenanceRecords SET fuelConsumption = ?, tripDistance = ? WHERE id = ?",
        fuel_consumption,
        trip_distance,
        record_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_maintenance(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
) -> AppResult<Json<Value>> {
    tracing::debug!(
        "Listing maintenance records for motorcycle ID: {} for user: {}",
        motorcycle_id,
        user.id
    );
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let records = sqlx::query_as::<_, MaintenanceRecord>(
        "SELECT * FROM maintenanceRecords WHERE motorcycleId = ? ORDER BY date DESC, id DESC",
    )
    .bind(motorcycle_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(json!({ "maintenanceRecords": records })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceRequest {
    pub date: Option<String>,
    pub odo: Option<i64>,
    #[serde(rename = "type")]
    pub record_type: Option<String>,
    pub cost: Option<f64>,
    pub normalized_cost: Option<f64>,
    pub currency: Option<String>,
    pub description: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub tire_position: Option<String>,
    pub tire_size: Option<String>,
    pub dot_code: Option<String>,
    pub battery_type: Option<String>,
    pub fluid_type: Option<String>,
    pub viscosity: Option<String>,
    pub oil_type: Option<String>,
    pub inspection_location: Option<String>,
    pub location_id: Option<i64>,
    pub fuel_type: Option<String>,
    pub fuel_amount: Option<f64>,
    pub price_per_unit: Option<f64>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub location_name: Option<String>,
    pub fuel_consumption: Option<f64>,
    pub trip_distance: Option<f64>,
}

pub async fn create_maintenance(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Json(body): Json<MaintenanceRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    tracing::info!(
        "Creating maintenance record for motorcycle ID: {} for user: {}",
        motorcycle_id,
        user.id
    );
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let date = body
        .date
        .ok_or_else(|| AppError::BadRequest("date is required".to_string()))?;
    let odo = body
        .odo
        .ok_or_else(|| AppError::BadRequest("odo is required".to_string()))?;
    let record_type = body
        .record_type
        .ok_or_else(|| AppError::BadRequest("type is required".to_string()))?;

    let id = sqlx::query(
        "INSERT INTO maintenanceRecords \
         (date, odo, motorcycleId, cost, normalizedCost, currency, description, type, \
          brand, model, tirePosition, tireSize, dotCode, batteryType, fluidType, viscosity, \
          oilType, inspectionLocation, locationId, fuelType, fuelAmount, pricePerUnit, \
          latitude, longitude, locationName, fuelConsumption, tripDistance) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&date)
    .bind(odo)
    .bind(motorcycle_id)
    .bind(body.cost)
    .bind(body.normalized_cost)
    .bind(&body.currency)
    .bind(&body.description)
    .bind(&record_type)
    .bind(&body.brand)
    .bind(&body.model)
    .bind(&body.tire_position)
    .bind(&body.tire_size)
    .bind(&body.dot_code)
    .bind(&body.battery_type)
    .bind(&body.fluid_type)
    .bind(&body.viscosity)
    .bind(&body.oil_type)
    .bind(&body.inspection_location)
    .bind(body.location_id)
    .bind(&body.fuel_type)
    .bind(body.fuel_amount)
    .bind(body.price_per_unit)
    .bind(body.latitude)
    .bind(body.longitude)
    .bind(&body.location_name)
    .bind(body.fuel_consumption)
    .bind(body.trip_distance)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    if record_type == "fuel" {
        if let Some(fuel_amount) = body.fuel_amount {
            let _ = recalculate_fuel_consumption(
                &pool,
                id,
                motorcycle_id,
                odo,
                fuel_amount,
                body.trip_distance,
            )
            .await;
        }
    }

    let record =
        sqlx::query_as::<_, MaintenanceRecord>("SELECT * FROM maintenanceRecords WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await?;

    tracing::info!(
        "Maintenance record created ID: {} for motorcycle ID: {}",
        id,
        motorcycle_id
    );
    Ok((
        StatusCode::CREATED,
        Json(json!({ "maintenanceRecord": record })),
    ))
}

pub async fn update_maintenance(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((motorcycle_id, mid)): Path<(i64, i64)>,
    Json(body): Json<MaintenanceRequest>,
) -> AppResult<Json<Value>> {
    tracing::info!(
        "Updating maintenance record ID: {} for motorcycle ID: {} for user: {}",
        mid,
        motorcycle_id,
        user.id
    );
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let existing = sqlx::query_as::<_, MaintenanceRecord>(
        "SELECT * FROM maintenanceRecords WHERE id = ? AND motorcycleId = ?",
    )
    .bind(mid)
    .bind(motorcycle_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Maintenance record not found".to_string()))?;

    let date = body.date.unwrap_or(existing.date);
    let odo = body.odo.unwrap_or(existing.odo);
    let record_type = body.record_type.unwrap_or(existing.record_type);
    let cost = body.cost.or(existing.cost);
    let normalized_cost = body.normalized_cost.or(existing.normalized_cost);
    let currency: Option<String> = body.currency.or(existing.currency);
    let description: Option<String> = body.description.or(existing.description);
    let brand: Option<String> = body.brand.or(existing.brand);
    let model: Option<String> = body.model.or(existing.model);
    let tire_position: Option<String> = body.tire_position.or(existing.tire_position);
    let tire_size: Option<String> = body.tire_size.or(existing.tire_size);
    let dot_code: Option<String> = body.dot_code.or(existing.dot_code);
    let battery_type: Option<String> = body.battery_type.or(existing.battery_type);
    let fluid_type: Option<String> = body.fluid_type.or(existing.fluid_type);
    let viscosity: Option<String> = body.viscosity.or(existing.viscosity);
    let oil_type: Option<String> = body.oil_type.or(existing.oil_type);
    let inspection_location: Option<String> =
        body.inspection_location.or(existing.inspection_location);
    let location_id: Option<i64> = body.location_id.or(existing.location_id);
    let fuel_type: Option<String> = body.fuel_type.or(existing.fuel_type);
    let fuel_amount: Option<f64> = body.fuel_amount.or(existing.fuel_amount);
    let price_per_unit: Option<f64> = body.price_per_unit.or(existing.price_per_unit);
    let latitude: Option<f64> = body.latitude.or(existing.latitude);
    let longitude: Option<f64> = body.longitude.or(existing.longitude);
    let location_name: Option<String> = body.location_name.or(existing.location_name);
    let fuel_consumption: Option<f64> = body.fuel_consumption.or(existing.fuel_consumption);
    let trip_distance: Option<f64> = body.trip_distance.or(existing.trip_distance);

    sqlx::query(
        "UPDATE maintenanceRecords SET \
         date = ?, odo = ?, cost = ?, normalizedCost = ?, currency = ?, description = ?, \
         type = ?, brand = ?, model = ?, tirePosition = ?, tireSize = ?, dotCode = ?, \
         batteryType = ?, fluidType = ?, viscosity = ?, oilType = ?, inspectionLocation = ?, \
         locationId = ?, fuelType = ?, fuelAmount = ?, pricePerUnit = ?, latitude = ?, \
         longitude = ?, locationName = ?, fuelConsumption = ?, tripDistance = ? \
         WHERE id = ?",
    )
    .bind(&date)
    .bind(odo)
    .bind(cost)
    .bind(normalized_cost)
    .bind(&currency)
    .bind(&description)
    .bind(&record_type)
    .bind(&brand)
    .bind(&model)
    .bind(&tire_position)
    .bind(&tire_size)
    .bind(&dot_code)
    .bind(&battery_type)
    .bind(&fluid_type)
    .bind(&viscosity)
    .bind(&oil_type)
    .bind(&inspection_location)
    .bind(location_id)
    .bind(&fuel_type)
    .bind(fuel_amount)
    .bind(price_per_unit)
    .bind(latitude)
    .bind(longitude)
    .bind(&location_name)
    .bind(fuel_consumption)
    .bind(trip_distance)
    .bind(mid)
    .execute(&pool)
    .await?;

    if record_type == "fuel" {
        if let Some(fa) = fuel_amount {
            let _ = recalculate_fuel_consumption(
                &pool,
                mid,
                motorcycle_id,
                odo,
                fa,
                body.trip_distance,
            )
            .await;
        }
    }

    let record =
        sqlx::query_as::<_, MaintenanceRecord>("SELECT * FROM maintenanceRecords WHERE id = ?")
            .bind(mid)
            .fetch_one(&pool)
            .await?;

    tracing::info!("Maintenance record updated ID: {}", mid);
    Ok(Json(json!({ "maintenanceRecord": record })))
}

pub async fn delete_maintenance(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((motorcycle_id, mid)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    tracing::info!(
        "Deleting maintenance record ID: {} for motorcycle ID: {} for user: {}",
        mid,
        motorcycle_id,
        user.id
    );
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let result = sqlx::query("DELETE FROM maintenanceRecords WHERE id = ? AND motorcycleId = ?")
        .bind(mid)
        .bind(motorcycle_id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Maintenance record not found".to_string(),
        ));
    }

    tracing::info!("Maintenance record deleted ID: {}", mid);
    Ok(Json(json!({ "message": "Maintenance record deleted" })))
}
