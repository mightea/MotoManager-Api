//! Torque values in the model catalog (see migration 064).
//!
//! Catalog lists are shared with every user of the instance, so they only get
//! there through an explicit "share" from a bike's Werkstattdaten. Importing
//! copies catalog rows back into a bike as ordinary `torqueSpecs` rows — the
//! same shape the own-bike import produces, so the iOS sync picks them up.

use std::collections::HashSet;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, Row, SqlitePool};

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::{
        maintenance::sync_now,
        model_series::{compatible_series_ids, verify_series_accessible},
        motorcycles::verify_motorcycle_ownership,
    },
    models::TorqueSpec,
};

/// A catalog torque value with the names a client needs to show its origin.
/// Contributors are identified by username only (never email).
#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct CatalogTorqueSpec {
    pub id: i64,
    pub series_id: i64,
    pub series_name: String,
    pub series_manufacturer: String,
    pub user_id: i64,
    pub contributor_username: String,
    pub category: String,
    pub name: String,
    pub torque: f64,
    pub torque_end: Option<f64>,
    pub variation: Option<f64>,
    pub tool_size: Option<String>,
    pub description: Option<String>,
    pub description_markup: Option<String>,
    pub unverified: bool,
    pub created_at: String,
}

const CATALOG_SELECT: &str = "SELECT c.id, c.seriesId, s.name AS seriesName, \
     s.manufacturer AS seriesManufacturer, c.userId, u.username AS contributorUsername, \
     c.category, c.name, c.torque, c.torqueEnd, c.variation, c.toolSize, c.description, \
     c.descriptionMarkup, c.unverified, c.createdAt \
     FROM modelSeriesTorqueSpecs c \
     JOIN modelSeries s ON s.id = c.seriesId \
     JOIN users u ON u.id = c.userId";

/// `GET /api/model-series/{sid}/torque-specs` — catalog values for the node
/// and every compatible node (ancestors-or-self + descendants, the same rule
/// parts use), plus one summary per contribution (user × node).
pub async fn list_catalog_torque_specs(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(sid): Path<i64>,
) -> AppResult<Json<Value>> {
    verify_series_accessible(&pool, sid, user.id).await?;
    let compatible = compatible_series_ids(&pool, sid, user.id).await?;

    let placeholders = vec!["?"; compatible.len()].join(",");
    let sql = format!(
        "{CATALOG_SELECT} WHERE c.seriesId IN ({placeholders}) \
         ORDER BY c.category ASC, c.name ASC, c.id ASC"
    );
    let mut query = sqlx::query_as::<_, CatalogTorqueSpec>(sqlx::AssertSqlSafe(sql));
    for id in &compatible {
        query = query.bind(id);
    }
    let specs = query.fetch_all(&pool).await?;

    // One summary per (node, contributor), in first-seen order of the newest
    // share first.
    let mut contributions: Vec<Value> = Vec::new();
    let mut seen: HashSet<(i64, i64)> = HashSet::new();
    let mut by_newest: Vec<&CatalogTorqueSpec> = specs.iter().collect();
    by_newest.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    for spec in by_newest {
        if !seen.insert((spec.series_id, spec.user_id)) {
            continue;
        }
        let count = specs
            .iter()
            .filter(|s| s.series_id == spec.series_id && s.user_id == spec.user_id)
            .count();
        contributions.push(json!({
            "seriesId": spec.series_id,
            "seriesName": spec.series_name,
            "seriesManufacturer": spec.series_manufacturer,
            "userId": spec.user_id,
            "contributorUsername": spec.contributor_username,
            "count": count,
            "sharedAt": spec.created_at,
            "isOwn": spec.user_id == user.id,
        }));
    }

    Ok(Json(json!({
        "torqueSpecs": specs,
        "contributions": contributions,
    })))
}

/// `POST /api/motorcycles/{id}/torque-specs/share-catalog` — copy the bike's
/// current torque specs into the catalog under the bike's node. Replaces this
/// user's earlier contribution for that node.
pub async fn share_torque_specs_to_catalog(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let series_id: Option<i64> = sqlx::query("SELECT seriesId FROM motorcycles WHERE id = ?")
        .bind(motorcycle_id)
        .fetch_one(&pool)
        .await?
        .get("seriesId");
    let series_id = series_id.ok_or_else(|| {
        AppError::BadRequest("Motorcycle has no model series assigned".to_string())
    })?;
    verify_series_accessible(&pool, series_id, user.id).await?;

    let specs = sqlx::query_as::<_, TorqueSpec>(
        "SELECT * FROM torqueSpecs WHERE motorcycleId = ? AND deletedAt IS NULL \
         ORDER BY category ASC, name ASC",
    )
    .bind(motorcycle_id)
    .fetch_all(&pool)
    .await?;
    if specs.is_empty() {
        return Err(AppError::BadRequest(
            "Motorcycle has no torque specs to share".to_string(),
        ));
    }

    let now = sync_now();
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM modelSeriesTorqueSpecs WHERE userId = ? AND seriesId = ?")
        .bind(user.id)
        .bind(series_id)
        .execute(&mut *tx)
        .await?;
    for spec in &specs {
        sqlx::query(
            "INSERT INTO modelSeriesTorqueSpecs \
             (seriesId, userId, sourceMotorcycleId, category, name, torque, torqueEnd, \
              variation, toolSize, description, descriptionMarkup, unverified, createdAt) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(series_id)
        .bind(user.id)
        .bind(motorcycle_id)
        .bind(&spec.category)
        .bind(&spec.name)
        .bind(spec.torque)
        .bind(spec.torque_end)
        .bind(spec.variation)
        .bind(&spec.tool_size)
        .bind(&spec.description)
        .bind(&spec.description_markup)
        .bind(spec.unverified)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(Json(json!({
        "message": format!("Shared {} torque specs", specs.len()),
        "seriesId": series_id,
        "count": specs.len(),
        "sharedAt": now,
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveContributionQuery {
    /// Contributor whose rows to remove; defaults to the caller. Only admins
    /// may remove someone else's contribution.
    pub user_id: Option<i64>,
}

/// `DELETE /api/model-series/{sid}/torque-specs[?userId=]` — withdraw a
/// contribution from the catalog node.
pub async fn remove_catalog_contribution(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(sid): Path<i64>,
    Query(query): Query<RemoveContributionQuery>,
) -> AppResult<Json<Value>> {
    let contributor = query.user_id.unwrap_or(user.id);
    if contributor != user.id && user.role != "admin" {
        return Err(AppError::Forbidden);
    }
    verify_series_accessible(&pool, sid, user.id).await?;

    let removed =
        sqlx::query("DELETE FROM modelSeriesTorqueSpecs WHERE seriesId = ? AND userId = ?")
            .bind(sid)
            .bind(contributor)
            .execute(&pool)
            .await?
            .rows_affected();
    if removed == 0 {
        return Err(AppError::NotFound(
            "Catalog contribution not found".to_string(),
        ));
    }

    Ok(Json(json!({
        "message": format!("Removed {} catalog torque specs", removed),
        "count": removed,
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCatalogRequest {
    pub catalog_spec_ids: Vec<i64>,
}

fn duplicate_key(category: &str, name: &str, torque: f64) -> (String, String, u64) {
    (
        category.trim().to_lowercase(),
        name.trim().to_lowercase(),
        torque.to_bits(),
    )
}

/// `POST /api/motorcycles/{id}/torque-specs/import-catalog` — copy catalog
/// values into the bike as ordinary torque specs (same sync fields as the
/// own-bike import). Exact duplicates (category + name + torque) already on
/// the bike — or earlier in the same request — are skipped.
pub async fn import_catalog_torque_specs(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Json(body): Json<ImportCatalogRequest>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let existing = sqlx::query_as::<_, TorqueSpec>(
        "SELECT * FROM torqueSpecs WHERE motorcycleId = ? AND deletedAt IS NULL",
    )
    .bind(motorcycle_id)
    .fetch_all(&pool)
    .await?;
    let mut present: HashSet<(String, String, u64)> = existing
        .iter()
        .map(|spec| duplicate_key(&spec.category, &spec.name, spec.torque))
        .collect();

    let now = sync_now();
    let mut imported_count: i64 = 0;
    let mut skipped_count: i64 = 0;
    let lookup = format!("{CATALOG_SELECT} WHERE c.id = ? AND (s.userId IS NULL OR s.userId = ?)");

    let mut tx = pool.begin().await?;
    for spec_id in &body.catalog_spec_ids {
        let Some(spec) =
            sqlx::query_as::<_, CatalogTorqueSpec>(sqlx::AssertSqlSafe(lookup.clone()))
                .bind(spec_id)
                .bind(user.id)
                .fetch_optional(&mut *tx)
                .await?
        else {
            continue;
        };

        if !present.insert(duplicate_key(&spec.category, &spec.name, spec.torque)) {
            skipped_count += 1;
            continue;
        }

        sqlx::query(
            "INSERT INTO torqueSpecs \
             (motorcycleId, category, name, torque, torqueEnd, variation, toolSize, description, descriptionMarkup, unverified, createdAt, updatedAt) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(motorcycle_id)
        .bind(&spec.category)
        .bind(&spec.name)
        .bind(spec.torque)
        .bind(spec.torque_end)
        .bind(spec.variation)
        .bind(&spec.tool_size)
        .bind(&spec.description)
        .bind(&spec.description_markup)
        .bind(spec.unverified)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        imported_count += 1;
    }
    tx.commit().await?;

    Ok(Json(json!({
        "message": format!("Imported {} torque specs", imported_count),
        "count": imported_count,
        "skipped": skipped_count,
    })))
}

/// Number of catalog torque values stored directly on each node (not the
/// hierarchy-merged view) — for the catalog management page.
pub async fn catalog_torque_spec_counts(pool: &SqlitePool) -> AppResult<Value> {
    let rows = sqlx::query(
        "SELECT seriesId, COUNT(*) AS cnt FROM modelSeriesTorqueSpecs GROUP BY seriesId",
    )
    .fetch_all(pool)
    .await?;
    let mut counts = serde_json::Map::new();
    for row in rows {
        counts.insert(
            row.get::<i64, _>("seriesId").to_string(),
            json!(row.get::<i64, _>("cnt")),
        );
    }
    Ok(Value::Object(counts))
}
