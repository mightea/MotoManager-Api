use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::handlers::documents::{format_doc_paths, get_motorcycle_ids_for_doc};
use crate::{
    auth::AuthUser,
    config::Config,
    error::{AppError, AppResult},
    models::{
        Document, Issue, MaintenanceRecord, Motorcycle, MotorcycleWithStats, PreviousOwner,
        TorqueSpec,
    },
};

async fn save_image(config: &Config, data: Vec<u8>, content_type: &str) -> AppResult<String> {
    let ext = if content_type.contains("png") {
        "png"
    } else if content_type.contains("webp") {
        "webp"
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

fn format_image_url(image: Option<String>) -> Option<String> {
    image.map(|i| {
        format!(
            "/images/{}",
            i.replace("/data/images/", "").replace("data/images/", "")
        )
    })
}

pub async fn list_motorcycles(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    tracing::debug!(
        "Listing motorcycles for user: {} (ID: {})",
        user.username,
        user.id
    );

    let motorcycles = sqlx::query_as::<_, MotorcycleWithStats>(r"
        SELECT 
            m.*,
            (SELECT COUNT(*) FROM issues i WHERE i.motorcycleId = m.id AND i.status != 'done') as openIssues,
            (SELECT COUNT(*) FROM maintenanceRecords mr WHERE mr.motorcycleId = m.id) as maintenanceCount,
            (SELECT MAX(odo) FROM maintenanceRecords mr WHERE mr.motorcycleId = m.id) as latestOdo
        FROM motorcycles m
        WHERE m.userId = ?
        ORDER BY m.id ASC
    ")
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    let result: Vec<Value> = motorcycles
        .into_iter()
        .map(|mut m| {
            m.image = format_image_url(m.image);
            serde_json::to_value(m).unwrap_or(json!({}))
        })
        .collect();

    Ok(Json(json!({ "motorcycles": result })))
}

pub async fn create_motorcycle(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(user): AuthUser,
    mut multipart: Multipart,
) -> AppResult<(StatusCode, Json<Value>)> {
    tracing::info!(
        "Creating motorcycle for user: {} (ID: {})",
        user.username,
        user.id
    );
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
    .bind(manual_odo)
    .bind(&purchase_date)
    .bind(purchase_price)
    .bind(normalized_purchase_price)
    .bind(&currency_code)
    .bind(fuel_tank_size)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    let mut motorcycle = sqlx::query_as::<_, Motorcycle>("SELECT * FROM motorcycles WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    motorcycle.image = format_image_url(motorcycle.image);

    tracing::info!("Motorcycle created: {} {} (ID: {})", make, model, id);
    Ok((
        StatusCode::CREATED,
        Json(json!({ "motorcycle": motorcycle })),
    ))
}

pub async fn get_motorcycle(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    tracing::debug!("Fetching motorcycle ID: {} for user: {}", id, user.id);
    let mut motorcycle =
        sqlx::query_as::<_, Motorcycle>("SELECT * FROM motorcycles WHERE id = ? AND userId = ?")
            .bind(id)
            .bind(user.id)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Motorcycle not found".to_string()))?;

    motorcycle.image = format_image_url(motorcycle.image);

    let issues = sqlx::query_as::<_, Issue>(
        "SELECT * FROM issues WHERE motorcycleId = ? ORDER BY date DESC",
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let maintenance = sqlx::query_as::<_, MaintenanceRecord>(
        "SELECT * FROM maintenanceRecords WHERE motorcycleId = ? ORDER BY date DESC, id DESC",
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let previous_owners = sqlx::query_as::<_, PreviousOwner>(
        "SELECT * FROM previousOwners WHERE motorcycleId = ? ORDER BY purchaseDate DESC",
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let torque_specs = sqlx::query_as::<_, TorqueSpec>(
        "SELECT * FROM torqueSpecs WHERE motorcycleId = ? ORDER BY category ASC, name ASC",
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let documents = sqlx::query_as::<_, Document>(
        "SELECT d.* FROM documents d JOIN documentMotorcycles dm ON d.id = dm.documentId WHERE dm.motorcycleId = ? AND (d.isPrivate = 0 OR d.ownerId = ?) ORDER BY d.createdAt DESC",
    )
    .bind(id)
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    #[derive(sqlx::FromRow, serde::Serialize)]
    struct MaintenanceLocation {
        name: String,
        latitude: Option<f64>,
        longitude: Option<f64>,
    }

    let maintenance_locations = sqlx::query_as::<_, MaintenanceLocation>(
        "SELECT DISTINCT name, latitude, longitude FROM ( \
           SELECT locationName as name, latitude, longitude FROM maintenanceRecords \
           WHERE motorcycleId = ? AND locationName IS NOT NULL AND locationName != '' \
           UNION \
           SELECT inspectionLocation as name, NULL as latitude, NULL as longitude FROM maintenanceRecords \
           WHERE motorcycleId = ? AND inspectionLocation IS NOT NULL AND inspectionLocation != '' \
         ) ORDER BY name ASC",
    )
    .bind(id)
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let locations_json = maintenance_locations
        .into_iter()
        .map(|row| {
            json!({
                "name": row.name,
                "latitude": row.latitude,
                "longitude": row.longitude,
            })
        })
        .collect::<Vec<_>>();


    let mut formatted_docs = Vec::new();
    for row in documents {
        let doc_id = row.id;
        let motorcycle_ids = get_motorcycle_ids_for_doc(&pool, doc_id).await?;
        let doc = format_doc_paths(row);
        let mut doc_val = serde_json::to_value(doc).unwrap_or(json!({}));
        if let Some(obj) = doc_val.as_object_mut() {
            obj.insert("motorcycleIds".to_string(), json!(motorcycle_ids));
        }
        formatted_docs.push(doc_val);
    }

    Ok(Json(json!({
        "motorcycle": motorcycle,
        "issues": issues,
        "maintenanceRecords": maintenance,
        "maintenanceLocations": locations_json,
        "previousOwners": previous_owners,
        "torqueSpecs": torque_specs,
        "torqueSpecifications": torque_specs,
        "documents": formatted_docs,
    })))
}

pub async fn update_motorcycle(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    tracing::info!("Updating motorcycle ID: {} for user: {}", id, user.id);
    // Verify ownership
    let existing =
        sqlx::query_as::<_, Motorcycle>("SELECT * FROM motorcycles WHERE id = ? AND userId = ?")
            .bind(id)
            .bind(user.id)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Motorcycle not found".to_string()))?;

    let mut fields = std::collections::HashMap::<String, String>::new();
    let mut image_filename: Option<String> = existing.image.clone();

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

    let make: String = fields.get("make").cloned().unwrap_or(existing.make);
    let model: String = fields.get("model").cloned().unwrap_or(existing.model);
    let model_year: Option<String> = fields
        .get("fabricationDate")
        .cloned()
        .or(existing.model_year);
    let is_veteran: bool = fields
        .get("isVeteran")
        .map(|v| v == "true")
        .unwrap_or(existing.is_veteran);
    let is_archived: bool = fields
        .get("isArchived")
        .map(|v| v == "true")
        .unwrap_or(existing.is_archived);
    let initial_odo: i64 = fields
        .get("initialOdo")
        .and_then(|v| v.parse().ok())
        .unwrap_or(existing.initial_odo);
    let purchase_price: Option<f64> = fields
        .get("purchasePrice")
        .and_then(|v| v.parse().ok())
        .or(existing.purchase_price);
    let normalized_purchase_price: Option<f64> = fields
        .get("normalizedPurchasePrice")
        .and_then(|v| v.parse().ok())
        .or(existing.normalized_purchase_price);
    let fuel_tank_size: Option<f64> = fields
        .get("fuelTankSize")
        .and_then(|v| v.parse().ok())
        .or(existing.fuel_tank_size);
    let manual_odo: Option<i64> = fields
        .get("manualOdo")
        .and_then(|v| v.parse().ok())
        .or(existing.manual_odo);
    let vin: Option<String> = fields.get("vin").cloned().or(existing.vin);
    let engine_number: Option<String> = fields
        .get("engineNumber")
        .cloned()
        .or(existing.engine_number);
    let vehicle_nr: Option<String> = fields.get("vehicleNr").cloned().or(existing.vehicle_nr);
    let number_plate: Option<String> = fields.get("numberPlate").cloned().or(existing.number_plate);
    let first_registration: Option<String> = fields
        .get("firstRegistration")
        .cloned()
        .or(existing.first_registration);
    let purchase_date: Option<String> = fields
        .get("purchaseDate")
        .cloned()
        .or(existing.purchase_date);
    let currency_code: Option<String> = fields
        .get("currencyCode")
        .cloned()
        .or(existing.currency_code);

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
    .bind(manual_odo)
    .bind(&purchase_date)
    .bind(purchase_price)
    .bind(normalized_purchase_price)
    .bind(&currency_code)
    .bind(fuel_tank_size)
    .bind(id)
    .bind(user.id)
    .execute(&pool)
    .await?;

    let mut motorcycle = sqlx::query_as::<_, Motorcycle>("SELECT * FROM motorcycles WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    motorcycle.image = format_image_url(motorcycle.image);

    tracing::info!("Motorcycle updated ID: {}", id);
    Ok(Json(json!({ "motorcycle": motorcycle })))
}

pub async fn delete_motorcycle(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    tracing::info!("Deleting motorcycle ID: {} for user: {}", id, user.id);

    // Get image path before deleting
    let motorcycle =
        sqlx::query_as::<_, Motorcycle>("SELECT * FROM motorcycles WHERE id = ? AND userId = ?")
            .bind(id)
            .bind(user.id)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Motorcycle not found".to_string()))?;

    let result = sqlx::query("DELETE FROM motorcycles WHERE id = ? AND userId = ?")
        .bind(id)
        .bind(user.id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Motorcycle not found".to_string()));
    }

    // Delete image and resized cache
    if let Some(path_str) = motorcycle.image {
        let filename = path_str
            .replace("/data/images/", "")
            .replace("data/images/", "");

        // Delete original
        let full_path = config.images_dir().join(&filename);
        let _ = tokio::fs::remove_file(full_path).await;

        // Delete resized versions (look for anything starting with stem_)
        let stem = std::path::Path::new(&filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&filename);
        if let Ok(mut entries) = tokio::fs::read_dir(config.resized_images_dir()).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Some(entry_name) = entry.file_name().to_str() {
                    if entry_name.starts_with(stem) {
                        let _ = tokio::fs::remove_file(entry.path()).await;
                    }
                }
            }
        }
    }

    tracing::info!("Motorcycle deleted ID: {}", id);
    Ok(Json(json!({ "message": "Motorcycle deleted" })))
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
