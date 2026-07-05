use std::collections::HashMap;

use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::{
    auth::AuthUser,
    config::Config,
    error::{AppError, AppResult},
    handlers::{
        maintenance::sync_now,
        model_series::verify_series_accessible,
        motorcycles::{format_image_url, save_image},
    },
    models::{Part, PartConsumption, PartStock, PartWithMeta, PublicPart},
};

/// Helper: verify a live part belongs to the user. Foreign and tombstoned
/// parts are masked as NotFound, mirroring `verify_motorcycle_ownership`.
pub async fn verify_part_ownership(
    pool: &SqlitePool,
    part_id: i64,
    user_id: i64,
) -> AppResult<()> {
    let count: i64 = sqlx::query(
        "SELECT COUNT(*) as cnt FROM parts WHERE id = ? AND userId = ? AND deletedAt IS NULL",
    )
    .bind(part_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?
    .get("cnt");
    if count == 0 {
        return Err(AppError::NotFound("Part not found".to_string()));
    }
    Ok(())
}

/// On-hand is always derived, never stored: live stock minus live consumption.
const ON_HAND_SQL: &str = "SELECT \
     COALESCE((SELECT SUM(quantity) FROM partStocks WHERE partId = ? AND deletedAt IS NULL), 0) \
   - COALESCE((SELECT SUM(quantity) FROM partConsumptions WHERE partId = ? AND deletedAt IS NULL), 0)";

async fn on_hand<'e, E>(executor: E, part_id: i64) -> AppResult<i64>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let value: i64 = sqlx::query_scalar(ON_HAND_SQL)
        .bind(part_id)
        .bind(part_id)
        .fetch_one(executor)
        .await?;
    Ok(value)
}

async fn fetch_series_ids(pool: &SqlitePool, part_id: i64) -> AppResult<Vec<i64>> {
    let rows = sqlx::query(
        "SELECT seriesId FROM partSeriesCompat WHERE partId = ? ORDER BY seriesId ASC",
    )
    .bind(part_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(|r| r.get::<i64, _>("seriesId")).collect())
}

/// Assemble the owner-facing representation for a single part.
async fn part_with_meta(pool: &SqlitePool, mut part: Part) -> AppResult<PartWithMeta> {
    let series_ids = fetch_series_ids(pool, part.id).await?;
    let on_hand = on_hand(pool, part.id).await?;
    let stock_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM partStocks WHERE partId = ? AND deletedAt IS NULL",
    )
    .bind(part.id)
    .fetch_one(pool)
    .await?;

    part.image = format_image_url(part.image);
    Ok(PartWithMeta {
        part,
        series_ids,
        on_hand,
        stock_count,
    })
}

async fn validate_series_ids(
    pool: &SqlitePool,
    series_ids: &[i64],
    user_id: i64,
) -> AppResult<()> {
    for sid in series_ids {
        verify_series_accessible(pool, *sid, user_id).await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Parts
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartsFilter {
    /// Incremental-sync cursor; see maintenance `?since`.
    pub since: Option<String>,
    /// Restrict to parts compatible with this motorcycle's series.
    pub motorcycle_id: Option<i64>,
}

pub async fn list_parts(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Query(filter): Query<PartsFilter>,
) -> AppResult<Json<Value>> {
    // Derived compatibility: filter through the motorcycle's catalog node, if
    // set. Matching is hierarchy-aware — a part linked anywhere on the node's
    // ancestor/descendant chain fits. A bike without a node has no derivable
    // fitment -> empty result.
    let mut series_filter: Option<Vec<i64>> = None;
    if let Some(motorcycle_id) = filter.motorcycle_id {
        let series_id: Option<Option<i64>> = sqlx::query_scalar(
            "SELECT seriesId FROM motorcycles WHERE id = ? AND userId = ?",
        )
        .bind(motorcycle_id)
        .bind(user.id)
        .fetch_optional(&pool)
        .await?;
        let series_id =
            series_id.ok_or_else(|| AppError::NotFound("Motorcycle not found".to_string()))?;
        match series_id {
            Some(sid) => {
                series_filter =
                    Some(crate::handlers::model_series::compatible_series_ids(&pool, sid, user.id).await?)
            }
            None => return Ok(Json(json!({ "parts": [] }))),
        }
    }

    let mut query_str = "SELECT * FROM parts WHERE userId = ?".to_string();
    if filter.since.is_some() {
        query_str.push_str(" AND updatedAt > ?");
    } else {
        query_str.push_str(" AND deletedAt IS NULL");
    }
    if let Some(series_ids) = &series_filter {
        let placeholders = vec!["?"; series_ids.len().max(1)].join(", ");
        query_str.push_str(&format!(
            " AND id IN (SELECT partId FROM partSeriesCompat WHERE seriesId IN ({}))",
            placeholders
        ));
    }
    query_str.push_str(" ORDER BY name ASC, id ASC");

    let mut query = sqlx::query_as::<_, Part>(sqlx::AssertSqlSafe(query_str)).bind(user.id);
    if let Some(since) = filter.since {
        query = query.bind(since);
    }
    if let Some(series_ids) = &series_filter {
        if series_ids.is_empty() {
            query = query.bind(-1); // impossible id keeps the IN clause valid
        }
        for sid in series_ids {
            query = query.bind(sid);
        }
    }
    let parts = query.fetch_all(&pool).await?;

    // Batch the meta (fitment + inventory) instead of per-part queries.
    let series_rows = sqlx::query(
        "SELECT psc.partId, psc.seriesId FROM partSeriesCompat psc \
         JOIN parts p ON p.id = psc.partId WHERE p.userId = ? ORDER BY psc.seriesId ASC",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;
    let mut series_by_part: HashMap<i64, Vec<i64>> = HashMap::new();
    for row in series_rows {
        series_by_part
            .entry(row.get("partId"))
            .or_default()
            .push(row.get("seriesId"));
    }

    let stock_rows = sqlx::query(
        "SELECT partId, COUNT(*) as cnt, SUM(quantity) as total FROM partStocks \
         WHERE deletedAt IS NULL \
         AND partId IN (SELECT id FROM parts WHERE userId = ?) GROUP BY partId",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;
    let mut stock_by_part: HashMap<i64, (i64, i64)> = HashMap::new();
    for row in stock_rows {
        stock_by_part.insert(
            row.get("partId"),
            (row.get::<i64, _>("total"), row.get::<i64, _>("cnt")),
        );
    }

    let consumption_rows = sqlx::query(
        "SELECT partId, SUM(quantity) as total FROM partConsumptions \
         WHERE deletedAt IS NULL \
         AND partId IN (SELECT id FROM parts WHERE userId = ?) GROUP BY partId",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;
    let mut consumed_by_part: HashMap<i64, i64> = HashMap::new();
    for row in consumption_rows {
        consumed_by_part.insert(row.get("partId"), row.get::<i64, _>("total"));
    }

    let result: Vec<PartWithMeta> = parts
        .into_iter()
        .map(|mut part| {
            let (stocked, stock_count) = stock_by_part.get(&part.id).copied().unwrap_or((0, 0));
            let consumed = consumed_by_part.get(&part.id).copied().unwrap_or(0);
            let series_ids = series_by_part.get(&part.id).cloned().unwrap_or_default();
            part.image = format_image_url(part.image);
            PartWithMeta {
                part,
                series_ids,
                on_hand: stocked - consumed,
                stock_count,
            }
        })
        .collect();

    Ok(Json(json!({ "parts": result })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePartRequest {
    pub part_number: String,
    pub name: String,
    pub manufacturer: Option<String>,
    pub description: Option<String>,
    pub is_public: Option<bool>,
    pub series_ids: Option<Vec<i64>>,
    /// Client-generated idempotency key (UUID).
    pub client_id: Option<String>,
}

pub async fn create_part(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreatePartRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    let part_number = body.part_number.trim().to_string();
    let name = body.name.trim().to_string();
    if part_number.is_empty() {
        return Err(AppError::BadRequest("partNumber is required".to_string()));
    }
    if name.is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }
    let manufacturer = body
        .manufacturer
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "BMW".to_string());
    let series_ids = body.series_ids.unwrap_or_default();
    validate_series_ids(&pool, &series_ids, user.id).await?;

    // Idempotency on clientId (see maintenance create).
    if let Some(client_id) = &body.client_id {
        if let Some(existing) =
            sqlx::query_as::<_, Part>("SELECT * FROM parts WHERE clientId = ? AND userId = ?")
                .bind(client_id)
                .bind(user.id)
                .fetch_optional(&pool)
                .await?
        {
            let meta = part_with_meta(&pool, existing).await?;
            return Ok((StatusCode::CREATED, Json(json!({ "part": meta }))));
        }
    }

    // Identity = partNumber + name per user (live rows only, tombstones may be
    // recreated). Pre-check for a friendlier error than the raw index violation.
    let duplicate: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM parts \
         WHERE userId = ? AND partNumber = ? AND name = ? AND deletedAt IS NULL",
    )
    .bind(user.id)
    .bind(&part_number)
    .bind(&name)
    .fetch_one(&pool)
    .await?;
    if duplicate > 0 {
        return Err(AppError::BadRequest(
            "A part with this part number and name already exists".to_string(),
        ));
    }

    let now = sync_now();
    let mut tx = pool.begin().await?;

    let id = sqlx::query(
        "INSERT INTO parts \
         (userId, partNumber, name, manufacturer, description, isPublic, clientId, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(user.id)
    .bind(&part_number)
    .bind(&name)
    .bind(&manufacturer)
    .bind(&body.description)
    .bind(body.is_public.unwrap_or(false))
    .bind(&body.client_id)
    .bind(&now)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    for sid in &series_ids {
        sqlx::query("INSERT OR IGNORE INTO partSeriesCompat (partId, seriesId) VALUES (?, ?)")
            .bind(id)
            .bind(sid)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    let part = sqlx::query_as::<_, Part>("SELECT * FROM parts WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;
    let meta = part_with_meta(&pool, part).await?;

    Ok((StatusCode::CREATED, Json(json!({ "part": meta }))))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePartRequest {
    pub part_number: Option<String>,
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub description: Option<String>,
    pub is_public: Option<bool>,
    /// Full replacement of the fitment set when present.
    pub series_ids: Option<Vec<i64>>,
}

pub async fn update_part(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<UpdatePartRequest>,
) -> AppResult<Json<Value>> {
    let existing = sqlx::query_as::<_, Part>(
        "SELECT * FROM parts WHERE id = ? AND userId = ? AND deletedAt IS NULL",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Part not found".to_string()))?;

    let part_number = body.part_number.unwrap_or(existing.part_number);
    let name = body.name.unwrap_or(existing.name);
    let manufacturer = body.manufacturer.unwrap_or(existing.manufacturer);
    let description = body.description.or(existing.description);
    let is_public = body.is_public.unwrap_or(existing.is_public);

    if let Some(series_ids) = &body.series_ids {
        validate_series_ids(&pool, series_ids, user.id).await?;
    }

    let duplicate: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM parts \
         WHERE userId = ? AND partNumber = ? AND name = ? AND deletedAt IS NULL AND id != ?",
    )
    .bind(user.id)
    .bind(&part_number)
    .bind(&name)
    .bind(id)
    .fetch_one(&pool)
    .await?;
    if duplicate > 0 {
        return Err(AppError::BadRequest(
            "A part with this part number and name already exists".to_string(),
        ));
    }

    let now = sync_now();
    let mut tx = pool.begin().await?;

    sqlx::query(
        "UPDATE parts SET partNumber = ?, name = ?, manufacturer = ?, description = ?, \
         isPublic = ?, updatedAt = ? WHERE id = ?",
    )
    .bind(&part_number)
    .bind(&name)
    .bind(&manufacturer)
    .bind(&description)
    .bind(is_public)
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    // Fitment is replaced wholesale; the parts.updatedAt bump above is what
    // carries the change to syncing clients (the junction has no sync columns).
    if let Some(series_ids) = &body.series_ids {
        sqlx::query("DELETE FROM partSeriesCompat WHERE partId = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for sid in series_ids {
            sqlx::query("INSERT OR IGNORE INTO partSeriesCompat (partId, seriesId) VALUES (?, ?)")
                .bind(id)
                .bind(sid)
                .execute(&mut *tx)
                .await?;
        }
    }

    tx.commit().await?;

    let part = sqlx::query_as::<_, Part>("SELECT * FROM parts WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;
    let meta = part_with_meta(&pool, part).await?;

    Ok(Json(json!({ "part": meta })))
}

pub async fn delete_part(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    verify_part_ownership(&pool, id, user.id).await?;

    // Soft cascade: tombstone the part and its live stock/consumption rows in
    // one transaction so offline clients remove all three via tombstone pull.
    let now = sync_now();
    let mut tx = pool.begin().await?;

    sqlx::query("UPDATE parts SET deletedAt = ?, updatedAt = ? WHERE id = ? AND deletedAt IS NULL")
        .bind(&now)
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE partStocks SET deletedAt = ?, updatedAt = ? \
         WHERE partId = ? AND deletedAt IS NULL",
    )
    .bind(&now)
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE partConsumptions SET deletedAt = ?, updatedAt = ? \
         WHERE partId = ? AND deletedAt IS NULL",
    )
    .bind(&now)
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(json!({ "message": "Part deleted" })))
}

// ---------------------------------------------------------------------------
// Part image
// ---------------------------------------------------------------------------

/// Delete a stored image file (original only; resized cache entries expire on
/// their own). Best-effort — a missing file must not fail the request.
async fn remove_image_file(config: &Config, image: &str) {
    let filename = image
        .replace("/data/images/", "")
        .replace("data/images/", "")
        .replace("/images/", "");
    let _ = tokio::fs::remove_file(config.images_dir().join(filename)).await;
}

pub async fn upload_part_image(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    verify_part_ownership(&pool, id, user.id).await?;

    let mut image_filename: Option<String> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))?
    {
        if field.name().unwrap_or("") == "image" {
            let ct = field.content_type().unwrap_or("image/jpeg").to_string();
            let data = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("File read error: {}", e)))?;
            if !data.is_empty() {
                image_filename = Some(save_image(&config, data.to_vec(), &ct).await?);
            }
        }
    }
    let Some(filename) = image_filename else {
        return Err(AppError::BadRequest("image is required".to_string()));
    };

    let old_image: Option<String> = sqlx::query_scalar("SELECT image FROM parts WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    // Bump updatedAt so offline clients pull the new image path via ?since.
    let now = sync_now();
    sqlx::query("UPDATE parts SET image = ?, updatedAt = ? WHERE id = ?")
        .bind(&filename)
        .bind(&now)
        .bind(id)
        .execute(&pool)
        .await?;

    if let Some(old) = old_image {
        remove_image_file(&config, &old).await;
    }

    let part = sqlx::query_as::<_, Part>("SELECT * FROM parts WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;
    let meta = part_with_meta(&pool, part).await?;

    Ok(Json(json!({ "part": meta })))
}

pub async fn delete_part_image(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    verify_part_ownership(&pool, id, user.id).await?;

    let old_image: Option<String> = sqlx::query_scalar("SELECT image FROM parts WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    let now = sync_now();
    sqlx::query("UPDATE parts SET image = NULL, updatedAt = ? WHERE id = ?")
        .bind(&now)
        .bind(id)
        .execute(&pool)
        .await?;

    if let Some(old) = old_image {
        remove_image_file(&config, &old).await;
    }

    let part = sqlx::query_as::<_, Part>("SELECT * FROM parts WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;
    let meta = part_with_meta(&pool, part).await?;

    Ok(Json(json!({ "part": meta })))
}

// ---------------------------------------------------------------------------
// Public browse
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicPartsFilter {
    pub query: Option<String>,
    pub series_id: Option<i64>,
}

pub async fn list_public_parts(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Query(filter): Query<PublicPartsFilter>,
) -> AppResult<Json<Value>> {
    // Whitelist projection: prices, purchase dates and storage locations are
    // never selected here — other users only see catalog data + availability.
    let mut query_str = "SELECT p.id, p.partNumber, p.name, p.manufacturer, p.description, \
         p.image, u.username as ownerName, \
         COALESCE((SELECT SUM(s.quantity) FROM partStocks s \
                   WHERE s.partId = p.id AND s.deletedAt IS NULL), 0) \
       - COALESCE((SELECT SUM(c.quantity) FROM partConsumptions c \
                   WHERE c.partId = p.id AND c.deletedAt IS NULL), 0) as totalQuantity \
         FROM parts p JOIN users u ON u.id = p.userId \
         WHERE p.isPublic = 1 AND p.deletedAt IS NULL AND p.userId != ?"
        .to_string();

    let search = filter
        .query
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty());
    if search.is_some() {
        query_str.push_str(" AND (p.partNumber LIKE ? OR p.name LIKE ?)");
    }
    if filter.series_id.is_some() {
        query_str
            .push_str(" AND p.id IN (SELECT partId FROM partSeriesCompat WHERE seriesId = ?)");
    }
    query_str.push_str(" ORDER BY p.name ASC, p.id ASC");

    let mut query = sqlx::query(sqlx::AssertSqlSafe(query_str)).bind(user.id);
    if let Some(q) = &search {
        let like = format!("%{}%", q);
        query = query.bind(like.clone()).bind(like);
    }
    if let Some(sid) = filter.series_id {
        query = query.bind(sid);
    }
    let rows = query.fetch_all(&pool).await?;

    // Batch fitment for the returned parts.
    let ids: Vec<i64> = rows.iter().map(|r| r.get::<i64, _>("id")).collect();
    let mut series_by_part: HashMap<i64, Vec<i64>> = HashMap::new();
    if !ids.is_empty() {
        let placeholders = vec!["?"; ids.len()].join(", ");
        let mut series_query = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT partId, seriesId FROM partSeriesCompat WHERE partId IN ({}) \
             ORDER BY seriesId ASC",
            placeholders
        )));
        for id in &ids {
            series_query = series_query.bind(id);
        }
        for row in series_query.fetch_all(&pool).await? {
            series_by_part
                .entry(row.get("partId"))
                .or_default()
                .push(row.get("seriesId"));
        }
    }

    let parts: Vec<PublicPart> = rows
        .into_iter()
        .map(|row| {
            let id: i64 = row.get("id");
            // Negative on-hand can result from stock-deletion corrections;
            // clamp for display so availability never reads below zero.
            let total_quantity = row.get::<i64, _>("totalQuantity").max(0);
            PublicPart {
                id,
                part_number: row.get("partNumber"),
                name: row.get("name"),
                manufacturer: row.get("manufacturer"),
                description: row.get("description"),
                image: format_image_url(row.get("image")),
                series_ids: series_by_part.get(&id).cloned().unwrap_or_default(),
                owner_name: row.get("ownerName"),
                has_stock: total_quantity > 0,
                total_quantity,
            }
        })
        .collect();

    Ok(Json(json!({ "parts": parts })))
}

// ---------------------------------------------------------------------------
// Part stocks
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartStockFilter {
    /// Incremental-sync cursor; see maintenance `?since`.
    pub since: Option<String>,
    pub part_id: Option<i64>,
}

pub async fn list_part_stocks(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Query(filter): Query<PartStockFilter>,
) -> AppResult<Json<Value>> {
    let mut query_str = "SELECT s.* FROM partStocks s \
         JOIN parts p ON p.id = s.partId WHERE p.userId = ?"
        .to_string();
    if filter.since.is_some() {
        query_str.push_str(" AND s.updatedAt > ?");
    } else {
        query_str.push_str(" AND s.deletedAt IS NULL");
    }
    if filter.part_id.is_some() {
        query_str.push_str(" AND s.partId = ?");
    }
    query_str.push_str(" ORDER BY s.purchaseDate DESC, s.id DESC");

    let mut query = sqlx::query_as::<_, PartStock>(sqlx::AssertSqlSafe(query_str)).bind(user.id);
    if let Some(since) = filter.since {
        query = query.bind(since);
    }
    if let Some(part_id) = filter.part_id {
        query = query.bind(part_id);
    }
    let stocks = query.fetch_all(&pool).await?;

    Ok(Json(json!({ "partStocks": stocks })))
}

async fn verify_storage_location(
    pool: &SqlitePool,
    storage_location_id: i64,
    user_id: i64,
) -> AppResult<()> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM storageLocations \
         WHERE id = ? AND userId = ? AND deletedAt IS NULL",
    )
    .bind(storage_location_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    if count == 0 {
        return Err(AppError::BadRequest(
            "Storage location not found".to_string(),
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePartStockRequest {
    pub part_id: i64,
    pub quantity: Option<i64>,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub normalized_price: Option<f64>,
    pub purchase_date: Option<String>,
    pub storage_location_id: Option<i64>,
    pub notes: Option<String>,
    /// Client-generated idempotency key (UUID).
    pub client_id: Option<String>,
}

pub async fn create_part_stock(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreatePartStockRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    verify_part_ownership(&pool, body.part_id, user.id).await?;
    let quantity = body.quantity.unwrap_or(1);
    if quantity < 1 {
        return Err(AppError::BadRequest(
            "quantity must be at least 1".to_string(),
        ));
    }
    if let Some(slid) = body.storage_location_id {
        verify_storage_location(&pool, slid, user.id).await?;
    }

    // Idempotency on clientId (see maintenance create).
    if let Some(client_id) = &body.client_id {
        if let Some(existing) = sqlx::query_as::<_, PartStock>(
            "SELECT s.* FROM partStocks s JOIN parts p ON p.id = s.partId \
             WHERE s.clientId = ? AND p.userId = ?",
        )
        .bind(client_id)
        .bind(user.id)
        .fetch_optional(&pool)
        .await?
        {
            return Ok((StatusCode::CREATED, Json(json!({ "partStock": existing }))));
        }
    }

    let now = sync_now();
    let id = sqlx::query(
        "INSERT INTO partStocks \
         (partId, quantity, price, currency, normalizedPrice, purchaseDate, \
          storageLocationId, notes, clientId, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(body.part_id)
    .bind(quantity)
    .bind(body.price)
    .bind(&body.currency)
    .bind(body.normalized_price)
    .bind(&body.purchase_date)
    .bind(body.storage_location_id)
    .bind(&body.notes)
    .bind(&body.client_id)
    .bind(&now)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    let stock = sqlx::query_as::<_, PartStock>("SELECT * FROM partStocks WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok((StatusCode::CREATED, Json(json!({ "partStock": stock }))))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePartStockRequest {
    pub quantity: Option<i64>,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub normalized_price: Option<f64>,
    pub purchase_date: Option<String>,
    pub storage_location_id: Option<i64>,
    pub notes: Option<String>,
}

pub async fn update_part_stock(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<UpdatePartStockRequest>,
) -> AppResult<Json<Value>> {
    let existing = sqlx::query_as::<_, PartStock>(
        "SELECT s.* FROM partStocks s JOIN parts p ON p.id = s.partId \
         WHERE s.id = ? AND p.userId = ? AND s.deletedAt IS NULL",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Part stock not found".to_string()))?;

    let quantity = body.quantity.unwrap_or(existing.quantity);
    if quantity < 1 {
        return Err(AppError::BadRequest(
            "quantity must be at least 1".to_string(),
        ));
    }
    if let Some(slid) = body.storage_location_id {
        verify_storage_location(&pool, slid, user.id).await?;
    }
    let price = body.price.or(existing.price);
    let currency = body.currency.or(existing.currency);
    let normalized_price = body.normalized_price.or(existing.normalized_price);
    let purchase_date = body.purchase_date.or(existing.purchase_date);
    let storage_location_id = body.storage_location_id.or(existing.storage_location_id);
    let notes = body.notes.or(existing.notes);

    let now = sync_now();
    sqlx::query(
        "UPDATE partStocks SET quantity = ?, price = ?, currency = ?, normalizedPrice = ?, \
         purchaseDate = ?, storageLocationId = ?, notes = ?, updatedAt = ? WHERE id = ?",
    )
    .bind(quantity)
    .bind(price)
    .bind(&currency)
    .bind(normalized_price)
    .bind(&purchase_date)
    .bind(storage_location_id)
    .bind(&notes)
    .bind(&now)
    .bind(id)
    .execute(&pool)
    .await?;

    let stock = sqlx::query_as::<_, PartStock>("SELECT * FROM partStocks WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok(Json(json!({ "partStock": stock })))
}

pub async fn delete_part_stock(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    // A deletion that drives on-hand negative is allowed deliberately — it is
    // a correction of a mis-entered purchase, not a consumption.
    let now = sync_now();
    let result = sqlx::query(
        "UPDATE partStocks SET deletedAt = ?, updatedAt = ? \
         WHERE id = ? AND deletedAt IS NULL \
         AND partId IN (SELECT id FROM parts WHERE userId = ?)",
    )
    .bind(&now)
    .bind(&now)
    .bind(id)
    .bind(user.id)
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Part stock not found".to_string()));
    }

    Ok(Json(json!({ "message": "Part stock deleted" })))
}

// ---------------------------------------------------------------------------
// Part consumptions
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartConsumptionFilter {
    /// Incremental-sync cursor; see maintenance `?since`.
    pub since: Option<String>,
    pub part_id: Option<i64>,
    pub maintenance_record_id: Option<i64>,
}

pub async fn list_part_consumptions(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Query(filter): Query<PartConsumptionFilter>,
) -> AppResult<Json<Value>> {
    let mut query_str = "SELECT c.* FROM partConsumptions c \
         JOIN parts p ON p.id = c.partId WHERE p.userId = ?"
        .to_string();
    if filter.since.is_some() {
        query_str.push_str(" AND c.updatedAt > ?");
    } else {
        query_str.push_str(" AND c.deletedAt IS NULL");
    }
    if filter.part_id.is_some() {
        query_str.push_str(" AND c.partId = ?");
    }
    if filter.maintenance_record_id.is_some() {
        query_str.push_str(" AND c.maintenanceRecordId = ?");
    }
    query_str.push_str(" ORDER BY c.date DESC, c.id DESC");

    let mut query =
        sqlx::query_as::<_, PartConsumption>(sqlx::AssertSqlSafe(query_str)).bind(user.id);
    if let Some(since) = filter.since {
        query = query.bind(since);
    }
    if let Some(part_id) = filter.part_id {
        query = query.bind(part_id);
    }
    if let Some(mid) = filter.maintenance_record_id {
        query = query.bind(mid);
    }
    let consumptions = query.fetch_all(&pool).await?;

    Ok(Json(json!({ "partConsumptions": consumptions })))
}

/// Verify the maintenance record belongs to one of the caller's motorcycles
/// and is live; returns its date for defaulting the consumption date.
async fn verify_maintenance_record(
    pool: &SqlitePool,
    maintenance_record_id: i64,
    user_id: i64,
) -> AppResult<String> {
    let date: Option<String> = sqlx::query_scalar(
        "SELECT mr.date FROM maintenanceRecords mr \
         JOIN motorcycles m ON m.id = mr.motorcycleId \
         WHERE mr.id = ? AND m.userId = ? AND mr.deletedAt IS NULL",
    )
    .bind(maintenance_record_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    date.ok_or_else(|| AppError::NotFound("Maintenance record not found".to_string()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePartConsumptionRequest {
    pub part_id: i64,
    pub quantity: i64,
    pub maintenance_record_id: Option<i64>,
    pub date: Option<String>,
    pub notes: Option<String>,
    /// Client-generated idempotency key (UUID).
    pub client_id: Option<String>,
}

pub async fn create_part_consumption(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreatePartConsumptionRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    verify_part_ownership(&pool, body.part_id, user.id).await?;
    if body.quantity < 1 {
        return Err(AppError::BadRequest(
            "quantity must be at least 1".to_string(),
        ));
    }

    let mut record_date: Option<String> = None;
    if let Some(mid) = body.maintenance_record_id {
        record_date = Some(verify_maintenance_record(&pool, mid, user.id).await?);
    }

    // Idempotency on clientId (see maintenance create).
    if let Some(client_id) = &body.client_id {
        if let Some(existing) = sqlx::query_as::<_, PartConsumption>(
            "SELECT c.* FROM partConsumptions c JOIN parts p ON p.id = c.partId \
             WHERE c.clientId = ? AND p.userId = ?",
        )
        .bind(client_id)
        .bind(user.id)
        .fetch_optional(&pool)
        .await?
        {
            return Ok((
                StatusCode::CREATED,
                Json(json!({ "partConsumption": existing })),
            ));
        }
    }

    let date = body
        .date
        .or(record_date)
        .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());

    let now = sync_now();
    // On-hand check and insert share one transaction; SQLite's single-writer
    // model makes the read-then-insert race-safe.
    let mut tx = pool.begin().await?;

    let available = on_hand(&mut *tx, body.part_id).await?;
    if body.quantity > available {
        return Err(AppError::BadRequest("Not enough stock".to_string()));
    }

    let id = sqlx::query(
        "INSERT INTO partConsumptions \
         (partId, maintenanceRecordId, quantity, date, notes, clientId, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(body.part_id)
    .bind(body.maintenance_record_id)
    .bind(body.quantity)
    .bind(&date)
    .bind(&body.notes)
    .bind(&body.client_id)
    .bind(&now)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    tx.commit().await?;

    let consumption =
        sqlx::query_as::<_, PartConsumption>("SELECT * FROM partConsumptions WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "partConsumption": consumption })),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePartConsumptionRequest {
    pub quantity: Option<i64>,
    pub date: Option<String>,
    pub notes: Option<String>,
}

pub async fn update_part_consumption(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<UpdatePartConsumptionRequest>,
) -> AppResult<Json<Value>> {
    let existing = sqlx::query_as::<_, PartConsumption>(
        "SELECT c.* FROM partConsumptions c JOIN parts p ON p.id = c.partId \
         WHERE c.id = ? AND p.userId = ? AND c.deletedAt IS NULL",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Part consumption not found".to_string()))?;

    let quantity = body.quantity.unwrap_or(existing.quantity);
    if quantity < 1 {
        return Err(AppError::BadRequest(
            "quantity must be at least 1".to_string(),
        ));
    }
    let date = body.date.unwrap_or(existing.date);
    let notes = body.notes.or(existing.notes);

    let now = sync_now();
    let mut tx = pool.begin().await?;

    // Revalidate on-hand with the delta only: the existing quantity is already
    // counted as consumed, so just the increase must be coverable.
    let delta = quantity - existing.quantity;
    if delta > 0 {
        let available = on_hand(&mut *tx, existing.part_id).await?;
        if delta > available {
            return Err(AppError::BadRequest("Not enough stock".to_string()));
        }
    }

    sqlx::query(
        "UPDATE partConsumptions SET quantity = ?, date = ?, notes = ?, updatedAt = ? \
         WHERE id = ?",
    )
    .bind(quantity)
    .bind(&date)
    .bind(&notes)
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let consumption =
        sqlx::query_as::<_, PartConsumption>("SELECT * FROM partConsumptions WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await?;

    Ok(Json(json!({ "partConsumption": consumption })))
}

pub async fn delete_part_consumption(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    // Tombstone; the consumed quantity flows back into on-hand by derivation.
    let now = sync_now();
    let result = sqlx::query(
        "UPDATE partConsumptions SET deletedAt = ?, updatedAt = ? \
         WHERE id = ? AND deletedAt IS NULL \
         AND partId IN (SELECT id FROM parts WHERE userId = ?)",
    )
    .bind(&now)
    .bind(&now)
    .bind(id)
    .bind(user.id)
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Part consumption not found".to_string()));
    }

    Ok(Json(json!({ "message": "Part consumption deleted" })))
}
