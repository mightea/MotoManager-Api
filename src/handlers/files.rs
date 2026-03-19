use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, Response, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;

use crate::config::Config;

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
                let w = query.width.unwrap_or(0);
                let h = query.height.unwrap_or(0);

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
                let cache_filename = format!("{}_{}x{}.{}", stem, w, h, if matches!(format, image::ImageFormat::WebP) { "webp" } else if matches!(format, image::ImageFormat::Png) { "png" } else { "jpg" });
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

                match resize_image(&data, w, h, format) {
                    Ok(resized) => {
                        tracing::info!("Resized image {} to {}x{} (format: {:?})", filename, w, h, format);
                        
                        // Save to cache (fire and forget)
                        let cache_path_clone = cache_path.clone();
                        let resized_clone = resized.clone();
                        tokio::spawn(async move {
                            if let Err(e) = tokio::fs::write(cache_path_clone, resized_clone).await {
                                tracing::warn!("Failed to write resized image to cache: {}", e);
                            }
                        });

                        let response = Response::builder()
                            .status(StatusCode::OK)
                            .header(header::CONTENT_TYPE, content_type)
                            .header(
                                header::CACHE_CONTROL,
                                "public, max-age=31536000",
                            )
                            .body(Body::from(resized))
                            .unwrap();
                        response.into_response()
                    }
                    Err(_) => serve_raw(data, &filename).into_response(),
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
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let filename = sanitize_filename(&filename);
    let path = config.documents_dir().join(&filename);

    match tokio::fs::read(&path).await {
        Ok(data) => {
            let content_type = mime_guess::from_path(&filename)
                .first_or_octet_stream()
                .to_string();

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
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
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let filename = sanitize_filename(&filename);
    let path = config.previews_dir().join(&filename);

    match tokio::fs::read(&path).await {
        Ok(data) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/jpeg")
            .header(header::CACHE_CONTROL, "public, max-age=31536000")
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

fn resize_image(data: &[u8], width: u32, height: u32, format: image::ImageFormat) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(data)
        .map_err(|e| format!("Failed to load image: {}", e))?;

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
