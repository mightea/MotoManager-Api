use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::motorcycles::verify_motorcycle_ownership,
};

fn row_to_issue(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": r.get::<i64, _>("id"),
        "motorcycleId": r.get::<i64, _>("motorcycle_id"),
        "odo": r.get::<i64, _>("odo"),
        "description": r.get::<Option<String>, _>("description"),
        "priority": r.get::<String, _>("priority"),
        "status": r.get::<String, _>("status"),
        "date": r.get::<Option<String>, _>("date"),
    })
}

pub async fn list_issues(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let rows = sqlx::query(
        "SELECT id, motorcycle_id, odo, description, priority, status, date \
         FROM issues WHERE motorcycle_id = ? ORDER BY date DESC, id DESC",
    )
    .bind(motorcycle_id)
    .fetch_all(&pool)
    .await?;

    let issues: Vec<Value> = rows.iter().map(row_to_issue).collect();
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
        "INSERT INTO issues (motorcycle_id, odo, description, priority, status, date) \
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

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "issue": {
                "id": id,
                "motorcycleId": motorcycle_id,
                "odo": body.odo,
                "description": body.description,
                "priority": priority,
                "status": status,
                "date": date,
            }
        })),
    ))
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

    let row = sqlx::query(
        "SELECT id, motorcycle_id, odo, description, priority, status, date \
         FROM issues WHERE id = ? AND motorcycle_id = ?",
    )
    .bind(issue_id)
    .bind(motorcycle_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Issue not found".to_string()))?;

    let odo = body.odo.unwrap_or_else(|| row.get("odo"));
    let description: Option<String> = body.description.or_else(|| row.get("description"));
    let priority = body.priority.unwrap_or_else(|| row.get("priority"));
    let status = body.status.unwrap_or_else(|| row.get("status"));
    let date: Option<String> = body.date.or_else(|| row.get("date"));

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

    Ok(Json(json!({
        "issue": {
            "id": issue_id,
            "motorcycleId": motorcycle_id,
            "odo": odo,
            "description": description,
            "priority": priority,
            "status": status,
            "date": date,
        }
    })))
}

pub async fn delete_issue(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((motorcycle_id, issue_id)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let result = sqlx::query("DELETE FROM issues WHERE id = ? AND motorcycle_id = ?")
        .bind(issue_id)
        .bind(motorcycle_id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Issue not found".to_string()));
    }

    Ok(Json(json!({ "message": "Issue deleted" })))
}
