use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::{maintenance::sync_now, motorcycles::verify_motorcycle_ownership},
    models::Issue,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFilter {
    /// Incremental-sync cursor; see maintenance `?since`.
    pub since: Option<String>,
}

pub async fn list_issues(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Query(filter): Query<IssueFilter>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let issues = if let Some(since) = filter.since {
        sqlx::query_as::<_, Issue>(
            "SELECT * FROM issues WHERE motorcycleId = ? AND updatedAt > ? \
             ORDER BY date DESC, id DESC",
        )
        .bind(motorcycle_id)
        .bind(since)
        .fetch_all(&pool)
        .await?
    } else {
        sqlx::query_as::<_, Issue>(
            "SELECT * FROM issues WHERE motorcycleId = ? AND deletedAt IS NULL \
             ORDER BY date DESC, id DESC",
        )
        .bind(motorcycle_id)
        .fetch_all(&pool)
        .await?
    };

    Ok(Json(json!({ "issues": issues })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateIssueRequest {
    pub odo: i64,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub date: Option<String>,
    /// Client-generated idempotency key (UUID).
    pub client_id: Option<String>,
}

pub async fn create_issue(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Json(body): Json<CreateIssueRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    // Idempotency on clientId (see maintenance create).
    if let Some(client_id) = &body.client_id {
        if let Some(existing) = sqlx::query_as::<_, Issue>(
            "SELECT * FROM issues WHERE clientId = ? AND motorcycleId = ?",
        )
        .bind(client_id)
        .bind(motorcycle_id)
        .fetch_optional(&pool)
        .await?
        {
            return Ok((StatusCode::CREATED, Json(json!({ "issue": existing }))));
        }
    }

    let title = body.title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::BadRequest("title must not be empty".to_string()));
    }
    let description = body
        .description
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let date = body
        .date
        .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
    let priority = body.priority.unwrap_or_else(|| "medium".to_string());
    let status = body.status.unwrap_or_else(|| "new".to_string());
    let now = sync_now();

    let id = sqlx::query(
        "INSERT INTO issues (motorcycleId, odo, title, description, priority, status, date, clientId, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(motorcycle_id)
    .bind(body.odo)
    .bind(&title)
    .bind(&description)
    .bind(&priority)
    .bind(&status)
    .bind(&date)
    .bind(&body.client_id)
    .bind(&now)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    let issue = sqlx::query_as::<_, Issue>("SELECT * FROM issues WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok((StatusCode::CREATED, Json(json!({ "issue": issue }))))
}

#[derive(Debug, Deserialize)]
pub struct UpdateIssueRequest {
    pub odo: Option<i64>,
    pub title: Option<String>,
    // Use a double Option to distinguish "field absent" from "field
    // explicitly set to null" — the latter clears description.
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub description: Option<Option<String>>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub date: Option<String>,
}

fn deserialize_optional_field<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}

pub async fn update_issue(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((motorcycle_id, issue_id)): Path<(i64, i64)>,
    Json(body): Json<UpdateIssueRequest>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let existing =
        sqlx::query_as::<_, Issue>("SELECT * FROM issues WHERE id = ? AND motorcycleId = ?")
            .bind(issue_id)
            .bind(motorcycle_id)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Issue not found".to_string()))?;

    let odo = body.odo.unwrap_or(existing.odo);
    let title = match body.title {
        Some(t) => {
            let t = t.trim().to_string();
            if t.is_empty() {
                return Err(AppError::BadRequest("title must not be empty".to_string()));
            }
            t
        }
        None => existing.title,
    };
    let description: Option<String> = match body.description {
        Some(new_desc) => new_desc
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        None => existing.description,
    };
    let priority = body.priority.unwrap_or(existing.priority);
    let status = body.status.unwrap_or(existing.status);
    let date = body.date.unwrap_or(existing.date);
    let now = sync_now();

    sqlx::query(
        "UPDATE issues SET odo = ?, title = ?, description = ?, priority = ?, status = ?, date = ?, updatedAt = ? \
         WHERE id = ?",
    )
    .bind(odo)
    .bind(&title)
    .bind(&description)
    .bind(&priority)
    .bind(&status)
    .bind(&date)
    .bind(&now)
    .bind(issue_id)
    .execute(&pool)
    .await?;

    let issue = sqlx::query_as::<_, Issue>("SELECT * FROM issues WHERE id = ?")
        .bind(issue_id)
        .fetch_one(&pool)
        .await?;

    Ok(Json(json!({ "issue": issue })))
}

pub async fn delete_issue(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((motorcycle_id, issue_id)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let now = sync_now();
    let result = sqlx::query(
        "UPDATE issues SET deletedAt = ?, updatedAt = ? \
         WHERE id = ? AND motorcycleId = ? AND deletedAt IS NULL",
    )
    .bind(&now)
    .bind(&now)
    .bind(issue_id)
    .bind(motorcycle_id)
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Issue not found".to_string()));
    }

    Ok(Json(json!({ "message": "Issue deleted" })))
}
