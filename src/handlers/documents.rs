use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use pdfium_render::prelude::*;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    config::Config,
    error::{AppError, AppResult},
    models::Document,
};

pub fn format_doc_paths(mut doc: Document) -> Document {
    doc.file_path = format!(
        "/documents/{}",
        doc.file_path
            .replace("/data/documents/", "")
            .replace("data/documents/", "")
    );
    doc.preview_path = doc.preview_path.map(|p| {
        format!(
            "/previews/{}",
            p.replace("/data/previews/", "")
                .replace("data/previews/", "")
        )
    });
    doc
}

pub async fn get_motorcycle_ids_for_doc(pool: &SqlitePool, doc_id: i64) -> AppResult<Vec<i64>> {
    let rows = sqlx::query!(
        "SELECT motorcycleId FROM documentMotorcycles WHERE documentId = ?",
        doc_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.motorcycleId).collect())
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

    tracing::info!("Saving document file: {} as {}", filename, stored_filename);
    tokio::fs::write(&file_path, &data).await?;

    // Generate preview for images or PDFs
    let image_extensions = ["jpg", "jpeg", "png", "webp", "gif"];
    let preview_filename = if image_extensions.contains(&ext.as_str()) {
        tracing::info!("Generating preview for image document: {}", stored_filename);
        match generate_image_preview(config, &data, &uuid).await {
            Ok(pf) => {
                tracing::info!("Preview generated successfully: {}", pf);
                Some(pf)
            }
            Err(e) => {
                tracing::error!("Failed to generate preview for {}: {}", stored_filename, e);
                None
            }
        }
    } else if ext == "pdf" {
        tracing::info!("Generating preview for PDF document: {}", stored_filename);
        match generate_pdf_preview(config, &data, &uuid).await {
            Ok(pf) => {
                tracing::info!("PDF preview generated successfully: {}", pf);
                Some(pf)
            }
            Err(e) => {
                tracing::error!(
                    "Failed to generate PDF preview for {}: {}",
                    stored_filename,
                    e
                );
                None
            }
        }
    } else {
        tracing::debug!("Skipping preview generation for extension: {}", ext);
        None
    };

    Ok((stored_filename, preview_filename))
}

async fn generate_pdf_preview(config: &Config, data: &[u8], uuid: &str) -> AppResult<String> {
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
            .or_else(|_| Pdfium::bind_to_system_library())
            .map_err(|e| AppError::Image(format!("Could not bind to Pdfium library: {}", e)))?,
    );

    let document = pdfium
        .load_pdf_from_byte_slice(data, None)
        .map_err(|e| AppError::Image(format!("Failed to load PDF: {:?}", e)))?;

    let first_page = document
        .pages()
        .get(0)
        .map_err(|e| AppError::Image(format!("Failed to get first page of PDF: {:?}", e)))?;

    let render_config = PdfRenderConfig::new()
        .set_target_width(800)
        .set_maximum_height(1200);

    let bitmap = first_page
        .render_with_config(&render_config)
        .map_err(|e| AppError::Image(format!("Failed to render PDF page: {:?}", e)))?;

    let preview_filename = format!("{}.jpg", uuid);
    let preview_path = config.previews_dir().join(&preview_filename);

    let img = bitmap.as_image();
    let thumbnail = img.thumbnail(400, 400);

    thumbnail
        .save_with_format(&preview_path, image::ImageFormat::Jpeg)
        .map_err(|e| AppError::Image(format!("Failed to save PDF preview: {}", e)))?;

    Ok(preview_filename)
}

async fn generate_image_preview(config: &Config, data: &[u8], uuid: &str) -> AppResult<String> {
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
    tracing::debug!(
        "Listing documents for user: {} (ID: {})",
        user.username,
        user.id
    );
    let rows = sqlx::query_as::<_, Document>(
        "SELECT * FROM documents WHERE isPrivate = 0 OR ownerId = ? ORDER BY createdAt DESC",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    let mut docs = Vec::new();
    for row in rows {
        let doc_id = row.id;
        let motorcycle_ids = get_motorcycle_ids_for_doc(&pool, doc_id).await?;
        let doc = format_doc_paths(row);
        let mut doc_val = serde_json::to_value(doc).unwrap_or(json!({}));
        if let Some(obj) = doc_val.as_object_mut() {
            obj.insert("motorcycleIds".to_string(), json!(motorcycle_ids));
        }
        docs.push(doc_val);
    }

    let motorcycles = sqlx::query!(
        "SELECT id, make, model FROM motorcycles WHERE userId = ?",
        user.id
    )
    .fetch_all(&pool)
    .await?;

    let all_motorcycles: Vec<Value> = motorcycles
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "make": r.make,
                "model": r.model,
            })
        })
        .collect();

    let assignments_rows = sqlx::query!("SELECT documentId, motorcycleId FROM documentMotorcycles")
        .fetch_all(&pool)
        .await?;

    let assignments: Vec<Value> = assignments_rows
        .into_iter()
        .map(|r| {
            json!({
                "documentId": r.documentId,
                "motorcycleId": r.motorcycleId,
            })
        })
        .collect();

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
    tracing::info!(
        "Creating document for user: {} (ID: {})",
        user.username,
        user.id
    );
    let mut title: Option<String> = None;
    let mut is_private = false;
    let mut motorcycle_ids: Vec<i64> = Vec::new();
    let mut file_data: Option<(Vec<u8>, String)> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "title" => {
                title =
                    Some(field.text().await.map_err(|e| {
                        AppError::BadRequest(format!("Failed to read title: {}", e))
                    })?);
            }
            "isPrivate" => {
                let val = field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("Failed to read isPrivate: {}", e))
                })?;
                is_private = val == "true" || val == "1";
            }
            "motorcycleIds" | "motorcycleIds[]" => {
                let val = field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("Failed to read motorcycleIds: {}", e))
                })?;
                if let Ok(id) = val.parse::<i64>() {
                    motorcycle_ids.push(id);
                }
            }
            "file" => {
                let original_name = field.file_name().unwrap_or("document.bin").to_string();
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

    let doc_id = sqlx::query(
        "INSERT INTO documents (title, filePath, previewPath, uploadedBy, ownerId, isPrivate, createdAt, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&title)
    .bind(&stored_filename)
    .bind(&preview_filename)
    .bind(&user.name)
    .bind(user.id)
    .bind(is_private)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    // Associate with motorcycles (verifying ownership)
    for moto_id in &motorcycle_ids {
        let count: i64 = sqlx::query!(
            "SELECT COUNT(*) as cnt FROM motorcycles WHERE id = ? AND userId = ?",
            moto_id,
            user.id
        )
        .fetch_one(&pool)
        .await?
        .cnt as i64;
        if count > 0 {
            sqlx::query!(
                "INSERT OR IGNORE INTO documentMotorcycles (documentId, motorcycleId) VALUES (?, ?)",
                doc_id, moto_id
            )
            .execute(&pool)
            .await?;
        }
    }

    let doc = sqlx::query_as::<_, Document>("SELECT * FROM documents WHERE id = ?")
        .bind(doc_id)
        .fetch_one(&pool)
        .await?;

    let saved_moto_ids = get_motorcycle_ids_for_doc(&pool, doc_id).await?;
    let doc = format_doc_paths(doc);
    let mut doc_val = serde_json::to_value(doc).unwrap_or(json!({}));
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
    let existing = sqlx::query_as::<_, Document>("SELECT * FROM documents WHERE id = ?")
        .bind(doc_id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Document not found".to_string()))?;

    let is_owner = existing.owner_id == Some(user.id);

    // If document is private and user is not owner, deny
    if existing.is_private && !is_owner {
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
                    new_title = Some(field.text().await.map_err(|e| {
                        AppError::BadRequest(format!("Failed to read title: {}", e))
                    })?);
                } else {
                    let _ = field.bytes().await;
                }
            }
            "isPrivate" => {
                if is_owner {
                    let val = field.text().await.map_err(|e| {
                        AppError::BadRequest(format!("Failed to read isPrivate: {}", e))
                    })?;
                    new_is_private = Some(val == "true" || val == "1");
                } else {
                    let _ = field.bytes().await;
                }
            }
            "motorcycleIds" | "motorcycleIds[]" => {
                let val = field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("Failed to read motorcycleIds: {}", e))
                })?;
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
        let title = new_title.unwrap_or(existing.title);
        let is_private = new_is_private.unwrap_or(existing.is_private);

        let (new_file_path, new_preview_path) = if let Some((file_bytes, original_name)) = file_data
        {
            let old_file = config.documents_dir().join(&existing.file_path);
            let _ = tokio::fs::remove_file(old_file).await;
            if let Some(old_preview) = &existing.preview_path {
                let old_preview_file = config.previews_dir().join(old_preview);
                let _ = tokio::fs::remove_file(old_preview_file).await;
            }

            save_document_file(&config, file_bytes, &original_name).await?
        } else {
            (existing.file_path, existing.preview_path)
        };

        sqlx::query!(
            "UPDATE documents SET title = ?, filePath = ?, previewPath = ?, isPrivate = ?, updatedAt = ? \
             WHERE id = ?",
            title, new_file_path, new_preview_path, is_private, now, doc_id
        )
        .execute(&pool)
        .await?;
    }

    if let Some(moto_ids) = new_motorcycle_ids {
        if is_owner {
            sqlx::query!(
                "DELETE FROM documentMotorcycles WHERE documentId = ?",
                doc_id
            )
            .execute(&pool)
            .await?;
            for moto_id in &moto_ids {
                let count = sqlx::query!(
                    "SELECT COUNT(*) as cnt FROM motorcycles WHERE id = ? AND userId = ?",
                    moto_id,
                    user.id
                )
                .fetch_one(&pool)
                .await?
                .cnt;
                if count > 0 {
                    sqlx::query!("INSERT OR IGNORE INTO documentMotorcycles (documentId, motorcycleId) VALUES (?, ?)", doc_id, moto_id)
                        .execute(&pool).await?;
                }
            }
        } else {
            let user_motos = sqlx::query!("SELECT id FROM motorcycles WHERE userId = ?", user.id)
                .fetch_all(&pool)
                .await?;

            for moto_row in user_motos {
                sqlx::query!(
                    "DELETE FROM documentMotorcycles WHERE documentId = ? AND motorcycleId = ?",
                    doc_id,
                    moto_row.id
                )
                .execute(&pool)
                .await?;
            }

            for moto_id in &moto_ids {
                let count = sqlx::query!(
                    "SELECT COUNT(*) as cnt FROM motorcycles WHERE id = ? AND userId = ?",
                    moto_id,
                    user.id
                )
                .fetch_one(&pool)
                .await?
                .cnt;
                if count > 0 {
                    sqlx::query!("INSERT OR IGNORE INTO documentMotorcycles (documentId, motorcycleId) VALUES (?, ?)", doc_id, moto_id)
                        .execute(&pool).await?;
                }
            }
        }
    }

    let doc = sqlx::query_as::<_, Document>("SELECT * FROM documents WHERE id = ?")
        .bind(doc_id)
        .fetch_one(&pool)
        .await?;

    let saved_moto_ids = get_motorcycle_ids_for_doc(&pool, doc_id).await?;
    let doc = format_doc_paths(doc);
    let mut doc_val = serde_json::to_value(doc).unwrap_or(json!({}));
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
    let doc = sqlx::query_as::<_, Document>("SELECT * FROM documents WHERE id = ?")
        .bind(doc_id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Document not found".to_string()))?;

    if doc.owner_id != Some(user.id) {
        return Err(AppError::Forbidden);
    }

    sqlx::query!("DELETE FROM documents WHERE id = ?", doc_id)
        .execute(&pool)
        .await?;

    let filename = doc
        .file_path
        .replace("/data/documents/", "")
        .replace("data/documents/", "");
    let _ = tokio::fs::remove_file(config.documents_dir().join(&filename)).await;

    if let Some(preview) = doc.preview_path {
        let preview_filename = preview
            .replace("/data/previews/", "")
            .replace("data/previews/", "");
        let _ = tokio::fs::remove_file(config.previews_dir().join(&preview_filename)).await;
    }

    Ok(Json(json!({ "message": "Document deleted" })))
}
