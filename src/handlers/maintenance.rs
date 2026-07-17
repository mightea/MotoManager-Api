use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{SecondsFormat, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::{locations::verify_location_ownership, motorcycles::verify_motorcycle_ownership},
    models::MaintenanceRecord,
};

/// Server-authoritative sync timestamp, kept in one canonical format
/// (RFC3339, millis, `Z`) so lexical `updatedAt > since` comparisons are
/// chronologically correct. Mirrors the backfill format in migration 011.
pub(crate) fn sync_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceFilter {
    pub types: Option<String>,
    /// When present, return rows changed after this `updatedAt` cursor,
    /// including soft-deleted tombstones, for incremental sync.
    pub since: Option<String>,
}

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
    Query(filter): Query<MaintenanceFilter>,
) -> AppResult<Json<Value>> {
    tracing::debug!(
        "Listing maintenance records for motorcycle ID: {} for user: {} with filter: {:?}",
        motorcycle_id,
        user.id,
        filter
    );
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let mut query_str = "SELECT * FROM maintenanceRecords WHERE motorcycleId = ?".to_string();
    let mut types_list = Vec::new();

    // Incremental sync: with ?since, return changed rows (incl. tombstones);
    // otherwise the normal listing hides soft-deleted rows.
    if filter.since.is_some() {
        query_str.push_str(" AND updatedAt > ?");
    } else {
        query_str.push_str(" AND deletedAt IS NULL");
    }

    if let Some(types) = filter.types {
        if !types.is_empty() {
            let parts: Vec<String> = types.split(',').map(|s| s.trim().to_string()).collect();
            if !parts.is_empty() {
                let placeholders: Vec<&str> = vec!["?"; parts.len()];
                query_str.push_str(&format!(" AND type IN ({})", placeholders.join(", ")));
                types_list = parts;
            }
        }
    }

    query_str.push_str(" ORDER BY date DESC, id DESC");

    let mut query =
        sqlx::query_as::<_, MaintenanceRecord>(sqlx::AssertSqlSafe(query_str)).bind(motorcycle_id);

    // Bind order must match the placeholder order above: motorcycleId, since, types.
    if let Some(since) = filter.since {
        query = query.bind(since);
    }
    for t in types_list {
        query = query.bind(t);
    }

    let records = query.fetch_all(&pool).await?;

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
    pub location_id: Option<i64>,
    pub fuel_type: Option<String>,
    pub fuel_amount: Option<f64>,
    pub price_per_unit: Option<f64>,
    pub fuel_consumption: Option<f64>,
    pub trip_distance: Option<f64>,
    pub fuel_additive_added: Option<bool>,
    pub lead_substitute_added: Option<bool>,
    pub parent_id: Option<i64>,
    pub bundled_items: Option<Vec<String>>,
    /// Client-generated idempotency key (UUID). Retried creates with the same
    /// value return the existing record instead of duplicating.
    pub client_id: Option<String>,
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
    if let Some(lid) = body.location_id {
        verify_location_ownership(&pool, lid, user.id).await?;
    }

    // Idempotency: a retried create with the same clientId returns the
    // already-stored record instead of inserting a duplicate.
    if let Some(client_id) = &body.client_id {
        if let Some(existing) = sqlx::query_as::<_, MaintenanceRecord>(
            "SELECT * FROM maintenanceRecords WHERE clientId = ? AND motorcycleId = ?",
        )
        .bind(client_id)
        .bind(motorcycle_id)
        .fetch_optional(&pool)
        .await?
        {
            return Ok((
                StatusCode::CREATED,
                Json(json!({ "maintenanceRecord": existing })),
            ));
        }
    }

    let date = body
        .date
        .ok_or_else(|| AppError::BadRequest("date is required".to_string()))?;
    let odo = body
        .odo
        .ok_or_else(|| AppError::BadRequest("odo is required".to_string()))?;
    let record_type = body
        .record_type
        .ok_or_else(|| AppError::BadRequest("type is required".to_string()))?;

    let now = sync_now();
    let mut tx = pool.begin().await?;

    let id = sqlx::query(
        "INSERT INTO maintenanceRecords \
         (date, odo, motorcycleId, cost, normalizedCost, currency, description, type, \
          brand, model, tirePosition, tireSize, dotCode, batteryType, fluidType, viscosity, \
          oilType, locationId, fuelType, fuelAmount, pricePerUnit, \
          fuelConsumption, tripDistance, fuelAdditiveAdded, leadSubstituteAdded, \
          parentId, clientId, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(body.location_id)
    .bind(&body.fuel_type)
    .bind(body.fuel_amount)
    .bind(body.price_per_unit)
    .bind(body.fuel_consumption)
    .bind(body.trip_distance)
    .bind(body.fuel_additive_added.unwrap_or(false))
    .bind(body.lead_substitute_added.unwrap_or(false))
    .bind(body.parent_id)
    .bind(&body.client_id)
    .bind(&now)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    if let Some(bundled) = body.bundled_items {
        for item in bundled {
            let (rec_type, fluid_type) = match item.as_str() {
                "engineoil"
                | "gearboxoil"
                | "finaldriveoil"
                | "finaldrivegearboxoil"
                | "forkoil"
                | "brakefluid"
                | "coolant" => ("fluid", Some(item)),
                "chain" => ("chain", None),
                _ => ("general", None),
            };

            sqlx::query(
                "INSERT INTO maintenanceRecords (date, odo, motorcycleId, type, fluidType, parentId, updatedAt) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&date)
            .bind(odo)
            .bind(motorcycle_id)
            .bind(rec_type)
            .bind(fluid_type)
            .bind(id)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;

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
    if let Some(lid) = body.location_id {
        verify_location_ownership(&pool, lid, user.id).await?;
    }

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
    let location_id: Option<i64> = body.location_id.or(existing.location_id);
    let fuel_type: Option<String> = body.fuel_type.or(existing.fuel_type);
    let fuel_amount: Option<f64> = body.fuel_amount.or(existing.fuel_amount);
    let price_per_unit: Option<f64> = body.price_per_unit.or(existing.price_per_unit);
    let fuel_consumption: Option<f64> = body.fuel_consumption.or(existing.fuel_consumption);
    let trip_distance: Option<f64> = body.trip_distance.or(existing.trip_distance);
    let fuel_additive_added = body
        .fuel_additive_added
        .unwrap_or(existing.fuel_additive_added);
    let lead_substitute_added = body
        .lead_substitute_added
        .unwrap_or(existing.lead_substitute_added);
    let parent_id: Option<i64> = body.parent_id.or(existing.parent_id);

    let now = sync_now();
    let mut tx = pool.begin().await?;

    sqlx::query(
        "UPDATE maintenanceRecords SET \
         date = ?, odo = ?, cost = ?, normalizedCost = ?, currency = ?, description = ?, \
         type = ?, brand = ?, model = ?, tirePosition = ?, tireSize = ?, dotCode = ?, \
         batteryType = ?, fluidType = ?, viscosity = ?, oilType = ?, \
         locationId = ?, fuelType = ?, fuelAmount = ?, pricePerUnit = ?, \
         fuelConsumption = ?, tripDistance = ?, fuelAdditiveAdded = ?, \
         leadSubstituteAdded = ?, parentId = ?, updatedAt = ? \
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
    .bind(location_id)
    .bind(&fuel_type)
    .bind(fuel_amount)
    .bind(price_per_unit)
    .bind(fuel_consumption)
    .bind(trip_distance)
    .bind(fuel_additive_added)
    .bind(lead_substitute_added)
    .bind(parent_id)
    .bind(&now)
    .bind(mid)
    .execute(&mut *tx)
    .await?;

    // Reconcile bundled items if provided
    if let Some(bundled) = body.bundled_items {
        // Fetch existing child records
        let existing_children = sqlx::query!(
            "SELECT id, type AS record_type, fluidType FROM maintenanceRecords WHERE parentId = ?",
            mid
        )
        .fetch_all(&mut *tx)
        .await?;

        // 1. Delete children that are no longer in the bundled list
        for child in &existing_children {
            let item_key = match child.record_type.as_str() {
                "fluid" => child.fluidType.clone().unwrap_or_default(),
                "chain" => "chain".to_string(),
                _ => "general".to_string(), // Simplified, might need more robust matching
            };

            if !bundled.contains(&item_key) {
                sqlx::query!("DELETE FROM maintenanceRecords WHERE id = ?", child.id)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        // 2. Add new children that don't exist yet
        for item in bundled {
            let (rec_type, fluid_type) = match item.as_str() {
                "engineoil"
                | "gearboxoil"
                | "finaldriveoil"
                | "finaldrivegearboxoil"
                | "forkoil"
                | "brakefluid"
                | "coolant" => ("fluid", Some(item)),
                "chain" => ("chain", None),
                _ => ("general", None),
            };

            let already_exists = existing_children.iter().any(|c| {
                if rec_type == "fluid" {
                    c.record_type == "fluid" && c.fluidType.as_deref() == fluid_type.as_deref()
                } else {
                    c.record_type == rec_type
                }
            });

            if !already_exists {
                sqlx::query!(
                    "INSERT INTO maintenanceRecords (date, odo, motorcycleId, type, fluidType, parentId) \
                     VALUES (?, ?, ?, ?, ?, ?)",
                    date,
                    odo,
                    motorcycle_id,
                    rec_type,
                    fluid_type,
                    mid
                )
                .execute(&mut *tx)
                .await?;
            } else {
                // Update existing child's date and odo to match parent
                sqlx::query!(
                    "UPDATE maintenanceRecords SET date = ?, odo = ? WHERE parentId = ? AND type = ? AND (fluidType = ? OR fluidType IS NULL)",
                    date,
                    odo,
                    mid,
                    rec_type,
                    fluid_type
                )
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    tx.commit().await?;

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

    // Soft-delete: keep the row as a tombstone (with bumped updatedAt) so
    // offline clients learn about the deletion on their next ?since pull.
    let now = sync_now();
    let mut tx = pool.begin().await?;

    let result = sqlx::query(
        "UPDATE maintenanceRecords SET deletedAt = ?, updatedAt = ? \
         WHERE id = ? AND motorcycleId = ? AND deletedAt IS NULL",
    )
    .bind(&now)
    .bind(&now)
    .bind(mid)
    .bind(motorcycle_id)
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Maintenance record not found".to_string(),
        ));
    }

    // Deleting a repair restores the parts it consumed: tombstone the linked
    // consumptions so on-hand (a pure derivation) recovers and offline clients
    // remove them via tombstone pull.
    sqlx::query(
        "UPDATE partConsumptions SET deletedAt = ?, updatedAt = ? \
         WHERE maintenanceRecordId = ? AND deletedAt IS NULL",
    )
    .bind(&now)
    .bind(&now)
    .bind(mid)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    tracing::info!("Maintenance record soft-deleted ID: {}", mid);
    Ok(Json(json!({ "message": "Maintenance record deleted" })))
}
