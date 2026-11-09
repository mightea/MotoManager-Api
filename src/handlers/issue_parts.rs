//! Spare parts linked to defects (issues), migration 063.
//!
//! A link says "this part in the inventory fixes that open Mangel". Both ends
//! must belong to the caller; foreign or tombstoned rows are masked as
//! NotFound, mirroring the other ownership helpers. Linking never touches
//! `issues.updatedAt`, so the iOS issue sync is unaffected.

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::parts::verify_part_ownership,
    models::Issue,
};

/// Verify a live issue belongs to one of the user's motorcycles.
async fn verify_issue_ownership(pool: &SqlitePool, issue_id: i64, user_id: i64) -> AppResult<()> {
    let count: i64 = sqlx::query(
        "SELECT COUNT(*) as cnt FROM issues i JOIN motorcycles m ON m.id = i.motorcycleId \
         WHERE i.id = ? AND m.userId = ? AND i.deletedAt IS NULL",
    )
    .bind(issue_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?
    .get("cnt");
    if count == 0 {
        return Err(AppError::NotFound("Issue not found".to_string()));
    }
    Ok(())
}

/// `GET /api/parts/{id}/issues` — the issues this part is linked to, with the
/// bike they belong to. Open issues first, newest first within each group.
pub async fn list_part_issues(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(part_id): Path<i64>,
) -> AppResult<Json<Value>> {
    verify_part_ownership(&pool, part_id, user.id).await?;

    let rows = sqlx::query(
        "SELECT i.id, i.motorcycleId, i.title, i.priority, i.status, i.date, \
                m.make, m.model, ip.createdAt AS linkedAt \
         FROM issueParts ip \
         JOIN issues i ON i.id = ip.issueId \
         JOIN motorcycles m ON m.id = i.motorcycleId \
         WHERE ip.partId = ? AND m.userId = ? AND i.deletedAt IS NULL \
         ORDER BY (i.status = 'done') ASC, i.date DESC, i.id DESC",
    )
    .bind(part_id)
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    let issues: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            let status: String = row.get("status");
            json!({
                "id": row.get::<i64, _>("id"),
                "motorcycleId": row.get::<i64, _>("motorcycleId"),
                "motorcycleMake": row.get::<String, _>("make"),
                "motorcycleModel": row.get::<String, _>("model"),
                "title": row.get::<String, _>("title"),
                "priority": row.get::<String, _>("priority"),
                "date": row.get::<Option<String>, _>("date"),
                "isDone": status == "done",
                "status": status,
                "linkedAt": row.get::<String, _>("linkedAt"),
            })
        })
        .collect();

    Ok(Json(json!({ "issues": issues })))
}

/// `GET /api/issue-parts` — every link of the caller in one go, so a client
/// can mirror the whole join table without one request per part. Only links
/// whose issue and part are both live and owned by the caller are listed.
pub async fn list_issue_parts(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query(
        "SELECT ip.issueId, ip.partId, ip.createdAt \
         FROM issueParts ip \
         JOIN issues i ON i.id = ip.issueId \
         JOIN motorcycles m ON m.id = i.motorcycleId \
         JOIN parts p ON p.id = ip.partId \
         WHERE m.userId = ? AND p.userId = ? \
           AND i.deletedAt IS NULL AND p.deletedAt IS NULL \
         ORDER BY ip.issueId ASC, ip.partId ASC",
    )
    .bind(user.id)
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    let links: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "issueId": row.get::<i64, _>("issueId"),
                "partId": row.get::<i64, _>("partId"),
                "createdAt": row.get::<String, _>("createdAt"),
            })
        })
        .collect();

    Ok(Json(json!({ "links": links })))
}

/// `PUT /api/issues/{issue_id}/parts/{part_id}` — link (idempotent).
pub async fn link_issue_part(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((issue_id, part_id)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    verify_issue_ownership(&pool, issue_id, user.id).await?;
    verify_part_ownership(&pool, part_id, user.id).await?;

    sqlx::query("INSERT OR IGNORE INTO issueParts (issueId, partId) VALUES (?, ?)")
        .bind(issue_id)
        .bind(part_id)
        .execute(&pool)
        .await?;

    Ok(Json(json!({ "issueId": issue_id, "partId": part_id })))
}

/// `DELETE /api/issues/{issue_id}/parts/{part_id}` — unlink.
pub async fn unlink_issue_part(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((issue_id, part_id)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    verify_issue_ownership(&pool, issue_id, user.id).await?;
    verify_part_ownership(&pool, part_id, user.id).await?;

    let result = sqlx::query("DELETE FROM issueParts WHERE issueId = ? AND partId = ?")
        .bind(issue_id)
        .bind(part_id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Link not found".to_string()));
    }

    Ok(Json(json!({ "message": "Link removed" })))
}

/// Serialize a motorcycle's issues with an additive `linkedParts` array
/// (`[{id, name, partNumber, onHand}]`) on each — empty when nothing is
/// linked. Only the user's own live parts are listed.
pub async fn issues_with_linked_parts(
    pool: &SqlitePool,
    motorcycle_id: i64,
    user_id: i64,
    issues: Vec<Issue>,
) -> AppResult<Vec<Value>> {
    let rows = sqlx::query(
        "SELECT ip.issueId, p.id, p.name, p.partNumber, \
           COALESCE((SELECT SUM(quantity) FROM partStocks s WHERE s.partId = p.id AND s.deletedAt IS NULL), 0) \
         - COALESCE((SELECT SUM(quantity) FROM partConsumptions c WHERE c.partId = p.id AND c.deletedAt IS NULL), 0) \
           AS onHand \
         FROM issueParts ip \
         JOIN parts p ON p.id = ip.partId \
         JOIN issues i ON i.id = ip.issueId \
         WHERE i.motorcycleId = ? AND p.userId = ? AND p.deletedAt IS NULL \
         ORDER BY p.name ASC, p.id ASC",
    )
    .bind(motorcycle_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut parts_by_issue: HashMap<i64, Vec<Value>> = HashMap::new();
    for row in rows {
        parts_by_issue
            .entry(row.get("issueId"))
            .or_default()
            .push(json!({
                "id": row.get::<i64, _>("id"),
                "name": row.get::<String, _>("name"),
                "partNumber": row.get::<String, _>("partNumber"),
                "onHand": row.get::<i64, _>("onHand"),
            }));
    }

    issues
        .into_iter()
        .map(|issue| {
            let linked = parts_by_issue.remove(&issue.id).unwrap_or_default();
            let mut value = serde_json::to_value(issue)
                .map_err(|e| AppError::Internal(format!("serialize issue: {e}")))?;
            if let Some(obj) = value.as_object_mut() {
                obj.insert("linkedParts".to_string(), Value::Array(linked));
            }
            Ok(value)
        })
        .collect()
}
