use axum::{
    extract::{Path, State},
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
    handlers::motorcycles::verify_motorcycle_ownership,
    models::Issue,
};

pub async fn list_issues(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let issues = sqlx::query_as::<_, Issue>(
        "SELECT * FROM issues WHERE motorcycleId = ? ORDER BY date DESC, id DESC",
    )
    .bind(motorcycle_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(json!({ "issues": issues })))
}

#[derive(Debug, Deserialize)]
pub struct CreateIssueRequest {
    pub odo: i64,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub date: Option<String>,
}

pub async fn create_issue(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Json(body): Json<CreateIssueRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let date = body
        .date
        .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
    let priority = body.priority.unwrap_or_else(|| "medium".to_string());
    let status = body.status.unwrap_or_else(|| "new".to_string());

    let id = sqlx::query(
        "INSERT INTO issues (motorcycleId, odo, description, priority, status, date) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(motorcycle_id)
    .bind(body.odo)
    .bind(&body.description)
    .bind(&priority)
    .bind(&status)
    .bind(&date)
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
    pub description: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub date: Option<String>,
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
    let description: Option<String> = body.description.or(existing.description);
    let priority = body.priority.unwrap_or(existing.priority);
    let status = body.status.unwrap_or(existing.status);
    let date = body.date.unwrap_or(existing.date);

    sqlx::query(
        "UPDATE issues SET odo = ?, description = ?, priority = ?, status = ?, date = ? \
         WHERE id = ?",
    )
    .bind(odo)
    .bind(&description)
    .bind(&priority)
    .bind(&status)
    .bind(&date)
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

    let result = sqlx::query("DELETE FROM issues WHERE id = ? AND motorcycleId = ?")
        .bind(issue_id)
        .bind(motorcycle_id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Issue not found".to_string()));
    }

    Ok(Json(json!({ "message": "Issue deleted" })))
}
