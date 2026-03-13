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
        "fabricationDate": r.get::<Option<String>, _>("model_year"),
        "userId": r.get::<i64, _>("user_id"),
        "vin": r.get::<Option<String>, _>("vin"),
        "engineNumber": r.get::<Option<String>, _>("engine_number"),
        "vehicleNr": r.get::<Option<String>, _>("vehicle_nr"),
        "numberPlate": r.get::<Option<String>, _>("number_plate"),
        "image": r.get::<Option<String>, _>("image"),
        "isVeteran": r.get::<bool, _>("is_veteran"),
        "isArchived": r.get::<bool, _>("is_archived"),
        "firstRegistration": r.get::<Option<String>, _>("firstRegistration"),
        "initialOdo": r.get::<i64, _>("initialOdo"),
        "manualOdo": r.get::<Option<i64>, _>("manual_odo"),
        "purchaseDate": r.get::<Option<String>, _>("purchase_date"),
        "purchasePrice": r.get::<Option<f64>, _>("purchase_price"),
        "normalizedPurchasePrice": r.get::<Option<f64>, _>("normalized_purchase_price"),
        "currencyCode": r.get::<Option<String>, _>("currency_code"),
        "fuelTankSize": r.get::<Option<f64>, _>("fuel_tank_size"),
    })
}

const MOTORCYCLE_SELECT: &str = r#"
    SELECT id, make, model, model_year, user_id, vin, engine_number, vehicle_nr,
           number_plate, image, is_veteran, is_archived, "firstRegistration", "initialOdo",
           manual_odo, purchase_date, purchase_price, normalized_purchase_price,
           currency_code, fuel_tank_size
    FROM motorcycles
"#;

pub async fn list_motorcycles(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query(&format!("{} WHERE user_id = ? ORDER BY id ASC", MOTORCYCLE_SELECT))
        .bind(user.id)
        .fetch_all(&pool)
        .await?;

    let mut result = Vec::new();
    for r in &rows {
        let moto_id: i64 = r.get("id");

        let open_issues: i64 = sqlx::query(
            "SELECT COUNT(*) as cnt FROM issues WHERE motorcycle_id = ? AND status != 'done'",
        )
        .bind(moto_id)
        .fetch_one(&pool)
        .await?
        .get("cnt");

        let maintenance_count: i64 =
            sqlx::query("SELECT COUNT(*) as cnt FROM maintenance_records WHERE motorcycle_id = ?")
                .bind(moto_id)
                .fetch_one(&pool)
                .await?
                .get("cnt");

        let latest_odo: Option<i64> =
            sqlx::query("SELECT MAX(odo) as max_odo FROM maintenance_records WHERE motorcycle_id = ?")
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
        r#"INSERT INTO motorcycles
           (make, model, model_year, user_id, vin, engine_number, vehicle_nr, number_plate,
            image, is_veteran, is_archived, "firstRegistration", "initialOdo", manual_odo,
            purchase_date, purchase_price, normalized_purchase_price, currency_code, fuel_tank_size)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
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
        "{} WHERE id = ? AND user_id = ?",
        MOTORCYCLE_SELECT
    ))
    .bind(id)
    .bind(user.id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Motorcycle not found".to_string()))?;

    let issues = sqlx::query(
        "SELECT id, motorcycle_id, odo, description, priority, status, date FROM issues WHERE motorcycle_id = ? ORDER BY date DESC",
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let issues_json: Vec<Value> = issues
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<i64, _>("id"),
                "motorcycleId": r.get::<i64, _>("motorcycle_id"),
                "odo": r.get::<i64, _>("odo"),
                "description": r.get::<Option<String>, _>("description"),
                "priority": r.get::<String, _>("priority"),
                "status": r.get::<String, _>("status"),
                "date": r.get::<Option<String>, _>("date"),
            })
        })
        .collect();

    let maintenance = sqlx::query(
        r#"SELECT id, date, odo, motorcycle_id, cost, normalized_cost, currency, description, type,
           brand, model, tire_position, tire_size, dot_code, battery_type, fluid_type, viscosity,
           oil_type, inspection_location, location_id, fuel_type, fuel_amount, price_per_unit,
           latitude, longitude, location_name, fuel_consumption, trip_distance
           FROM maintenance_records WHERE motorcycle_id = ? ORDER BY date DESC, id DESC"#,
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let maintenance_json: Vec<Value> = maintenance.iter().map(maintenance_row_to_value).collect();

    let previous_owners = sqlx::query(
        r#"SELECT id, motorcycle_id, name, surname, purchase_date, address, city, postcode,
           country, phone_number, email, comments, created_at, updated_at
           FROM previous_owners WHERE motorcycle_id = ? ORDER BY purchase_date DESC"#,
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let owners_json: Vec<Value> = previous_owners
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<i64, _>("id"),
                "motorcycleId": r.get::<i64, _>("motorcycle_id"),
                "name": r.get::<String, _>("name"),
                "surname": r.get::<String, _>("surname"),
                "purchaseDate": r.get::<String, _>("purchase_date"),
                "address": r.get::<Option<String>, _>("address"),
                "city": r.get::<Option<String>, _>("city"),
                "postcode": r.get::<Option<String>, _>("postcode"),
                "country": r.get::<Option<String>, _>("country"),
                "phoneNumber": r.get::<Option<String>, _>("phone_number"),
                "email": r.get::<Option<String>, _>("email"),
                "comments": r.get::<Option<String>, _>("comments"),
                "createdAt": r.get::<String, _>("created_at"),
                "updatedAt": r.get::<String, _>("updated_at"),
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
        "{} WHERE id = ? AND user_id = ?",
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
        .or_else(|| existing.get("model_year"));
    let is_veteran: bool = fields
        .get("isVeteran")
        .map(|v| v == "true")
        .unwrap_or_else(|| existing.get("is_veteran"));
    let is_archived: bool = fields
        .get("isArchived")
        .map(|v| v == "true")
        .unwrap_or_else(|| existing.get("is_archived"));
    let initial_odo: i64 = fields
        .get("initialOdo")
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| existing.get("initialOdo"));
    let purchase_price: Option<f64> = fields
        .get("purchasePrice")
        .and_then(|v| v.parse().ok())
        .or_else(|| existing.get("purchase_price"));
    let normalized_purchase_price: Option<f64> = fields
        .get("normalizedPurchasePrice")
        .and_then(|v| v.parse().ok())
        .or_else(|| existing.get("normalized_purchase_price"));
    let fuel_tank_size: Option<f64> = fields
        .get("fuelTankSize")
        .and_then(|v| v.parse().ok())
        .or_else(|| existing.get("fuel_tank_size"));
    let manual_odo: Option<i64> = fields
        .get("manualOdo")
        .and_then(|v| v.parse().ok())
        .or_else(|| existing.get("manual_odo"));
    let vin: Option<String> = fields.get("vin").cloned().or_else(|| existing.get("vin"));
    let engine_number: Option<String> = fields
        .get("engineNumber")
        .cloned()
        .or_else(|| existing.get("engine_number"));
    let vehicle_nr: Option<String> = fields
        .get("vehicleNr")
        .cloned()
        .or_else(|| existing.get("vehicle_nr"));
    let number_plate: Option<String> = fields
        .get("numberPlate")
        .cloned()
        .or_else(|| existing.get("number_plate"));
    let first_registration: Option<String> = fields
        .get("firstRegistration")
        .cloned()
        .or_else(|| existing.get("firstRegistration"));
    let purchase_date: Option<String> = fields
        .get("purchaseDate")
        .cloned()
        .or_else(|| existing.get("purchase_date"));
    let currency_code: Option<String> = fields
        .get("currencyCode")
        .cloned()
        .or_else(|| existing.get("currency_code"));

    sqlx::query(
        r#"UPDATE motorcycles SET
           make = ?, model = ?, model_year = ?, vin = ?, engine_number = ?,
           vehicle_nr = ?, number_plate = ?, image = ?, is_veteran = ?, is_archived = ?,
           "firstRegistration" = ?, "initialOdo" = ?, manual_odo = ?, purchase_date = ?,
           purchase_price = ?, normalized_purchase_price = ?, currency_code = ?, fuel_tank_size = ?
           WHERE id = ? AND user_id = ?"#,
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
    let result = sqlx::query("DELETE FROM motorcycles WHERE id = ? AND user_id = ?")
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
        "motorcycleId": r.get::<i64, _>("motorcycle_id"),
        "cost": r.get::<Option<f64>, _>("cost"),
        "normalizedCost": r.get::<Option<f64>, _>("normalized_cost"),
        "currency": r.get::<Option<String>, _>("currency"),
        "description": r.get::<Option<String>, _>("description"),
        "type": r.get::<String, _>("type"),
        "brand": r.get::<Option<String>, _>("brand"),
        "model": r.get::<Option<String>, _>("model"),
        "tirePosition": r.get::<Option<String>, _>("tire_position"),
        "tireSize": r.get::<Option<String>, _>("tire_size"),
        "dotCode": r.get::<Option<String>, _>("dot_code"),
        "batteryType": r.get::<Option<String>, _>("battery_type"),
        "fluidType": r.get::<Option<String>, _>("fluid_type"),
        "viscosity": r.get::<Option<String>, _>("viscosity"),
        "oilType": r.get::<Option<String>, _>("oil_type"),
        "inspectionLocation": r.get::<Option<String>, _>("inspection_location"),
        "locationId": r.get::<Option<i64>, _>("location_id"),
        "fuelType": r.get::<Option<String>, _>("fuel_type"),
        "fuelAmount": r.get::<Option<f64>, _>("fuel_amount"),
        "pricePerUnit": r.get::<Option<f64>, _>("price_per_unit"),
        "latitude": r.get::<Option<f64>, _>("latitude"),
        "longitude": r.get::<Option<f64>, _>("longitude"),
        "locationName": r.get::<Option<String>, _>("location_name"),
        "fuelConsumption": r.get::<Option<f64>, _>("fuel_consumption"),
        "tripDistance": r.get::<Option<f64>, _>("trip_distance"),
    })
}

/// Helper: verify motorcycle belongs to user
pub async fn verify_motorcycle_ownership(
    pool: &SqlitePool,
    motorcycle_id: i64,
    user_id: i64,
) -> AppResult<()> {
    let count: i64 =
        sqlx::query("SELECT COUNT(*) as cnt FROM motorcycles WHERE id = ? AND user_id = ?")
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
