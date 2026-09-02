//! Personal API token management (`/api/settings/api-tokens`) and the MCP
//! audit view. Session-authenticated like every other settings route; the
//! tokens themselves are consumed only by the `/mcp` middleware.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    auth::{
        api_token::{
            display_prefix, generate_token, hash_token, TokenScope, MAX_ACTIVE_TOKENS_PER_USER,
            MAX_TOKEN_LIFETIME_DAYS, MAX_TOKEN_NAME_LEN,
        },
        AuthUser, NotImpersonated,
    },
    error::{AppError, AppResult},
    models::{ApiToken, McpAuditEntry},
};

pub async fn list_api_tokens(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let tokens = sqlx::query_as::<_, ApiToken>(
        "SELECT * FROM apiTokens WHERE userId = ? AND revokedAt IS NULL \
         ORDER BY createdAt DESC, id DESC",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(json!({ "apiTokens": tokens })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiTokenRequest {
    pub name: String,
    /// "read" (default) or "write".
    pub scope: Option<String>,
    /// Optional lifetime; absent = does not expire until revoked.
    pub expires_in_days: Option<i64>,
}

/// Mints a token. The secret is part of this response only — it is not
/// stored and cannot be shown again. Guarded by `NotImpersonated`: a support
/// admin acting as the user must not be able to mint a credential that keeps
/// working after the support session ends.
pub async fn create_api_token(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    _guard: NotImpersonated,
    Json(body): Json<CreateApiTokenRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }
    if name.chars().count() > MAX_TOKEN_NAME_LEN {
        return Err(AppError::BadRequest(format!(
            "name must be at most {MAX_TOKEN_NAME_LEN} characters"
        )));
    }
    let scope = match body.scope.as_deref() {
        None => TokenScope::Read,
        Some(s) => TokenScope::parse(s)
            .ok_or_else(|| AppError::BadRequest("scope must be 'read' or 'write'".to_string()))?,
    };
    let now = Utc::now();
    let expires_at = match body.expires_in_days {
        None => None,
        Some(days) if (1..=MAX_TOKEN_LIFETIME_DAYS).contains(&days) => {
            Some((now + Duration::days(days)).to_rfc3339())
        }
        Some(_) => {
            return Err(AppError::BadRequest(format!(
                "expiresInDays must be between 1 and {MAX_TOKEN_LIFETIME_DAYS}"
            )))
        }
    };

    let active: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM apiTokens WHERE userId = ? AND revokedAt IS NULL")
            .bind(user.id)
            .fetch_one(&pool)
            .await?;
    if active >= MAX_ACTIVE_TOKENS_PER_USER {
        return Err(AppError::Conflict(format!(
            "At most {MAX_ACTIVE_TOKENS_PER_USER} active API tokens are allowed; revoke one first"
        )));
    }

    let secret = generate_token();
    let token_hash = hash_token(&secret);
    let token_prefix = display_prefix(&secret);
    let created_at = now.to_rfc3339();

    let id = sqlx::query(
        "INSERT INTO apiTokens (userId, name, tokenHash, tokenPrefix, scope, createdAt, expiresAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(user.id)
    .bind(&name)
    .bind(&token_hash)
    .bind(&token_prefix)
    .bind(scope.as_str())
    .bind(&created_at)
    .bind(&expires_at)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    let token = sqlx::query_as::<_, ApiToken>("SELECT * FROM apiTokens WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    tracing::info!(
        "API token {} ({}) created for user {} with scope {}",
        id,
        token_prefix,
        user.id,
        scope.as_str()
    );

    Ok((
        StatusCode::CREATED,
        Json(json!({ "apiToken": token, "token": secret })),
    ))
}

/// Revokes (soft-deletes) a token. The row stays so the audit log keeps its
/// token name; the secret stops working immediately.
pub async fn revoke_api_token(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    _guard: NotImpersonated,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE apiTokens SET revokedAt = ? WHERE id = ? AND userId = ? AND revokedAt IS NULL",
    )
    .bind(&now)
    .bind(id)
    .bind(user.id)
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("API token not found".to_string()));
    }

    tracing::info!("API token {} revoked by user {}", id, user.id);
    Ok(Json(json!({ "message": "API token revoked" })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditQuery {
    pub limit: Option<i64>,
}

const AUDIT_DEFAULT_LIMIT: i64 = 50;
const AUDIT_MAX_LIMIT: i64 = 200;

/// Most recent MCP tool calls made with any of the user's tokens, including
/// revoked ones (the token name is kept for context).
pub async fn list_mcp_audit(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Query(query): Query<AuditQuery>,
) -> AppResult<Json<Value>> {
    let limit = query
        .limit
        .unwrap_or(AUDIT_DEFAULT_LIMIT)
        .clamp(1, AUDIT_MAX_LIMIT);

    let entries = sqlx::query_as::<_, McpAuditEntry>(
        "SELECT a.id, a.tokenId, t.name AS tokenName, a.tool, a.arguments, a.outcome, \
                a.detail, a.createdAt \
         FROM mcpAuditLog a LEFT JOIN apiTokens t ON t.id = a.tokenId \
         WHERE a.userId = ? ORDER BY a.id DESC LIMIT ?",
    )
    .bind(user.id)
    .bind(limit)
    .fetch_all(&pool)
    .await?;

    Ok(Json(json!({ "entries": entries })))
}
