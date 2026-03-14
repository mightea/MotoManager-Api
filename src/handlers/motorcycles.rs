use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    config::Config,
    error::{AppError, AppResult},
};

async fn save_image(config: &Config, data: Vec<u8>, content_type: &str) -> AppResult<String> {
    let ext = if content_type.contains("png") {
        "png"
    } else if content_type.contains("gif") {
        "gif"
    } else {
        "jpg"
    };
    let filename = format!("{}.{}", Uuid::new_v4(), ext);
    let path = config.images_dir().join(&filename);
    tokio::fs::create_dir_all(config.images_dir()).await?;
    tokio::fs::write(&path, data).await?;
    Ok(filename)
}

fn row_to_motorcycle(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
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
    })
}

const MOTORCYCLE_SELECT: &str = r#"
    SELECT id, make, model, modelYear, userId, vin, engineNumber, vehicleNr,
           numberPlate, image, isVeteran, isArchived, firstRegistration, initialOdo,
           manualOdo, purchaseDate, purchasePrice, normalizedPurchasePrice,
           currencyCode, fuelTankSize
    FROM motorcycles
"#;

pub async fn list_motorcycles(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query(&format!("{} WHERE userId = ? ORDER BY id ASC", MOTORCYCLE_SELECT))
        .bind(user.id)
        .fetch_all(&pool)
        .await?;

    let mut result = Vec::new();
    for r in &rows {
        let moto_id: i64 = r.get("id");

        let open_issues: i64 = sqlx::query(
            "SELECT COUNT(*) as cnt FROM issues WHERE motorcycleId = ? AND status != 'done'",
        )
        .bind(moto_id)
        .fetch_one(&pool)
        .await?
        .get("cnt");

        let maintenance_count: i64 =
            sqlx::query("SELECT COUNT(*) as cnt FROM maintenanceRecords WHERE motorcycleId = ?")
                .bind(moto_id)
                .fetch_one(&pool)
                .await?
                .get("cnt");

        let latest_odo: Option<i64> =
            sqlx::query("SELECT MAX(odo) as max_odo FROM maintenanceRecords WHERE motorcycleId = ?")
                .bind(moto_id)
                .fetch_one(&pool)
                .await?
                .get("max_odo");

        let mut moto = row_to_motorcycle(r);
        if let Some(obj) = moto.as_object_mut() {
            obj.insert("openIssues".to_string(), json!(open_issues));
            obj.insert("maintenanceCount".to_string(), json!(maintenance_count));
            obj.insert("latestOdo".to_string(), json!(latest_odo));
        }
        result.push(moto);
    }

    Ok(Json(json!({ "motorcycles": result })))
}

pub async fn create_motorcycle(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(user): AuthUser,
    mut multipart: Multipart,
) -> AppResult<(StatusCode, Json<Value>)> {
    let mut fields = std::collections::HashMap::<String, String>::new();
    let mut image_filename: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "image" {
            let ct = field.content_type().unwrap_or("image/jpeg").to_string();
            let data = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("File read error: {}", e)))?;
            if !data.is_empty() {
                image_filename = Some(save_image(&config, data.to_vec(), &ct).await?);
            }
        } else {
            let value = field
                .text()
                .await
                .map_err(|e| AppError::BadRequest(format!("Field read error: {}", e)))?;
            fields.insert(name, value);
        }
    }

    let make = fields
        .get("make")
        .cloned()
        .ok_or_else(|| AppError::BadRequest("make is required".to_string()))?;
    let model = fields
        .get("model")
        .cloned()
        .ok_or_else(|| AppError::BadRequest("model is required".to_string()))?;
    let model_year = fields.get("fabricationDate").cloned();
    let is_veteran = fields
        .get("isVeteran")
        .map(|v| v == "true")
        .unwrap_or(false);
    let is_archived = fields
        .get("isArchived")
        .map(|v| v == "true")
        .unwrap_or(false);
    let initial_odo: i64 = fields
        .get("initialOdo")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let purchase_price: Option<f64> = fields.get("purchasePrice").and_then(|v| v.parse().ok());
    let normalized_purchase_price: Option<f64> = fields
        .get("normalizedPurchasePrice")
        .and_then(|v| v.parse().ok());
    let fuel_tank_size: Option<f64> = fields.get("fuelTankSize").and_then(|v| v.parse().ok());
    let manual_odo: Option<i64> = fields.get("manualOdo").and_then(|v| v.parse().ok());
    let vin = fields.get("vin").cloned();
    let engine_number = fields.get("engineNumber").cloned();
    let vehicle_nr = fields.get("vehicleNr").cloned();
    let number_plate = fields.get("numberPlate").cloned();
    let first_registration = fields.get("firstRegistration").cloned();
    let purchase_date = fields.get("purchaseDate").cloned();
    let currency_code = fields.get("currencyCode").cloned();

    let id = sqlx::query(
        "INSERT INTO motorcycles
           (make, model, modelYear, userId, vin, engineNumber, vehicleNr, numberPlate,
            image, isVeteran, isArchived, firstRegistration, initialOdo, manualOdo,
            purchaseDate, purchasePrice, normalizedPurchasePrice, currencyCode, fuelTankSize)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&make)
    .bind(&model)
    .bind(&model_year)
    .bind(user.id)
    .bind(&vin)
    .bind(&engine_number)
    .bind(&vehicle_nr)
    .bind(&number_plate)
    .bind(&image_filename)
    .bind(is_veteran)
    .bind(is_archived)
    .bind(&first_registration)
    .bind(initial_odo)
    .bind(&manual_odo)
    .bind(&purchase_date)
    .bind(purchase_price)
    .bind(normalized_purchase_price)
    .bind(&currency_code)
    .bind(fuel_tank_size)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    let row = sqlx::query(&format!("{} WHERE id = ?", MOTORCYCLE_SELECT))
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "motorcycle": row_to_motorcycle(&row) })),
    ))
}

pub async fn get_motorcycle(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let row = sqlx::query(&format!(
        "{} WHERE id = ? AND userId = ?",
        MOTORCYCLE_SELECT
    ))
    .bind(id)
    .bind(user.id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Motorcycle not found".to_string()))?;

    let issues = sqlx::query(
        "SELECT id, motorcycleId, odo, description, priority, status, date FROM issues WHERE motorcycleId = ? ORDER BY date DESC",
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let issues_json: Vec<Value> = issues
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<i64, _>("id"),
                "motorcycleId": r.get::<i64, _>("motorcycleId"),
                "odo": r.get::<i64, _>("odo"),
                "description": r.get::<Option<String>, _>("description"),
                "priority": r.get::<String, _>("priority"),
                "status": r.get::<String, _>("status"),
                "date": r.get::<Option<String>, _>("date"),
            })
        })
        .collect();

    let maintenance = sqlx::query(
        r#"SELECT id, date, odo, motorcycleId, cost, normalizedCost, currency, description, type,
           brand, model, tirePosition, tireSize, dotCode, batteryType, fluidType, viscosity,
           oilType, inspectionLocation, locationId, fuelType, fuelAmount, pricePerUnit,
           latitude, longitude, locationName, fuelConsumption, tripDistance
           FROM maintenanceRecords WHERE motorcycleId = ? ORDER BY date DESC, id DESC"#,
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let maintenance_json: Vec<Value> = maintenance.iter().map(maintenance_row_to_value).collect();

    let previous_owners = sqlx::query(
        r#"SELECT id, motorcycleId, name, surname, purchaseDate, address, city, postcode,
           country, phoneNumber, email, comments, createdAt, updatedAt
           FROM previousOwners WHERE motorcycleId = ? ORDER BY purchaseDate DESC"#,
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let owners_json: Vec<Value> = previous_owners
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<i64, _>("id"),
                "motorcycleId": r.get::<i64, _>("motorcycleId"),
                "name": r.get::<String, _>("name"),
                "surname": r.get::<String, _>("surname"),
                "purchaseDate": r.get::<String, _>("purchaseDate"),
                "address": r.get::<Option<String>, _>("address"),
                "city": r.get::<Option<String>, _>("city"),
                "postcode": r.get::<Option<String>, _>("postcode"),
                "country": r.get::<Option<String>, _>("country"),
                "phoneNumber": r.get::<Option<String>, _>("phoneNumber"),
                "email": r.get::<Option<String>, _>("email"),
                "comments": r.get::<Option<String>, _>("comments"),
                "createdAt": r.get::<String, _>("createdAt"),
                "updatedAt": r.get::<String, _>("updatedAt"),
            })
        })
        .collect();

    Ok(Json(json!({
        "motorcycle": row_to_motorcycle(&row),
        "issues": issues_json,
        "maintenanceRecords": maintenance_json,
        "previousOwners": owners_json,
    })))
}

pub async fn update_motorcycle(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    // Verify ownership
    let existing = sqlx::query(&format!(
        "{} WHERE id = ? AND userId = ?",
        MOTORCYCLE_SELECT
    ))
    .bind(id)
    .bind(user.id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Motorcycle not found".to_string()))?;

    let mut fields = std::collections::HashMap::<String, String>::new();
    let mut image_filename: Option<String> = existing.get("image");

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "image" {
            let ct = field.content_type().unwrap_or("image/jpeg").to_string();
            let data = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("File read error: {}", e)))?;
            if !data.is_empty() {
                image_filename = Some(save_image(&config, data.to_vec(), &ct).await?);
            }
        } else {
            let value = field
                .text()
                .await
                .map_err(|e| AppError::BadRequest(format!("Field read error: {}", e)))?;
            fields.insert(name, value);
        }
    }

    let make: String = fields
        .get("make")
        .cloned()
        .unwrap_or_else(|| existing.get("make"));
    let model: String = fields
        .get("model")
        .cloned()
        .unwrap_or_else(|| existing.get("model"));
    let model_year: Option<String> = fields
        .get("fabricationDate")
        .cloned()
        .or_else(|| existing.get("modelYear"));
    let is_veteran: bool = fields
        .get("isVeteran")
        .map(|v| v == "true")
        .unwrap_or_else(|| existing.get("isVeteran"));
    let is_archived: bool = fields
        .get("isArchived")
        .map(|v| v == "true")
        .unwrap_or_else(|| existing.get("isArchived"));
    let initial_odo: i64 = fields
        .get("initialOdo")
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| existing.get("initialOdo"));
    let purchase_price: Option<f64> = fields
        .get("purchasePrice")
        .and_then(|v| v.parse().ok())
        .or_else(|| existing.get("purchasePrice"));
    let normalized_purchase_price: Option<f64> = fields
        .get("normalizedPurchasePrice")
        .and_then(|v| v.parse().ok())
        .or_else(|| existing.get("normalizedPurchasePrice"));
    let fuel_tank_size: Option<f64> = fields
        .get("fuelTankSize")
        .and_then(|v| v.parse().ok())
        .or_else(|| existing.get("fuelTankSize"));
    let manual_odo: Option<i64> = fields
        .get("manualOdo")
        .and_then(|v| v.parse().ok())
        .or_else(|| existing.get("manualOdo"));
    let vin: Option<String> = fields.get("vin").cloned().or_else(|| existing.get("vin"));
    let engine_number: Option<String> = fields
        .get("engineNumber")
        .cloned()
        .or_else(|| existing.get("engineNumber"));
    let vehicle_nr: Option<String> = fields
        .get("vehicleNr")
        .cloned()
        .or_else(|| existing.get("vehicleNr"));
    let number_plate: Option<String> = fields
        .get("numberPlate")
        .cloned()
        .or_else(|| existing.get("numberPlate"));
    let first_registration: Option<String> = fields
        .get("firstRegistration")
        .cloned()
        .or_else(|| existing.get("firstRegistration"));
    let purchase_date: Option<String> = fields
        .get("purchaseDate")
        .cloned()
        .or_else(|| existing.get("purchaseDate"));
    let currency_code: Option<String> = fields
        .get("currencyCode")
        .cloned()
        .or_else(|| existing.get("currencyCode"));

    sqlx::query(
        "UPDATE motorcycles SET
           make = ?, model = ?, modelYear = ?, vin = ?, engineNumber = ?,
           vehicleNr = ?, numberPlate = ?, image = ?, isVeteran = ?, isArchived = ?,
           firstRegistration = ?, initialOdo = ?, manualOdo = ?, purchaseDate = ?,
           purchasePrice = ?, normalizedPurchasePrice = ?, currencyCode = ?, fuelTankSize = ?
           WHERE id = ? AND userId = ?",
    )
    .bind(&make)
    .bind(&model)
    .bind(&model_year)
    .bind(&vin)
    .bind(&engine_number)
    .bind(&vehicle_nr)
    .bind(&number_plate)
    .bind(&image_filename)
    .bind(is_veteran)
    .bind(is_archived)
    .bind(&first_registration)
    .bind(initial_odo)
    .bind(&manual_odo)
    .bind(&purchase_date)
    .bind(purchase_price)
    .bind(normalized_purchase_price)
    .bind(&currency_code)
    .bind(fuel_tank_size)
    .bind(id)
    .bind(user.id)
    .execute(&pool)
    .await?;

    let row = sqlx::query(&format!("{} WHERE id = ?", MOTORCYCLE_SELECT))
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok(Json(json!({ "motorcycle": row_to_motorcycle(&row) })))
}

pub async fn delete_motorcycle(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let result = sqlx::query("DELETE FROM motorcycles WHERE id = ? AND userId = ?")
        .bind(id)
        .bind(user.id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Motorcycle not found".to_string()));
    }

    Ok(Json(json!({ "message": "Motorcycle deleted" })))
}

pub fn maintenance_row_to_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": r.get::<i64, _>("id"),
        "date": r.get::<String, _>("date"),
        "odo": r.get::<i64, _>("odo"),
        "motorcycleId": r.get::<i64, _>("motorcycleId"),
        "cost": r.get::<Option<f64>, _>("cost"),
        "normalizedCost": r.get::<Option<f64>, _>("normalizedCost"),
        "currency": r.get::<Option<String>, _>("currency"),
        "description": r.get::<Option<String>, _>("description"),
        "type": r.get::<String, _>("type"),
        "brand": r.get::<Option<String>, _>("brand"),
        "model": r.get::<Option<String>, _>("model"),
        "tirePosition": r.get::<Option<String>, _>("tirePosition"),
        "tireSize": r.get::<Option<String>, _>("tireSize"),
        "dotCode": r.get::<Option<String>, _>("dotCode"),
        "batteryType": r.get::<Option<String>, _>("batteryType"),
        "fluidType": r.get::<Option<String>, _>("fluidType"),
        "viscosity": r.get::<Option<String>, _>("viscosity"),
        "oilType": r.get::<Option<String>, _>("oilType"),
        "inspectionLocation": r.get::<Option<String>, _>("inspectionLocation"),
        "locationId": r.get::<Option<i64>, _>("locationId"),
        "fuelType": r.get::<Option<String>, _>("fuelType"),
        "fuelAmount": r.get::<Option<f64>, _>("fuelAmount"),
        "pricePerUnit": r.get::<Option<f64>, _>("pricePerUnit"),
        "latitude": r.get::<Option<f64>, _>("latitude"),
        "longitude": r.get::<Option<f64>, _>("longitude"),
        "locationName": r.get::<Option<String>, _>("locationName"),
        "fuelConsumption": r.get::<Option<f64>, _>("fuelConsumption"),
        "tripDistance": r.get::<Option<f64>, _>("tripDistance"),
    })
}

/// Helper: verify motorcycle belongs to user
pub async fn verify_motorcycle_ownership(
    pool: &SqlitePool,
    motorcycle_id: i64,
    user_id: i64,
) -> AppResult<()> {
    let count: i64 =
        sqlx::query("SELECT COUNT(*) as cnt FROM motorcycles WHERE id = ? AND userId = ?")
            .bind(motorcycle_id)
            .bind(user_id)
            .fetch_one(pool)
            .await?
            .get("cnt");
    if count == 0 {
        return Err(AppError::NotFound("Motorcycle not found".to_string()));
    }
    Ok(())
}

/// Helper used by Utc::now() in other modules
pub fn now_str() -> String {
    Utc::now().to_rfc3339()
}
