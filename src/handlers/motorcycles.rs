use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::handlers::documents::format_doc_paths;
use crate::{
    auth::AuthUser,
    config::Config,
    error::{AppError, AppResult},
    models::{
        Document, Issue, Location, MaintenanceRecord, Motorcycle, MotorcycleWithStats,
        PreviousOwner, TorqueSpec,
    },
};

pub(crate) async fn save_image(
    config: &Config,
    data: Vec<u8>,
    content_type: &str,
) -> AppResult<String> {
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

type FieldMap = std::collections::HashMap<String, String>;

/// Read an optional text field with clear semantics: absent = keep `existing`,
/// empty = clear (NULL), value = replace. Multipart forms can't send null, so
/// the empty string is the explicit "clear" signal.
fn merge_text(fields: &FieldMap, key: &str, existing: Option<String>) -> Option<String> {
    match fields.get(key) {
        Some(value) if value.trim().is_empty() => None,
        Some(value) => Some(value.clone()),
        None => existing,
    }
}

/// Numeric twin of `merge_text`; an unparseable value keeps `existing` so a
/// typo can't silently wipe data.
fn merge_number<T: std::str::FromStr>(
    fields: &FieldMap,
    key: &str,
    existing: Option<T>,
) -> Option<T> {
    match fields.get(key) {
        Some(value) if value.trim().is_empty() => None,
        Some(value) => value.trim().parse().ok().or(existing),
        None => existing,
    }
}

/// Validate a lifecycle status; returns None for absent/invalid input so the
/// caller can fall back to the existing value (or "active").
fn normalize_status(raw: Option<&str>) -> Option<String> {
    match raw {
        Some(s @ ("active" | "sold")) => Some(s.to_string()),
        _ => None,
    }
}

/// A new (non-empty) text value, present or not — for optional create fields.
fn create_text(fields: &FieldMap, key: &str) -> Option<String> {
    fields
        .get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn format_image_url(image: Option<String>) -> Option<String> {
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
            (SELECT COUNT(*) FROM issues i WHERE i.motorcycleId = m.id AND i.status != 'done' AND i.deletedAt IS NULL) as openIssues,
            (SELECT COUNT(*) FROM maintenanceRecords mr WHERE mr.motorcycleId = m.id AND mr.deletedAt IS NULL) as maintenanceCount,
            (SELECT MAX(odo) FROM maintenanceRecords mr WHERE mr.motorcycleId = m.id AND mr.deletedAt IS NULL) as latestOdo
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
    // Lifecycle status ("active"/"sold"); invalid or absent defaults to active.
    let status = normalize_status(fields.get("status").map(String::as_str))
        .unwrap_or_else(|| "active".to_string());
    let has_sidecar = fields
        .get("hasSidecar")
        .map(|v| v == "true")
        .unwrap_or(false);
    let has_unknown_owners = fields
        .get("hasUnknownOwners")
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
    let vin = create_text(&fields, "vin");
    let engine_number = create_text(&fields, "engineNumber");
    // The webapp historically posts the Stammnummer as "vehicleIdNr".
    let vehicle_nr =
        create_text(&fields, "vehicleNr").or_else(|| create_text(&fields, "vehicleIdNr"));
    let number_plate = create_text(&fields, "numberPlate");
    let first_registration = create_text(&fields, "firstRegistration");
    let purchase_date = create_text(&fields, "purchaseDate");
    let currency_code = create_text(&fields, "currencyCode");
    let series_id: Option<i64> = fields.get("seriesId").and_then(|v| v.parse().ok());
    if let Some(sid) = series_id {
        crate::handlers::model_series::verify_series_accessible(&pool, sid, user.id).await?;
    }
    let front_brake_type = create_text(&fields, "frontBrakeType");
    let rear_brake_type = create_text(&fields, "rearBrakeType");
    let sidecar_brake_type = create_text(&fields, "sidecarBrakeType");
    let drive_type = create_text(&fields, "driveType");
    let sold_date = create_text(&fields, "soldDate");
    let sale_price: Option<f64> = fields.get("salePrice").and_then(|v| v.parse().ok());
    let normalized_sale_price: Option<f64> = fields
        .get("normalizedSalePrice")
        .and_then(|v| v.parse().ok());
    let sale_currency_code = create_text(&fields, "saleCurrencyCode");
    let buyer_name = create_text(&fields, "buyerName");

    let id = sqlx::query(
        "INSERT INTO motorcycles
           (make, model, modelYear, userId, vin, engineNumber, vehicleNr, numberPlate,
            image, isVeteran, hasSidecar, hasUnknownOwners, firstRegistration, initialOdo, manualOdo,
            purchaseDate, purchasePrice, normalizedPurchasePrice, currencyCode, fuelTankSize, seriesId,
            frontBrakeType, rearBrakeType, sidecarBrakeType, driveType,
            status, soldDate, salePrice, normalizedSalePrice, saleCurrencyCode, buyerName)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(has_sidecar)
    .bind(has_unknown_owners)
    .bind(&first_registration)
    .bind(initial_odo)
    .bind(manual_odo)
    .bind(&purchase_date)
    .bind(purchase_price)
    .bind(normalized_purchase_price)
    .bind(&currency_code)
    .bind(fuel_tank_size)
    .bind(series_id)
    .bind(&front_brake_type)
    .bind(&rear_brake_type)
    .bind(&sidecar_brake_type)
    .bind(&drive_type)
    .bind(&status)
    .bind(&sold_date)
    .bind(sale_price)
    .bind(normalized_sale_price)
    .bind(&sale_currency_code)
    .bind(&buyer_name)
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
        "SELECT * FROM issues WHERE motorcycleId = ? AND deletedAt IS NULL ORDER BY date DESC",
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let maintenance = sqlx::query_as::<_, MaintenanceRecord>(
        "SELECT * FROM maintenanceRecords WHERE motorcycleId = ? AND deletedAt IS NULL ORDER BY date DESC, id DESC",
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let previous_owners = sqlx::query_as::<_, PreviousOwner>(
        "SELECT * FROM previousOwners WHERE motorcycleId = ? ORDER BY sortOrder ASC, id ASC",
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let torque_specs = sqlx::query_as::<_, TorqueSpec>(
        "SELECT * FROM torqueSpecs WHERE motorcycleId = ? AND deletedAt IS NULL ORDER BY category ASC, name ASC",
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

    let maintenance_locations = sqlx::query_as::<_, Location>(
        "SELECT DISTINCT l.* FROM locations l \
         JOIN maintenanceRecords mr ON mr.locationId = l.id \
         WHERE mr.motorcycleId = ? \
         ORDER BY l.name ASC",
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    // Bulk-load document→motorcycle associations once instead of a query per
    // document (was an N+1 over this motorcycle's documents).
    let assoc_rows = sqlx::query!("SELECT documentId, motorcycleId FROM documentMotorcycles")
        .fetch_all(&pool)
        .await?;
    let mut ids_by_doc: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    for r in assoc_rows {
        ids_by_doc
            .entry(r.documentId)
            .or_default()
            .push(r.motorcycleId);
    }

    let mut formatted_docs = Vec::new();
    for row in documents {
        let motorcycle_ids = ids_by_doc.get(&row.id).cloned().unwrap_or_default();
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
        "maintenanceLocations": maintenance_locations,
        "previousOwners": previous_owners,
        "torqueSpecs": torque_specs,
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

    // Field semantics for every optional column: absent = keep, empty string =
    // clear, value = replace. Multipart can't express null, so this is the only
    // way the edit form can actually unset a field.
    let make: String = fields
        .get("make")
        .cloned()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(existing.make);
    let model: String = fields
        .get("model")
        .cloned()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(existing.model);
    let model_year = merge_text(&fields, "fabricationDate", existing.model_year);
    let is_veteran: bool = fields
        .get("isVeteran")
        .map(|v| v == "true")
        .unwrap_or(existing.is_veteran);
    // Lifecycle status ("active"/"sold"); absent keeps the existing one.
    let status =
        normalize_status(fields.get("status").map(String::as_str)).unwrap_or(existing.status);
    let has_sidecar: bool = fields
        .get("hasSidecar")
        .map(|v| v == "true")
        .unwrap_or(existing.has_sidecar);
    let has_unknown_owners: bool = fields
        .get("hasUnknownOwners")
        .map(|v| v == "true")
        .unwrap_or(existing.has_unknown_owners);
    let initial_odo: i64 = fields
        .get("initialOdo")
        .and_then(|v| v.parse().ok())
        .unwrap_or(existing.initial_odo);
    let purchase_price = merge_number(&fields, "purchasePrice", existing.purchase_price);
    let normalized_purchase_price = merge_number(
        &fields,
        "normalizedPurchasePrice",
        existing.normalized_purchase_price,
    );
    let fuel_tank_size = merge_number(&fields, "fuelTankSize", existing.fuel_tank_size);
    let manual_odo = merge_number(&fields, "manualOdo", existing.manual_odo);
    let vin = merge_text(&fields, "vin", existing.vin);
    let engine_number = merge_text(&fields, "engineNumber", existing.engine_number);
    // The webapp historically posts the Stammnummer as "vehicleIdNr".
    let vehicle_nr = if fields.contains_key("vehicleNr") {
        merge_text(&fields, "vehicleNr", existing.vehicle_nr)
    } else {
        merge_text(&fields, "vehicleIdNr", existing.vehicle_nr)
    };
    let number_plate = merge_text(&fields, "numberPlate", existing.number_plate);
    let first_registration = merge_text(&fields, "firstRegistration", existing.first_registration);
    let purchase_date = merge_text(&fields, "purchaseDate", existing.purchase_date);
    let currency_code = merge_text(&fields, "currencyCode", existing.currency_code);
    let series_id: Option<i64> = match fields.get("seriesId") {
        Some(value) if value.trim().is_empty() => None, // explicit clear
        Some(value) => match value.trim().parse::<i64>() {
            Ok(sid) => {
                crate::handlers::model_series::verify_series_accessible(&pool, sid, user.id)
                    .await?;
                Some(sid)
            }
            Err(_) => existing.series_id,
        },
        None => existing.series_id,
    };
    let front_brake_type = merge_text(&fields, "frontBrakeType", existing.front_brake_type);
    let rear_brake_type = merge_text(&fields, "rearBrakeType", existing.rear_brake_type);
    let sidecar_brake_type = merge_text(&fields, "sidecarBrakeType", existing.sidecar_brake_type);
    let drive_type = merge_text(&fields, "driveType", existing.drive_type);
    let sold_date = merge_text(&fields, "soldDate", existing.sold_date);
    let sale_price = merge_number(&fields, "salePrice", existing.sale_price);
    let normalized_sale_price = merge_number(
        &fields,
        "normalizedSalePrice",
        existing.normalized_sale_price,
    );
    let sale_currency_code = merge_text(&fields, "saleCurrencyCode", existing.sale_currency_code);
    let buyer_name = merge_text(&fields, "buyerName", existing.buyer_name);

    sqlx::query(
        "UPDATE motorcycles SET
           make = ?, model = ?, modelYear = ?, vin = ?, engineNumber = ?,
           vehicleNr = ?, numberPlate = ?, image = ?, isVeteran = ?,
           hasSidecar = ?, hasUnknownOwners = ?, firstRegistration = ?, initialOdo = ?, manualOdo = ?, purchaseDate = ?,
           purchasePrice = ?, normalizedPurchasePrice = ?, currencyCode = ?, fuelTankSize = ?,
           seriesId = ?, frontBrakeType = ?, rearBrakeType = ?, sidecarBrakeType = ?, driveType = ?,
           status = ?, soldDate = ?, salePrice = ?, normalizedSalePrice = ?, saleCurrencyCode = ?, buyerName = ?
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
    .bind(has_sidecar)
    .bind(has_unknown_owners)
    .bind(&first_registration)
    .bind(initial_odo)
    .bind(manual_odo)
    .bind(&purchase_date)
    .bind(purchase_price)
    .bind(normalized_purchase_price)
    .bind(&currency_code)
    .bind(fuel_tank_size)
    .bind(series_id)
    .bind(&front_brake_type)
    .bind(&rear_brake_type)
    .bind(&sidecar_brake_type)
    .bind(&drive_type)
    .bind(&status)
    .bind(&sold_date)
    .bind(sale_price)
    .bind(normalized_sale_price)
    .bind(&sale_currency_code)
    .bind(&buyer_name)
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

    // `maintenanceRecords`, `issues`, and `locationRecords` reference motorcycles
    // with `ON DELETE NO ACTION` (unlike the sibling tables, which cascade), so a
    // bare `DELETE FROM motorcycles` raises `FOREIGN KEY constraint failed` for any
    // bike that has history. Delete those children first, inside one transaction so
    // a mid-way failure can't leave the motorcycle half-deleted.
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM maintenanceRecords WHERE motorcycleId = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM issues WHERE motorcycleId = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM locationRecords WHERE motorcycleId = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    let result = sqlx::query("DELETE FROM motorcycles WHERE id = ? AND userId = ?")
        .bind(id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;

    if result.rows_affected() == 0 {
        // Ownership re-check already passed above; this guards a concurrent delete.
        return Err(AppError::NotFound("Motorcycle not found".to_string()));
    }

    tx.commit().await?;

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
