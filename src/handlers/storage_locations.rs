use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::{locations::verify_location_ownership, maintenance::sync_now},
    models::StorageLocation,
};

/// Upper bound when walking a parent chain; deeper nesting than this is
/// certainly a data error, and the cap keeps the cycle check O(1)-ish.
const MAX_HIERARCHY_DEPTH: usize = 50;

async fn verify_parent(pool: &SqlitePool, parent_id: i64, user_id: i64) -> AppResult<()> {
    let count: i64 = sqlx::query(
        "SELECT COUNT(*) as cnt FROM storageLocations \
         WHERE id = ? AND userId = ? AND deletedAt IS NULL",
    )
    .bind(parent_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?
    .get("cnt");
    if count == 0 {
        return Err(AppError::BadRequest(
            "Parent storage location not found".to_string(),
        ));
    }
    Ok(())
}

/// Reject re-parenting that would make `location_id` its own ancestor.
async fn verify_no_cycle(pool: &SqlitePool, location_id: i64, new_parent_id: i64) -> AppResult<()> {
    let mut current = Some(new_parent_id);
    for _ in 0..MAX_HIERARCHY_DEPTH {
        let Some(cid) = current else { return Ok(()) };
        if cid == location_id {
            return Err(AppError::BadRequest(
                "Storage location cannot be its own ancestor".to_string(),
            ));
        }
        current = sqlx::query("SELECT parentId FROM storageLocations WHERE id = ?")
            .bind(cid)
            .fetch_optional(pool)
            .await?
            .and_then(|row| row.get::<Option<i64>, _>("parentId"));
    }
    Err(AppError::BadRequest(
        "Storage location hierarchy too deep".to_string(),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageLocationFilter {
    /// Incremental-sync cursor; see maintenance `?since`.
    pub since: Option<String>,
}

pub async fn list_storage_locations(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Query(filter): Query<StorageLocationFilter>,
) -> AppResult<Json<Value>> {
    let locations = if let Some(since) = filter.since {
        sqlx::query_as::<_, StorageLocation>(
            "SELECT * FROM storageLocations WHERE userId = ? AND updatedAt > ? ORDER BY name ASC",
        )
        .bind(user.id)
        .bind(since)
        .fetch_all(&pool)
        .await?
    } else {
        sqlx::query_as::<_, StorageLocation>(
            "SELECT * FROM storageLocations WHERE userId = ? AND deletedAt IS NULL \
             ORDER BY name ASC",
        )
        .bind(user.id)
        .fetch_all(&pool)
        .await?
    };

    Ok(Json(json!({ "storageLocations": locations })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStorageLocationRequest {
    pub name: String,
    pub parent_id: Option<i64>,
    /// Physical place (locations entity); root-level entries only.
    pub location_id: Option<i64>,
    /// Client-generated idempotency key (UUID).
    pub client_id: Option<String>,
}

pub async fn create_storage_location(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreateStorageLocationRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }

    // Idempotency on clientId (see maintenance create).
    if let Some(client_id) = &body.client_id {
        if let Some(existing) = sqlx::query_as::<_, StorageLocation>(
            "SELECT * FROM storageLocations WHERE clientId = ? AND userId = ?",
        )
        .bind(client_id)
        .bind(user.id)
        .fetch_optional(&pool)
        .await?
        {
            return Ok((
                StatusCode::CREATED,
                Json(json!({ "storageLocation": existing })),
            ));
        }
    }

    if let Some(parent_id) = body.parent_id {
        verify_parent(&pool, parent_id, user.id).await?;
    }
    // The physical place lives on the tree root only; children inherit it.
    if body.location_id.is_some() && body.parent_id.is_some() {
        return Err(AppError::BadRequest(
            "Only root-level storage locations can be linked to a place".to_string(),
        ));
    }
    if let Some(location_id) = body.location_id {
        verify_location_ownership(&pool, location_id, user.id).await?;
    }

    let now = sync_now();
    let id = sqlx::query(
        "INSERT INTO storageLocations (userId, name, parentId, locationId, clientId, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(user.id)
    .bind(&name)
    .bind(body.parent_id)
    .bind(body.location_id)
    .bind(&body.client_id)
    .bind(&now)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    let location =
        sqlx::query_as::<_, StorageLocation>("SELECT * FROM storageLocations WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "storageLocation": location })),
    ))
}

/// Distinguish "field absent" (outer None via `default`) from "explicit null"
/// (Some(None)) from "value" (Some(Some(v))) — plain serde flattens both null
/// and absent to None otherwise.
fn double_option<'de, D>(deserializer: D) -> Result<Option<Option<i64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    Ok(Some(Option::<i64>::deserialize(deserializer)?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStorageLocationRequest {
    pub name: Option<String>,
    /// Absent = keep, null = move to root level, value = re-parent.
    #[serde(default, deserialize_with = "double_option")]
    pub parent_id: Option<Option<i64>>,
    /// Absent = keep, null = clear, value = set (see `double_option`).
    #[serde(default, deserialize_with = "double_option")]
    pub location_id: Option<Option<i64>>,
}

pub async fn update_storage_location(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<UpdateStorageLocationRequest>,
) -> AppResult<Json<Value>> {
    let existing = sqlx::query_as::<_, StorageLocation>(
        "SELECT * FROM storageLocations WHERE id = ? AND userId = ? AND deletedAt IS NULL",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Storage location not found".to_string()))?;

    let name = body.name.unwrap_or(existing.name);
    let parent_id = match body.parent_id {
        None => existing.parent_id,
        Some(None) => None, // explicit null: move to root level
        Some(Some(parent_id)) => {
            verify_parent(&pool, parent_id, user.id).await?;
            verify_no_cycle(&pool, id, parent_id).await?;
            Some(parent_id)
        }
    };

    // Place link: absent = keep, null = clear, value = validate and set —
    // but never on a nested location (the root of the tree owns the place).
    let location_id = match body.location_id {
        None => existing.location_id,
        Some(None) => None,
        Some(Some(lid)) => {
            if parent_id.is_some() {
                return Err(AppError::BadRequest(
                    "Only root-level storage locations can be linked to a place".to_string(),
                ));
            }
            verify_location_ownership(&pool, lid, user.id).await?;
            Some(lid)
        }
    };
    // Nesting a previously-linked root clears its place link.
    let location_id = if parent_id.is_some() {
        None
    } else {
        location_id
    };

    let now = sync_now();
    sqlx::query(
        "UPDATE storageLocations SET name = ?, parentId = ?, locationId = ?, updatedAt = ? \
         WHERE id = ?",
    )
    .bind(&name)
    .bind(parent_id)
    .bind(location_id)
    .bind(&now)
    .bind(id)
    .execute(&pool)
    .await?;

    let location =
        sqlx::query_as::<_, StorageLocation>("SELECT * FROM storageLocations WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await?;

    Ok(Json(json!({ "storageLocation": location })))
}

pub async fn delete_storage_location(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let existing = sqlx::query_as::<_, StorageLocation>(
        "SELECT * FROM storageLocations WHERE id = ? AND userId = ? AND deletedAt IS NULL",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Storage location not found".to_string()))?;

    let now = sync_now();
    let mut tx = pool.begin().await?;

    // Soft-delete the node itself (tombstone for sync).
    sqlx::query(
        "UPDATE storageLocations SET deletedAt = ?, updatedAt = ? \
         WHERE id = ? AND deletedAt IS NULL",
    )
    .bind(&now)
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    // Reparent live children to the deleted node's parent so the subtree
    // survives; bump updatedAt so offline clients pull the move.
    sqlx::query(
        "UPDATE storageLocations SET parentId = ?, updatedAt = ? \
         WHERE parentId = ? AND deletedAt IS NULL",
    )
    .bind(existing.parent_id)
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    // Detach live stock entries that were stored here; bump updatedAt so the
    // change syncs.
    sqlx::query(
        "UPDATE partStocks SET storageLocationId = NULL, updatedAt = ? \
         WHERE storageLocationId = ? AND deletedAt IS NULL",
    )
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(json!({ "message": "Storage location deleted" })))
}
