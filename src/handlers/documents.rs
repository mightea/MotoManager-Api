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

/// Which stored path a served file corresponds to.
#[derive(Clone, Copy)]
pub enum DocFileKind {
    Document,
    Preview,
}

/// Authorize access to a stored document/preview file by its bare filename,
/// enforcing the same visibility rule as `list_documents`: a private document is
/// only readable by its owner. Non-owners (and unregistered files) get NotFound
/// so a private document's existence isn't disclosed. This closes the gap where
/// the static file routes served "private" documents to anyone with the URL.
pub async fn authorize_doc_file(
    pool: &SqlitePool,
    user_id: i64,
    kind: DocFileKind,
    filename: &str,
) -> AppResult<()> {
    // Stored paths are usually bare (`{uuid}.ext`) but legacy rows carry a
    // `data/documents/` (or leading-slash) prefix — match every observed form.
    let (dir, sql) = match kind {
        DocFileKind::Document => (
            "documents",
            "SELECT isPrivate, ownerId FROM documents WHERE filePath IN (?, ?, ?)",
        ),
        DocFileKind::Preview => (
            "previews",
            "SELECT isPrivate, ownerId FROM documents WHERE previewPath IN (?, ?, ?)",
        ),
    };
    let row: Option<(bool, Option<i64>)> = sqlx::query_as(sql)
        .bind(filename)
        .bind(format!("data/{dir}/{filename}"))
        .bind(format!("/data/{dir}/{filename}"))
        .fetch_optional(pool)
        .await?;

    match row {
        Some((is_private, owner_id)) if !is_private || owner_id == Some(user_id) => Ok(()),
        // Private-and-not-owned, or no such registered file: mask as NotFound.
        _ => Err(AppError::NotFound("Not found".to_string())),
    }
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

    // Generate preview for images or PDFs. Image decode/Lanczos thumbnailing and
    // Pdfium rendering are CPU-bound; run them on a blocking thread so they don't
    // stall the async worker (and every other in-flight request) during an upload.
    let image_extensions = ["jpg", "jpeg", "png", "webp", "gif"];
    let preview_filename = if image_extensions.contains(&ext.as_str()) {
        tracing::info!("Generating preview for image document: {}", stored_filename);
        generate_preview_blocking(config, data, uuid, PreviewKind::Image).await
    } else if ext == "pdf" {
        tracing::info!("Generating preview for PDF document: {}", stored_filename);
        generate_preview_blocking(config, data, uuid, PreviewKind::Pdf).await
    } else {
        tracing::debug!("Skipping preview generation for extension: {}", ext);
        None
    };

    Ok((stored_filename, preview_filename))
}

enum PreviewKind {
    Image,
    Pdf,
}

/// Run the CPU-bound preview generation on a blocking thread. Returns the preview
/// filename, or `None` on failure (a missing preview is non-fatal for an upload).
async fn generate_preview_blocking(
    config: &Config,
    data: Vec<u8>,
    uuid: String,
    kind: PreviewKind,
) -> Option<String> {
    let config = config.clone();
    let result = tokio::task::spawn_blocking(move || match kind {
        PreviewKind::Image => generate_image_preview(&config, &data, &uuid),
        PreviewKind::Pdf => generate_pdf_preview(&config, &data, &uuid),
    })
    .await;

    match result {
        Ok(Ok(pf)) => {
            tracing::info!("Preview generated successfully: {}", pf);
            Some(pf)
        }
        Ok(Err(e)) => {
            tracing::error!("Failed to generate preview: {}", e);
            None
        }
        Err(e) => {
            tracing::error!("Preview generation task panicked: {}", e);
            None
        }
    }
}

fn generate_pdf_preview(config: &Config, data: &[u8], uuid: &str) -> AppResult<String> {
    // Shared instance — a second Pdfium::bind_to_library in the same process
    // fails, which used to silently break every preview after the first.
    let pdfium = crate::pdfium_lib::shared_pdfium().map_err(AppError::Image)?;

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

    let img = bitmap
        .as_image()
        .map_err(|e| AppError::Image(format!("Failed to convert PDF bitmap to image: {:?}", e)))?;
    let thumbnail = img.thumbnail(400, 400);

    thumbnail
        .save_with_format(&preview_path, image::ImageFormat::Jpeg)
        .map_err(|e| AppError::Image(format!("Failed to save PDF preview: {}", e)))?;

    Ok(preview_filename)
}

fn generate_image_preview(config: &Config, data: &[u8], uuid: &str) -> AppResult<String> {
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

    // One bulk query feeds both the per-doc motorcycleIds and the assignments payload
    let assignments_rows = sqlx::query!("SELECT documentId, motorcycleId FROM documentMotorcycles")
        .fetch_all(&pool)
        .await?;

    let mut ids_by_doc: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    let mut assignments = Vec::new();
    for r in assignments_rows {
        ids_by_doc
            .entry(r.documentId)
            .or_default()
            .push(r.motorcycleId);
        assignments.push(json!({
            "documentId": r.documentId,
            "motorcycleId": r.motorcycleId,
        }));
    }

    let mut docs = Vec::new();
    for row in rows {
        let doc_id = row.id;
        let motorcycle_ids = ids_by_doc.remove(&doc_id).unwrap_or_default();
        let doc = format_doc_paths(row);
        let mut doc_val = serde_json::to_value(doc).unwrap_or(json!({}));
        if let Some(obj) = doc_val.as_object_mut() {
            obj.insert("motorcycleIds".to_string(), json!(motorcycle_ids));
        }
        docs.push(doc_val);
    }

    let motorcycles = sqlx::query!(
        r#"
        SELECT m.id, m.userId, m.make, m.model, u.name as "ownerName!"
        FROM motorcycles m
        JOIN users u ON m.userId = u.id
        WHERE m.status = 'active'
        "#
    )
    .fetch_all(&pool)
    .await?;

    let all_motorcycles: Vec<Value> = motorcycles
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "userId": r.userId,
                "make": r.make,
                "model": r.model,
                "ownerName": r.ownerName,
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
            "title" if is_owner => {
                new_title =
                    Some(field.text().await.map_err(|e| {
                        AppError::BadRequest(format!("Failed to read title: {}", e))
                    })?);
            }
            "isPrivate" if is_owner => {
                let val = field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("Failed to read isPrivate: {}", e))
                })?;
                new_is_private = Some(val == "true" || val == "1");
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
            "file" if is_owner => {
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
