use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tokio_util::io::ReaderStream;

use crate::{
    auth::AdminUser,
    backup::{perform_backup, BackupGuard},
    config::Config,
    error::{AppError, AppResult},
    models::BackupRecord,
};

/// Most recent runs surfaced to the admin monitor. Bounded so the payload stays
/// small; retention keeps far fewer archives than this anyway.
const HISTORY_LIMIT: i64 = 50;

/// GET /api/admin/backups — schedule config, derived status, and run history.
pub async fn list_backups(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AdminUser(_admin): AdminUser,
) -> AppResult<Json<Value>> {
    let backups = sqlx::query_as::<_, BackupRecord>(
        "SELECT * FROM backups ORDER BY startedAt DESC LIMIT ?",
    )
    .bind(HISTORY_LIMIT)
    .fetch_all(&pool)
    .await?;

    let last_success_at: Option<String> = sqlx::query_scalar(
        "SELECT finishedAt FROM backups \
         WHERE status='success' AND finishedAt IS NOT NULL \
         ORDER BY finishedAt DESC LIMIT 1",
    )
    .fetch_optional(&pool)
    .await?
    .flatten();

    // Next run = last success + interval (only meaningful while the scheduler
    // is enabled and at least one backup has succeeded).
    let next_scheduled_at = if config.backup_enabled {
        last_success_at.as_deref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s).ok().map(|ts| {
                (ts + chrono::Duration::hours(config.backup_interval_hours as i64)).to_rfc3339()
            })
        })
    } else {
        None
    };

    let running = backups.iter().any(|b| b.status == "running");

    Ok(Json(json!({
        "config": {
            "enabled": config.backup_enabled,
            "intervalHours": config.backup_interval_hours,
            "keep": config.backup_keep,
        },
        "running": running,
        "lastSuccessAt": last_success_at,
        "nextScheduledAt": next_scheduled_at,
        "backups": backups,
    })))
}

/// Optional body for a manual backup: the webapp reports its own version so the
/// archive/manifest records the frontend that was live. Absent/invalid body is
/// fine — the backend then falls back to the FRONTEND_VERSION env, else null.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBackupRequest {
    pub frontend_version: Option<String>,
}

/// POST /api/admin/backups — run a backup now. 409 if one is already in flight.
pub async fn create_backup(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    State(lock): State<BackupGuard>,
    AdminUser(admin): AdminUser,
    body: Option<Json<CreateBackupRequest>>,
) -> AppResult<(StatusCode, Json<Value>)> {
    // Hold the guard for the whole run so a scheduled run can't race this one.
    let _guard = lock
        .try_lock()
        .map_err(|_| AppError::Conflict("A backup is already running".to_string()))?;

    // Trim + cap the client-supplied version so it can't bloat the row/manifest.
    let frontend_version = body
        .and_then(|Json(b)| b.frontend_version)
        .map(|v| v.trim().chars().take(64).collect::<String>())
        .filter(|v| !v.is_empty());

    tracing::info!("Admin {} (ID: {}) triggered a manual backup", admin.username, admin.id);
    let id = perform_backup(&pool, &config, "manual", frontend_version).await?;

    let record = sqlx::query_as::<_, BackupRecord>("SELECT * FROM backups WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok((StatusCode::CREATED, Json(json!({ "backup": record }))))
}

/// GET /api/admin/backups/{id}/download — stream the archive as an attachment.
pub async fn download_backup(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AdminUser(_admin): AdminUser,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let record = sqlx::query_as::<_, BackupRecord>("SELECT * FROM backups WHERE id = ?")
        .bind(id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Backup not found".to_string()))?;

    let file_name = record
        .file_path
        .filter(|_| record.status == "success")
        .ok_or_else(|| AppError::NotFound("No archive available for this backup".to_string()))?;

    // Defensive: only ever resolve a basename inside the backup dir.
    let path = config.backup_dir().join(sanitize(&file_name));
    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| AppError::NotFound("Backup archive is no longer on disk".to_string()))?;

    let stream = ReaderStream::new(file);
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/gzip")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", sanitize(&file_name)),
        )
        .body(Body::from_stream(stream))
        .unwrap())
}

/// DELETE /api/admin/backups/{id} — remove the archive and its history row.
pub async fn delete_backup(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AdminUser(admin): AdminUser,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let record = sqlx::query_as::<_, BackupRecord>("SELECT * FROM backups WHERE id = ?")
        .bind(id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Backup not found".to_string()))?;

    if record.status == "running" {
        return Err(AppError::Conflict(
            "Cannot delete a backup that is still running".to_string(),
        ));
    }

    if let Some(file_name) = record.file_path.as_deref() {
        let path = config.backup_dir().join(sanitize(file_name));
        let _ = tokio::fs::remove_file(&path).await;
    }

    // Runtime query (not the compile-time `query!` macro) so this doesn't require
    // the `backups` table to exist in the dev DB at build time.
    sqlx::query("DELETE FROM backups WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await?;

    tracing::info!("Admin {} (ID: {}) deleted backup #{id}", admin.username, admin.id);
    Ok(Json(json!({ "message": "Backup deleted" })))
}

/// Strip any directory components — the stored value is always a plain basename,
/// but never let a crafted DB value escape the backup dir.
fn sanitize(file_name: &str) -> String {
    std::path::Path::new(file_name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string()
}
