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
    models::{
        Part, PartConsumption, PartConsumptionWithContext, PartStock, PartWithMeta, PublicPart,
    },
};

/// Helper: verify a live part belongs to the user. Foreign and tombstoned
/// parts are masked as NotFound, mirroring `verify_motorcycle_ownership`.
pub async fn verify_part_ownership(pool: &SqlitePool, part_id: i64, user_id: i64) -> AppResult<()> {
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

/// Weighted-average normalized (CHF) unit price of a part across all its live
/// stock entries: total value / total quantity.
///
/// `partStocks.price` is the total paid for that batch ("Preis gesamt"), so the
/// per-piece figure only exists as this quotient. Entries without a price
/// contribute quantity but no value, which deliberately drags the average down
/// — a part half of which was salvaged for free really did cost less per piece.
/// Returns `None` when there is no live stock at all (nothing to average).
///
/// `normalizedPrice` is client-supplied and the webapp never sends it, so it is
/// only a fast path: the `price * conversionFactor` join is what actually
/// carries almost every real row. An unknown currency code falls back to a
/// factor of 1.0 rather than dropping the value, matching how the clients
/// display these figures.
async fn average_unit_price(pool: &SqlitePool, part_id: i64) -> AppResult<Option<f64>> {
    let row = sqlx::query(
        // The 0.0 literals matter: an integer 0 makes SQLite hand back an
        // INTEGER for `value` when no row carries a price, which then fails to
        // decode as f64.
        "SELECT CAST(COALESCE(SUM( \
                    COALESCE(s.normalizedPrice, s.price * COALESCE(c.conversionFactor, 1.0), 0.0) \
                ), 0.0) AS REAL) AS value, \
                COALESCE(SUM(s.quantity), 0) AS quantity \
         FROM partStocks s \
         LEFT JOIN currencies c ON c.code = s.currency \
         WHERE s.partId = ? AND s.deletedAt IS NULL",
    )
    .bind(part_id)
    .fetch_one(pool)
    .await?;

    let quantity: i64 = row.get("quantity");
    if quantity <= 0 {
        return Ok(None);
    }
    let value: f64 = row.get("value");
    Ok(Some(value / quantity as f64))
}

/// Recompute `maintenanceRecords.partsCost` from the record's live
/// consumptions. Call after any change to a consumption that points at a
/// maintenance record; see migration 046 for why this is stored rather than
/// derived on read.
///
/// A record whose consumptions are all gone goes back to NULL rather than 0.0,
/// so "no parts" and "parts worth nothing" stay distinguishable.
pub(crate) async fn recalculate_parts_cost(
    pool: &SqlitePool,
    maintenance_record_id: i64,
) -> AppResult<()> {
    let rows = sqlx::query(
        "SELECT partId, SUM(quantity) AS quantity FROM partConsumptions \
         WHERE maintenanceRecordId = ? AND deletedAt IS NULL GROUP BY partId",
    )
    .bind(maintenance_record_id)
    .fetch_all(pool)
    .await?;

    let parts_cost: Option<f64> = if rows.is_empty() {
        None
    } else {
        let mut total = 0.0f64;
        for row in &rows {
            let part_id: i64 = row.get("partId");
            let quantity: i64 = row.get("quantity");
            if let Some(unit) = average_unit_price(pool, part_id).await? {
                total += unit * quantity as f64;
            }
        }
        Some(total)
    };

    // `updatedAt` has to move as well: it is the `?since=` sync cursor, so a
    // partsCost written without it would never reach an offline client. Only
    // touched when the value actually changed, to avoid pointless sync churn
    // every time an unrelated consumption is saved.
    let now = sync_now();
    // `IS NOT` is SQLite's null-safe inequality, so this covers NULL <-> value
    // transitions in both directions without extra clauses.
    sqlx::query(
        "UPDATE maintenanceRecords SET partsCost = ?, updatedAt = ? \
         WHERE id = ? AND partsCost IS NOT ?",
    )
    .bind(parts_cost)
    .bind(&now)
    .bind(maintenance_record_id)
    .bind(parts_cost)
    .execute(pool)
    .await?;

    Ok(())
}

async fn fetch_series_ids(pool: &SqlitePool, part_id: i64) -> AppResult<Vec<i64>> {
    let rows =
        sqlx::query("SELECT seriesId FROM partSeriesCompat WHERE partId = ? ORDER BY seriesId ASC")
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

async fn validate_series_ids(pool: &SqlitePool, series_ids: &[i64], user_id: i64) -> AppResult<()> {
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
        let series_id: Option<Option<i64>> =
            sqlx::query_scalar("SELECT seriesId FROM motorcycles WHERE id = ? AND userId = ?")
                .bind(motorcycle_id)
                .bind(user.id)
                .fetch_optional(&pool)
                .await?;
        let series_id =
            series_id.ok_or_else(|| AppError::NotFound("Motorcycle not found".to_string()))?;
        match series_id {
            Some(sid) => {
                series_filter = Some(
                    crate::handlers::model_series::compatible_series_ids(&pool, sid, user.id)
                        .await?,
                )
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

    // Captured before the cascade: these records lose consumptions below, so
    // their partsCost has to be recomputed once the tombstones are committed.
    let affected_records: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT maintenanceRecordId FROM partConsumptions \
         WHERE partId = ? AND deletedAt IS NULL AND maintenanceRecordId IS NOT NULL",
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

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

    for record_id in affected_records {
        recalculate_parts_cost(&pool, record_id).await?;
    }

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

/// Hosts we are willing to fetch part images from. The endpoint takes a raw
/// URL, so without this allowlist it would be an SSRF proxy into whatever
/// network the API runs in.
const IMAGE_SOURCE_HOSTS: &[&str] = &[
    "bmw-classic-media-prod.s3.eu-central-1.amazonaws.com",
    "admin.bmwbike.com",
];

const IMAGE_DOWNLOAD_LIMIT: usize = 15 * 1024 * 1024; // matches the upload cap

/// Validate a user-supplied image source URL: https only, allowlisted host,
/// no credentials or port games. Returns the parsed URL.
fn validate_image_source(raw: &str) -> Result<url::Url, AppError> {
    let parsed =
        url::Url::parse(raw).map_err(|_| AppError::BadRequest("Ungültige Bild-URL".to_string()))?;
    let valid = parsed.scheme() == "https"
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.port().is_none()
        && parsed
            .host_str()
            .is_some_and(|host| IMAGE_SOURCE_HOSTS.contains(&host));
    if valid {
        Ok(parsed)
    } else {
        Err(AppError::BadRequest(
            "Bild-URL wird nicht unterstützt (nur BMWBike-Quellen)".to_string(),
        ))
    }
}

#[derive(serde::Deserialize)]
pub struct ImportImagePayload {
    pub url: String,
}

/// Download a part image from an allowlisted source (BMWBike import) and
/// store it exactly like an uploaded one.
pub async fn import_part_image_from_url(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    Json(payload): Json<ImportImagePayload>,
) -> AppResult<Json<Value>> {
    verify_part_ownership(&pool, id, user.id).await?;
    let source = validate_image_source(&payload.url)?;

    crate::install_crypto_provider();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let response = client
        .get(source)
        .send()
        .await
        .map_err(|_| AppError::BadRequest("Bild konnte nicht geladen werden".to_string()))?;
    if !response.status().is_success() {
        return Err(AppError::BadRequest(
            "Bild konnte nicht geladen werden".to_string(),
        ));
    }
    let content_type = response
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();
    if !content_type.starts_with("image/") {
        return Err(AppError::BadRequest(
            "Die URL liefert kein Bild".to_string(),
        ));
    }
    let data = response
        .bytes()
        .await
        .map_err(|_| AppError::BadRequest("Bild konnte nicht geladen werden".to_string()))?;
    if data.is_empty() || data.len() > IMAGE_DOWNLOAD_LIMIT {
        return Err(AppError::BadRequest(
            "Bild ist leer oder zu gross".to_string(),
        ));
    }

    let filename = save_image(&config, data.to_vec(), &content_type).await?;

    let old_image: Option<String> = sqlx::query_scalar("SELECT image FROM parts WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

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

#[cfg(test)]
mod image_source_tests {
    use super::validate_image_source;

    #[test]
    fn accepts_allowlisted_https_hosts() {
        assert!(validate_image_source(
            "https://bmw-classic-media-prod.s3.eu-central-1.amazonaws.com/some-image.png"
        )
        .is_ok());
        assert!(validate_image_source("https://admin.bmwbike.com/media/x.jpg").is_ok());
    }

    #[test]
    fn rejects_everything_else() {
        // Wrong host, wrong scheme, embedded credentials, explicit port, garbage.
        for url in [
            "https://example.com/x.png",
            "https://evil.bmwbike.com.attacker.net/x.png",
            "http://admin.bmwbike.com/x.png",
            "https://user:pw@admin.bmwbike.com/x.png",
            "https://admin.bmwbike.com:8443/x.png",
            "not a url",
            "file:///etc/passwd",
        ] {
            assert!(validate_image_source(url).is_err(), "{url}");
        }
    }
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
    // Catalog data of every other user's part is visible; availability and
    // stock detail are added below ONLY for parts marked public.
    let mut query_str = "SELECT p.id, p.partNumber, p.name, p.manufacturer, p.description, \
         p.image, p.isPublic, u.username as ownerName, \
         COALESCE((SELECT SUM(s.quantity) FROM partStocks s \
                   WHERE s.partId = p.id AND s.deletedAt IS NULL), 0) \
       - COALESCE((SELECT SUM(c.quantity) FROM partConsumptions c \
                   WHERE c.partId = p.id AND c.deletedAt IS NULL), 0) as totalQuantity \
         FROM parts p JOIN users u ON u.id = p.userId \
         WHERE p.deletedAt IS NULL AND p.userId != ?"
        .to_string();

    let search = filter
        .query
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty());
    if search.is_some() {
        query_str.push_str(" AND (p.partNumber LIKE ? OR p.name LIKE ?)");
    }
    if filter.series_id.is_some() {
        query_str.push_str(" AND p.id IN (SELECT partId FROM partSeriesCompat WHERE seriesId = ?)");
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

    // Full stock detail for the PUBLIC parts in the result set (whitelist:
    // private part ids never enter this query).
    let public_ids: Vec<i64> = rows
        .iter()
        .filter(|r| r.get::<i64, _>("isPublic") != 0)
        .map(|r| r.get::<i64, _>("id"))
        .collect();
    let mut stocks_by_part: HashMap<i64, Vec<crate::models::PublicStock>> = HashMap::new();
    if !public_ids.is_empty() {
        // Owner location hierarchies, for readable "Garage › Regal A" paths.
        let location_rows =
            sqlx::query("SELECT id, name, parentId FROM storageLocations WHERE deletedAt IS NULL")
                .fetch_all(&pool)
                .await?;
        let locations: HashMap<i64, (String, Option<i64>)> = location_rows
            .into_iter()
            .map(|r| {
                (
                    r.get::<i64, _>("id"),
                    (
                        r.get::<String, _>("name"),
                        r.get::<Option<i64>, _>("parentId"),
                    ),
                )
            })
            .collect();
        let location_path = |start: Option<i64>| -> Option<String> {
            let mut names = Vec::new();
            let mut cursor = start;
            while let Some(id) = cursor {
                let (name, parent) = locations.get(&id)?;
                names.push(name.clone());
                cursor = *parent;
                if names.len() > 10 {
                    break; // defensive: never loop on corrupt hierarchies
                }
            }
            names.reverse();
            Some(names.join(" › "))
        };

        let placeholders = vec!["?"; public_ids.len()].join(", ");
        let mut stock_query = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT partId, quantity, price, currency, purchaseDate, storageLocationId, isUsed \
             FROM partStocks WHERE deletedAt IS NULL AND partId IN ({}) \
             ORDER BY purchaseDate ASC, id ASC",
            placeholders
        )));
        for id in &public_ids {
            stock_query = stock_query.bind(id);
        }
        for row in stock_query.fetch_all(&pool).await? {
            stocks_by_part
                .entry(row.get("partId"))
                .or_default()
                .push(crate::models::PublicStock {
                    quantity: row.get("quantity"),
                    price: row.get("price"),
                    currency: row.get("currency"),
                    purchase_date: row.get("purchaseDate"),
                    storage_location: location_path(row.get("storageLocationId")),
                    is_used: row.get::<i64, _>("isUsed") != 0,
                });
        }
    }

    let parts: Vec<PublicPart> = rows
        .into_iter()
        .map(|row| {
            let id: i64 = row.get("id");
            let is_public = row.get::<i64, _>("isPublic") != 0;
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
                is_public,
                has_stock: is_public.then_some(total_quantity > 0),
                total_quantity: is_public.then_some(total_quantity),
                stocks: is_public.then(|| stocks_by_part.remove(&id).unwrap_or_default()),
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
    pub is_used: Option<bool>,
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
          storageLocationId, notes, isUsed, clientId, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(body.part_id)
    .bind(quantity)
    .bind(body.price)
    .bind(&body.currency)
    .bind(body.normalized_price)
    .bind(&body.purchase_date)
    .bind(body.storage_location_id)
    .bind(&body.notes)
    .bind(body.is_used.unwrap_or(false))
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
    pub is_used: Option<bool>,
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
    let is_used = body.is_used.unwrap_or(existing.is_used);

    let now = sync_now();
    sqlx::query(
        "UPDATE partStocks SET quantity = ?, price = ?, currency = ?, normalizedPrice = ?, \
         purchaseDate = ?, storageLocationId = ?, notes = ?, isUsed = ?, updatedAt = ? WHERE id = ?",
    )
    .bind(quantity)
    .bind(price)
    .bind(&currency)
    .bind(normalized_price)
    .bind(&purchase_date)
    .bind(storage_location_id)
    .bind(&notes)
    .bind(is_used)
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
    // The joined maintenance/motorcycle columns let a client link a consumption
    // back to the repair it belongs to (and name the bike) without an N+1 walk
    // over `/motorcycles/{id}/maintenance`. All additive: they are extra keys
    // on an existing response, and are NULL for a manual consumption that is
    // not tied to a record.
    let mut query_str = "SELECT c.*, \
            mr.motorcycleId AS motorcycleId, \
            mr.date AS maintenanceDate, \
            mr.type AS maintenanceType, \
            m.make AS motorcycleMake, \
            m.model AS motorcycleModel \
         FROM partConsumptions c \
         JOIN parts p ON p.id = c.partId \
         LEFT JOIN maintenanceRecords mr \
                ON mr.id = c.maintenanceRecordId AND mr.deletedAt IS NULL \
         LEFT JOIN motorcycles m ON m.id = mr.motorcycleId \
         WHERE p.userId = ?"
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

    let mut query = sqlx::query_as::<_, PartConsumptionWithContext>(sqlx::AssertSqlSafe(query_str))
        .bind(user.id);
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

    if let Some(mid) = body.maintenance_record_id {
        recalculate_parts_cost(&pool, mid).await?;
    }

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

    if let Some(mid) = existing.maintenance_record_id {
        recalculate_parts_cost(&pool, mid).await?;
    }

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
    // Captured before the tombstone so the record it was booked against can be
    // recosted afterwards.
    let maintenance_record_id: Option<i64> = sqlx::query_scalar(
        "SELECT c.maintenanceRecordId FROM partConsumptions c \
         JOIN parts p ON p.id = c.partId \
         WHERE c.id = ? AND p.userId = ? AND c.deletedAt IS NULL",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&pool)
    .await?
    .flatten();

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

    if let Some(mid) = maintenance_record_id {
        recalculate_parts_cost(&pool, mid).await?;
    }

    Ok(Json(json!({ "message": "Part consumption deleted" })))
}
