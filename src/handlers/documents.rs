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

fn row_to_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    let is_private_raw: i64 = r.get("isPrivate");
    json!({
        "id": r.get::<i64, _>("id"),
        "title": r.get::<String, _>("title"),
        "filePath": r.get::<String, _>("filePath"),
        "previewPath": r.get::<Option<String>, _>("previewPath"),
        "uploadedBy": r.get::<Option<String>, _>("uploadedBy"),
        "ownerId": r.get::<Option<i64>, _>("ownerId"),
        "isPrivate": is_private_raw != 0,
        "createdAt": r.get::<String, _>("createdAt"),
        "updatedAt": r.get::<String, _>("updatedAt"),
    })
}

async fn get_motorcycle_ids_for_doc(pool: &SqlitePool, doc_id: i64) -> AppResult<Vec<i64>> {
    let rows = sqlx::query(
        "SELECT motorcycleId FROM documentMotorcycles WHERE documentId = ?",
    )
    .bind(doc_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(|r| r.get::<i64, _>("motorcycleId")).collect())
}

async fn save_document_file(
    config: &Config,
    data: Vec<u8>,
    filename: &str,
) -> AppResult<(String, Option<String>)> {
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_lowercase();

    let uuid = Uuid::new_v4().to_string();
    let stored_filename = format!("{}.{}", uuid, ext);
    let file_path = config.documents_dir().join(&stored_filename);

    tokio::fs::write(&file_path, &data).await?;

    // Generate preview for images (not PDFs)
    let preview_filename = if ext != "pdf" && (ext == "jpg" || ext == "jpeg" || ext == "png" || ext == "webp" || ext == "gif") {
        match generate_image_preview(config, &data, &uuid).await {
            Ok(pf) => Some(pf),
            Err(e) => {
                tracing::warn!("Failed to generate preview: {}", e);
                None
            }
        }
    } else {
        None
    };

    Ok((stored_filename, preview_filename))
}

async fn generate_image_preview(
    config: &Config,
    data: &[u8],
    uuid: &str,
) -> AppResult<String> {
    let data = data.to_vec();
    let img = image::load_from_memory(&data)
        .map_err(|e| AppError::Image(format!("Failed to load image: {}", e)))?;

    let thumbnail = img.thumbnail(400, 400);
    let preview_filename = format!("{}.jpg", uuid);
    let preview_path = config.previews_dir().join(&preview_filename);

    thumbnail
        .save_with_format(&preview_path, image::ImageFormat::Jpeg)
        .map_err(|e| AppError::Image(format!("Failed to save preview: {}", e)))?;

    Ok(preview_filename)
}

pub async fn list_documents(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    tracing::debug!("Listing documents for user: {} (ID: {})", user.username, user.id);
    let rows = sqlx::query(
        "SELECT id, title, filePath, previewPath, uploadedBy, ownerId, isPrivate, createdAt, updatedAt \
         FROM documents WHERE isPrivate = 0 OR ownerId = ? \
         ORDER BY createdAt DESC",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    let mut docs = Vec::new();
    for row in &rows {
        let doc_id: i64 = row.get("id");
        let motorcycle_ids = get_motorcycle_ids_for_doc(&pool, doc_id).await?;
        let mut doc_val = row_to_value(row);
        if let Some(obj) = doc_val.as_object_mut() {
            obj.insert("motorcycleIds".to_string(), json!(motorcycle_ids));
        }
        docs.push(doc_val);
    }

    let motorcycles = sqlx::query(
        "SELECT id, make, model FROM motorcycles WHERE userId = ?"
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    let all_motorcycles: Vec<Value> = motorcycles.iter().map(|r| json!({
        "id": r.get::<i64, _>("id"),
        "make": r.get::<String, _>("make"),
        "model": r.get::<String, _>("model"),
    })).collect();

    let assignments_rows = sqlx::query(
        "SELECT documentId, motorcycleId FROM documentMotorcycles"
    )
    .fetch_all(&pool)
    .await?;

    let assignments: Vec<Value> = assignments_rows.iter().map(|r| json!({
        "documentId": r.get::<i64, _>("documentId"),
        "motorcycleId": r.get::<i64, _>("motorcycleId"),
    })).collect();

    Ok(Json(json!({ 
        "docs": docs,
        "allMotorcycles": all_motorcycles,
        "assignments": assignments
    })))
}

pub async fn create_document(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(user): AuthUser,
    mut multipart: Multipart,
) -> AppResult<(StatusCode, Json<Value>)> {
    tracing::info!("Creating document for user: {} (ID: {})", user.username, user.id);
    let mut title: Option<String> = None;
    let mut is_private = false;
    let mut motorcycle_ids: Vec<i64> = Vec::new();
    let mut file_data: Option<(Vec<u8>, String)> = None; // (bytes, original_name)

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "title" => {
                title = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::BadRequest(format!("Failed to read title: {}", e)))?,
                );
            }
            "isPrivate" => {
                let val = field
                    .text()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Failed to read isPrivate: {}", e)))?;
                is_private = val == "true" || val == "1";
            }
            "motorcycleIds" | "motorcycleIds[]" => {
                let val = field
                    .text()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Failed to read motorcycleIds: {}", e)))?;
                if let Ok(id) = val.parse::<i64>() {
                    motorcycle_ids.push(id);
                }
            }
            "file" => {
                let original_name = field
                    .file_name()
                    .unwrap_or("document.bin")
                    .to_string();
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Failed to read file: {}", e)))?;
                if !bytes.is_empty() {
                    file_data = Some((bytes.to_vec(), original_name));
                }
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }

    let title = title.ok_or_else(|| AppError::BadRequest("title is required".to_string()))?;
    let (file_bytes, original_name) =
        file_data.ok_or_else(|| AppError::BadRequest("file is required".to_string()))?;

    let (stored_filename, preview_filename) =
        save_document_file(&config, file_bytes, &original_name).await?;

    let now = Utc::now().to_rfc3339();
    let is_private_i = if is_private { 1i64 } else { 0i64 };

    let doc_id = sqlx::query(
        "INSERT INTO documents (title, filePath, previewPath, uploadedBy, ownerId, isPrivate, createdAt, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&title)
    .bind(&stored_filename)
    .bind(&preview_filename)
    .bind(&user.name)
    .bind(user.id)
    .bind(is_private_i)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    // Associate with motorcycles (verifying ownership)
    for moto_id in &motorcycle_ids {
        let count: i64 =
            sqlx::query("SELECT COUNT(*) as cnt FROM motorcycles WHERE id = ? AND userId = ?")
                .bind(moto_id)
                .bind(user.id)
                .fetch_one(&pool)
                .await?
                .get("cnt");
        if count > 0 {
            sqlx::query(
                "INSERT OR IGNORE INTO documentMotorcycles (documentId, motorcycleId) VALUES (?, ?)",
            )
            .bind(doc_id)
            .bind(moto_id)
            .execute(&pool)
            .await?;
        }
    }

    let row = sqlx::query(
        "SELECT id, title, filePath, previewPath, uploadedBy, ownerId, isPrivate, createdAt, updatedAt \
         FROM documents WHERE id = ?",
    )
    .bind(doc_id)
    .fetch_one(&pool)
    .await?;

    let saved_moto_ids = get_motorcycle_ids_for_doc(&pool, doc_id).await?;
    let mut doc_val = row_to_value(&row);
    if let Some(obj) = doc_val.as_object_mut() {
        obj.insert("motorcycleIds".to_string(), json!(saved_moto_ids));
    }

    tracing::info!("Document created: {} (ID: {})", title, doc_id);
    Ok((StatusCode::CREATED, Json(json!({ "document": doc_val }))))
}

pub async fn update_document(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(user): AuthUser,
    Path(doc_id): Path<i64>,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    tracing::info!("Updating document ID: {} for user: {}", doc_id, user.id);
    // Check document exists and user has access
    let existing = sqlx::query(
        "SELECT id, title, filePath, previewPath, uploadedBy, ownerId, isPrivate, createdAt, updatedAt \
         FROM documents WHERE id = ?",
    )
    .bind(doc_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Document not found".to_string()))?;

    let owner_id: Option<i64> = existing.get("ownerId");
    let is_owner = owner_id == Some(user.id);

    let is_private_raw: i64 = existing.get("isPrivate");
    let is_private_existing = is_private_raw != 0;

    // If document is private and user is not owner, deny
    if is_private_existing && !is_owner {
        return Err(AppError::Forbidden);
    }

    let mut new_title: Option<String> = None;
    let mut new_is_private: Option<bool> = None;
    let mut new_motorcycle_ids: Option<Vec<i64>> = None;
    let mut file_data: Option<(Vec<u8>, String)> = None;
    let mut motorcycle_ids_buf: Vec<i64> = Vec::new();
    let mut motorcycle_ids_provided = false;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "title" => {
                if is_owner {
                    new_title = Some(
                        field
                            .text()
                            .await
                            .map_err(|e| AppError::BadRequest(format!("Failed to read title: {}", e)))?,
                    );
                } else {
                    let _ = field.bytes().await;
                }
            }
            "isPrivate" => {
                if is_owner {
                    let val = field
                        .text()
                        .await
                        .map_err(|e| AppError::BadRequest(format!("Failed to read isPrivate: {}", e)))?;
                    new_is_private = Some(val == "true" || val == "1");
                } else {
                    let _ = field.bytes().await;
                }
            }
            "motorcycleIds" | "motorcycleIds[]" => {
                let val = field
                    .text()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Failed to read motorcycleIds: {}", e)))?;
                if let Ok(id) = val.parse::<i64>() {
                    motorcycle_ids_buf.push(id);
                }
                motorcycle_ids_provided = true;
            }
            "file" => {
                if is_owner {
                    let original_name = field.file_name().unwrap_or("document.bin").to_string();
                    let bytes = field
                        .bytes()
                        .await
                        .map_err(|e| AppError::BadRequest(format!("Failed to read file: {}", e)))?;
                    if !bytes.is_empty() {
                        file_data = Some((bytes.to_vec(), original_name));
                    }
                } else {
                    let _ = field.bytes().await;
                }
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }

    if motorcycle_ids_provided {
        new_motorcycle_ids = Some(motorcycle_ids_buf);
    }

    let now = Utc::now().to_rfc3339();

    if is_owner {
        let title = new_title.unwrap_or_else(|| existing.get("title"));
        let is_private = new_is_private.unwrap_or(is_private_existing);
        let is_private_i = if is_private { 1i64 } else { 0i64 };

        let (new_file_path, new_preview_path) = if let Some((file_bytes, original_name)) = file_data {
            // Delete old file
            let old_file_path: String = existing.get("filePath");
            let old_file = config.documents_dir().join(&old_file_path);
            let _ = tokio::fs::remove_file(old_file).await;
            if let Some(old_preview) = existing.get::<Option<String>, _>("previewPath") {
                let old_preview_file = config.previews_dir().join(&old_preview);
                let _ = tokio::fs::remove_file(old_preview_file).await;
            }

            let (stored, preview) =
                save_document_file(&config, file_bytes, &original_name).await?;
            (stored, preview)
        } else {
            let file_path: String = existing.get("filePath");
            let preview_path: Option<String> = existing.get("previewPath");
            (file_path, preview_path)
        };

        sqlx::query(
            "UPDATE documents SET title = ?, filePath = ?, previewPath = ?, isPrivate = ?, updatedAt = ? \
             WHERE id = ?",
        )
        .bind(&title)
        .bind(&new_file_path)
        .bind(&new_preview_path)
        .bind(is_private_i)
        .bind(&now)
        .bind(doc_id)
        .execute(&pool)
        .await?;
    }

    // Update motorcycle associations
    if let Some(moto_ids) = new_motorcycle_ids {
        if is_owner {
            // Owner replaces all associations
            sqlx::query("DELETE FROM documentMotorcycles WHERE documentId = ?")
                .bind(doc_id)
                .execute(&pool)
                .await?;
            for moto_id in &moto_ids {
                let count: i64 =
                    sqlx::query("SELECT COUNT(*) as cnt FROM motorcycles WHERE id = ? AND userId = ?")
                        .bind(moto_id)
                        .bind(user.id)
                        .fetch_one(&pool)
                        .await?
                        .get("cnt");
                if count > 0 {
                    sqlx::query(
                        "INSERT OR IGNORE INTO documentMotorcycles (documentId, motorcycleId) VALUES (?, ?)",
                    )
                    .bind(doc_id)
                    .bind(moto_id)
                    .execute(&pool)
                    .await?;
                }
            }
        } else {
            // Non-owner: only manage their own motorcycle associations
            let user_motos = sqlx::query(
                "SELECT id FROM motorcycles WHERE userId = ?",
            )
            .bind(user.id)
            .fetch_all(&pool)
            .await?;

            for moto_row in &user_motos {
                let moto_id: i64 = moto_row.get("id");
                sqlx::query(
                    "DELETE FROM documentMotorcycles WHERE documentId = ? AND motorcycleId = ?",
                )
                .bind(doc_id)
                .bind(moto_id)
                .execute(&pool)
                .await?;
            }

            // Add the requested motorcycles (only ones belonging to user)
            for moto_id in &moto_ids {
                let count: i64 =
                    sqlx::query("SELECT COUNT(*) as cnt FROM motorcycles WHERE id = ? AND userId = ?")
                        .bind(moto_id)
                        .bind(user.id)
                        .fetch_one(&pool)
                        .await?
                        .get("cnt");
                if count > 0 {
                    sqlx::query(
                        "INSERT OR IGNORE INTO documentMotorcycles (documentId, motorcycleId) VALUES (?, ?)",
                    )
                    .bind(doc_id)
                    .bind(moto_id)
                    .execute(&pool)
                    .await?;
                }
            }
        }
    }

    let row = sqlx::query(
        "SELECT id, title, filePath, previewPath, uploadedBy, ownerId, isPrivate, createdAt, updatedAt \
         FROM documents WHERE id = ?",
    )
    .bind(doc_id)
    .fetch_one(&pool)
    .await?;

    let saved_moto_ids = get_motorcycle_ids_for_doc(&pool, doc_id).await?;
    let mut doc_val = row_to_value(&row);
    if let Some(obj) = doc_val.as_object_mut() {
        obj.insert("motorcycleIds".to_string(), json!(saved_moto_ids));
    }

    tracing::info!("Document updated ID: {}", doc_id);
    Ok(Json(json!({ "document": doc_val })))
}

pub async fn delete_document(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(user): AuthUser,
    Path(doc_id): Path<i64>,
) -> AppResult<Json<Value>> {
    tracing::info!("Deleting document ID: {} for user: {}", doc_id, user.id);
    let row = sqlx::query(
        "SELECT id, filePath, previewPath, ownerId FROM documents WHERE id = ?",
    )
    .bind(doc_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Document not found".to_string()))?;

    let owner_id: Option<i64> = row.get("ownerId");
    if owner_id != Some(user.id) {
        tracing::warn!("Delete failed: user {} (ID: {}) is not owner of document {}", user.username, user.id, doc_id);
        return Err(AppError::Forbidden);
    }

    let file_path: String = row.get("filePath");
    let preview_path: Option<String> = row.get("previewPath");

    // Delete from DB (cascades to documentMotorcycles)
    sqlx::query("DELETE FROM documents WHERE id = ?")
        .bind(doc_id)
        .execute(&pool)
        .await?;

    // Delete files
    let full_path = config.documents_dir().join(&file_path);
    let _ = tokio::fs::remove_file(full_path).await;
    if let Some(preview) = preview_path {
        let preview_full = config.previews_dir().join(&preview);
        let _ = tokio::fs::remove_file(preview_full).await;
    }

    tracing::info!("Document deleted ID: {}", doc_id);
    Ok(Json(json!({ "message": "Document deleted" })))
}
