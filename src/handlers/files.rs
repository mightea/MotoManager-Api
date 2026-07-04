use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, Response, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::auth::AuthUser;
use crate::config::Config;
use crate::handlers::documents::{authorize_doc_file, DocFileKind};

/// Upper bound on a requested resize dimension. Without this, a client could ask
/// for e.g. `?width=100000`, forcing a multi-gigabyte allocation/encode (a memory
/// amplification DoS) and flooding the resize cache with arbitrary sizes.
const MAX_IMAGE_DIMENSION: u32 = 2048;

#[derive(Debug, Deserialize)]
pub struct ImageQuery {
    pub width: Option<u32>,
    pub height: Option<u32>,
}

pub async fn serve_image(
    State(config): State<Config>,
    Path(filename): Path<String>,
    Query(query): Query<ImageQuery>,
) -> impl IntoResponse {
    // Security: prevent path traversal
    let filename = sanitize_filename(&filename);
    let path = config.images_dir().join(&filename);

    match tokio::fs::read(&path).await {
        Ok(data) => {
            // If width/height requested, resize
            if query.width.is_some() || query.height.is_some() {
                // Clamp to a sane maximum to prevent memory-amplification via huge
                // dimensions and cache flooding.
                let w = query.width.unwrap_or(0).min(MAX_IMAGE_DIMENSION);
                let h = query.height.unwrap_or(0).min(MAX_IMAGE_DIMENSION);

                let format = if filename.to_lowercase().ends_with(".webp") {
                    image::ImageFormat::WebP
                } else if filename.to_lowercase().ends_with(".png") {
                    image::ImageFormat::Png
                } else {
                    image::ImageFormat::Jpeg
                };

                let content_type = match format {
                    image::ImageFormat::WebP => "image/webp",
                    image::ImageFormat::Png => "image/png",
                    _ => "image/jpeg",
                };

                // Check cache
                let stem = std::path::Path::new(&filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&filename);
                let cache_filename = format!(
                    "{}_{}x{}.{}",
                    stem,
                    w,
                    h,
                    if matches!(format, image::ImageFormat::WebP) {
                        "webp"
                    } else if matches!(format, image::ImageFormat::Png) {
                        "png"
                    } else {
                        "jpg"
                    }
                );
                let cache_path = config.resized_images_dir().join(&cache_filename);

                if let Ok(cached_data) = tokio::fs::read(&cache_path).await {
                    tracing::debug!("Serving cached resized image: {}", cache_filename);
                    return Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, content_type)
                        .header(header::CACHE_CONTROL, "public, max-age=31536000")
                        .body(Body::from(cached_data))
                        .unwrap()
                        .into_response();
                }

                // Decode + Lanczos3 resize is CPU-heavy; keep it off the async workers.
                // Clone the source bytes: the error fallback below still needs them.
                let data_for_resize = data.clone();
                let resize_result = tokio::task::spawn_blocking(move || {
                    resize_image(&data_for_resize, w, h, format)
                })
                .await;

                match resize_result {
                    Ok(Ok(resized)) => {
                        tracing::info!(
                            "Resized image {} to {}x{} (format: {:?})",
                            filename,
                            w,
                            h,
                            format
                        );

                        // Save to cache (fire and forget)
                        let cache_path_clone = cache_path.clone();
                        let resized_clone = resized.clone();
                        tokio::spawn(async move {
                            if let Err(e) = tokio::fs::write(cache_path_clone, resized_clone).await
                            {
                                tracing::warn!("Failed to write resized image to cache: {}", e);
                            }
                        });

                        let response = Response::builder()
                            .status(StatusCode::OK)
                            .header(header::CONTENT_TYPE, content_type)
                            .header(header::CACHE_CONTROL, "public, max-age=31536000")
                            .body(Body::from(resized))
                            .unwrap();
                        response.into_response()
                    }
                    // Resize failure or JoinError (panicked task): fall back to the original
                    _ => serve_raw(data, &filename).into_response(),
                }
            } else {
                serve_raw(data, &filename).into_response()
            }
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn serve_document(
    State(config): State<Config>,
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let filename = sanitize_filename(&filename);

    // Enforce document ownership/privacy — these files are no longer public.
    if let Err(e) = authorize_doc_file(&pool, user.id, DocFileKind::Document, &filename).await {
        return e.into_response();
    }

    let path = config.documents_dir().join(&filename);

    match tokio::fs::read(&path).await {
        Ok(data) => {
            let content_type = mime_guess::from_path(&filename)
                .first_or_octet_stream()
                .to_string();

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                // Content-addressed (UUID) filename → immutable. `private` because
                // the file is auth-gated and may be a private document (never cache
                // it in a shared proxy).
                .header(header::CACHE_CONTROL, "private, max-age=31536000")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("inline; filename=\"{}\"", filename),
                )
                .body(Body::from(data))
                .unwrap()
                .into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn serve_preview(
    State(config): State<Config>,
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let filename = sanitize_filename(&filename);

    // A preview is a thumbnail of its document, so it inherits the document's
    // privacy — authorize before serving.
    if let Err(e) = authorize_doc_file(&pool, user.id, DocFileKind::Preview, &filename).await {
        return e.into_response();
    }

    let path = config.previews_dir().join(&filename);

    match tokio::fs::read(&path).await {
        Ok(data) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/jpeg")
            // `private`: a preview is a thumbnail of an auth-gated (possibly
            // private) document, so it must not be cached by shared proxies.
            .header(header::CACHE_CONTROL, "private, max-age=31536000")
            .body(Body::from(data))
            .unwrap()
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

fn sanitize_filename(filename: &str) -> String {
    // Strip any directory components to prevent path traversal
    std::path::Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string()
}

fn serve_raw(data: Vec<u8>, filename: &str) -> impl IntoResponse {
    let content_type = mime_guess::from_path(filename)
        .first_or_octet_stream()
        .to_string();

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=31536000")
        .body(Body::from(data))
        .unwrap()
}

fn resize_image(
    data: &[u8],
    width: u32,
    height: u32,
    format: image::ImageFormat,
) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(data).map_err(|e| format!("Failed to load image: {}", e))?;

    let resized = if width > 0 && height > 0 {
        img.resize(width, height, image::imageops::FilterType::Lanczos3)
    } else if width > 0 {
        img.resize(width, u32::MAX, image::imageops::FilterType::Lanczos3)
    } else if height > 0 {
        img.resize(u32::MAX, height, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    let mut buf = std::io::Cursor::new(Vec::new());
    resized
        .write_to(&mut buf, format)
        .map_err(|e| format!("Failed to encode image: {}", e))?;

    Ok(buf.into_inner())
}
