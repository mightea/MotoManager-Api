//! Odometer changes (migration 065): the instrument was replaced or rolled
//! over, so what it shows differs from the distance the bike has run.
//!
//! Every stored `odo` is the TOTAL distance. This module owns the two places
//! where that invariant needs help:
//!
//! * recording, editing or deleting a change re-bases the records dated on or
//!   after it ([`shift_records`]), and
//! * record writes are normalised to the total ([`resolve_odo`]), whatever
//!   offset the client did or did not apply.

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::{
        maintenance::{recalculate_fuel_chain, sync_now},
        motorcycles::verify_motorcycle_ownership,
    },
    models::OdometerChange,
};

impl OdometerChange {
    /// Distance this change adds to every later instrument reading.
    pub(crate) fn offset(&self) -> i64 {
        self.reading_before - self.reading_after
    }
}

/// The day part of a stored date ("2018-11-22" or an ISO timestamp).
fn day(date: &str) -> &str {
    date.get(..10).unwrap_or(date)
}

/// Changes of one motorcycle, oldest first.
pub(crate) async fn load_changes(
    pool: &SqlitePool,
    motorcycle_id: i64,
) -> AppResult<Vec<OdometerChange>> {
    Ok(sqlx::query_as::<_, OdometerChange>(
        "SELECT * FROM odometerChanges WHERE motorcycleId = ? ORDER BY date ASC, id ASC",
    )
    .bind(motorcycle_id)
    .fetch_all(pool)
    .await?)
}

/// Offset that applies to a record dated `date`: every change on or before
/// that day counts, so a record on the day of the swap was read off the new
/// instrument.
pub(crate) fn offset_at(changes: &[OdometerChange], date: &str) -> i64 {
    changes
        .iter()
        .filter(|c| day(&c.date) <= day(date))
        .map(OdometerChange::offset)
        .sum()
}

/// The record a write is about, so it is not compared against itself.
#[derive(Debug, Clone, Copy)]
pub(crate) enum OdoRecord {
    New,
    Maintenance(i64),
    Issue(i64),
}

/// Total distance to store for a record write.
///
/// * `client_offset` present — the client added that much to the instrument
///   reading. It is swapped for the server's offset for the record's date, so
///   a client working from a stale list of changes (offline, or before the
///   swap was recorded) still ends up with the right total.
/// * absent — a client that predates this feature (see [`resolve_legacy`]).
pub(crate) async fn resolve_odo(
    pool: &SqlitePool,
    motorcycle_id: i64,
    date: &str,
    odo: i64,
    client_offset: Option<i64>,
    record: OdoRecord,
) -> AppResult<i64> {
    let changes = load_changes(pool, motorcycle_id).await?;
    let server_offset = offset_at(&changes, date);
    let resolved = match client_offset {
        Some(client_offset) => odo - client_offset + server_offset,
        None => match latest_change(&changes, date) {
            None => odo,
            Some((change_day, total_at_change)) => {
                let previous =
                    previous_reading(pool, motorcycle_id, change_day, date, record).await?;
                resolve_legacy(odo, server_offset, previous.unwrap_or(total_at_change))
            }
        },
    };
    if resolved < 0 {
        return Err(AppError::BadRequest("odo must not be negative".to_string()));
    }
    Ok(resolved)
}

/// Day of the latest change in force on `date`, and the total distance at that
/// moment (nothing recorded after it can legitimately be lower).
fn latest_change<'a>(changes: &'a [OdometerChange], date: &str) -> Option<(&'a str, i64)> {
    let applicable: Vec<&OdometerChange> = changes
        .iter()
        .filter(|c| day(&c.date) <= day(date))
        .collect();
    let (last, earlier) = applicable.split_last()?;
    let total = last.reading_before + earlier.iter().map(|c| c.offset()).sum::<i64>();
    Some((day(&last.date), total))
}

/// Highest stored total among the records dated from the latest change up to
/// the day before `date`. Same-day records are left out: several entries of
/// one workshop day are routinely a few km apart in either direction.
async fn previous_reading(
    pool: &SqlitePool,
    motorcycle_id: i64,
    change_day: &str,
    date: &str,
    record: OdoRecord,
) -> AppResult<Option<i64>> {
    let (maintenance_id, issue_id) = match record {
        OdoRecord::New => (-1, -1),
        OdoRecord::Maintenance(id) => (id, -1),
        OdoRecord::Issue(id) => (-1, id),
    };
    Ok(sqlx::query_scalar(
        "SELECT MAX(o) FROM ( \
           SELECT odo AS o FROM maintenanceRecords \
            WHERE motorcycleId = ?1 AND deletedAt IS NULL AND id != ?4 \
              AND substr(date, 1, 10) >= ?2 AND substr(date, 1, 10) < ?3 \
           UNION ALL \
           SELECT odo FROM issues \
            WHERE motorcycleId = ?1 AND deletedAt IS NULL AND id != ?5 \
              AND date IS NOT NULL \
              AND substr(date, 1, 10) >= ?2 AND substr(date, 1, 10) < ?3)",
    )
    .bind(motorcycle_id)
    .bind(change_day)
    .bind(day(date))
    .bind(maintenance_id)
    .bind(issue_id)
    .fetch_one(pool)
    .await?)
}

/// A step backwards counts this many times as much as a step forwards when
/// judging how far a candidate lies from the previous reading.
const BACKWARDS_PENALTY: i64 = 4;

/// `odo` from a client that knows nothing about odometer changes.
///
/// Such a client shows the stored totals, so its user types either the total
/// (carrying on from the number on screen) or what the instrument shows. The
/// two differ by exactly `offset`; the one that continues from `previous`
/// (the bike's latest earlier total) is taken. Odometers run forwards, so a
/// candidate below `previous` is the less likely one — but a small step back
/// (a correction) still beats a jump of a whole offset.
///
/// This holds for the life of the bike: unlike a fixed threshold it keeps
/// working once the new instrument passes the old one's final reading.
fn resolve_legacy(odo: i64, offset: i64, previous: i64) -> i64 {
    if offset == 0 {
        return odo;
    }
    let distance = |candidate: i64| {
        if candidate < 0 {
            i64::MAX
        } else if candidate >= previous {
            candidate - previous
        } else {
            (previous - candidate).saturating_mul(BACKWARDS_PENALTY)
        }
    };
    let lifted = odo + offset;
    if distance(lifted) < distance(odo) {
        lifted
    } else {
        odo
    }
}

fn change_json(change: &OdometerChange) -> Value {
    let mut value = serde_json::to_value(change).unwrap_or(json!({}));
    if let Some(obj) = value.as_object_mut() {
        obj.insert("offset".to_string(), json!(change.offset()));
    }
    value
}

fn changes_json(changes: &[OdometerChange]) -> Value {
    Value::Array(changes.iter().map(change_json).collect())
}

/// Adds `odometerChanges` and `odometerOffset` (the offset in force today) to
/// a serialized motorcycle. Both keys are additive; older clients ignore them.
pub(crate) fn attach_to_motorcycle(motorcycle: &mut Value, changes: &[OdometerChange]) {
    if let Some(obj) = motorcycle.as_object_mut() {
        let total: i64 = changes.iter().map(OdometerChange::offset).sum();
        obj.insert("odometerChanges".to_string(), changes_json(changes));
        obj.insert("odometerOffset".to_string(), json!(total));
    }
}

/// Changes of every motorcycle of a user, grouped by motorcycle.
pub(crate) async fn load_changes_for_user(
    pool: &SqlitePool,
    user_id: i64,
) -> AppResult<HashMap<i64, Vec<OdometerChange>>> {
    let rows = sqlx::query_as::<_, OdometerChange>(
        "SELECT oc.* FROM odometerChanges oc \
         JOIN motorcycles m ON m.id = oc.motorcycleId \
         WHERE m.userId = ? ORDER BY oc.date ASC, oc.id ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let mut grouped: HashMap<i64, Vec<OdometerChange>> = HashMap::new();
    for row in rows {
        grouped.entry(row.motorcycle_id).or_default().push(row);
    }
    Ok(grouped)
}

/// Moves every odometer value dated on or after `from_date` by `delta`.
/// Returns how many maintenance records and issues moved.
///
/// Sync-enabled rows get a fresh `updatedAt` so offline clients pull the new
/// totals. `initialOdo` and `saleOdo` move only when the purchase / sale is
/// dated on or after the change (they were read off the new instrument);
/// `manualOdo` carries no date and is left alone.
async fn shift_records(
    tx: &mut Transaction<'_, Sqlite>,
    motorcycle_id: i64,
    from_date: &str,
    delta: i64,
    now: &str,
) -> AppResult<u64> {
    if delta == 0 {
        return Ok(0);
    }
    let from = day(from_date);
    let mut moved = sqlx::query(
        "UPDATE maintenanceRecords SET odo = odo + ?, updatedAt = ? \
         WHERE motorcycleId = ? AND substr(date, 1, 10) >= ?",
    )
    .bind(delta)
    .bind(now)
    .bind(motorcycle_id)
    .bind(from)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    moved += sqlx::query(
        "UPDATE issues SET odo = odo + ?, updatedAt = ? \
         WHERE motorcycleId = ? AND date IS NOT NULL AND substr(date, 1, 10) >= ?",
    )
    .bind(delta)
    .bind(now)
    .bind(motorcycle_id)
    .bind(from)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    sqlx::query(
        "UPDATE locationRecords SET odometer = odometer + ? \
         WHERE motorcycleId = ? AND odometer IS NOT NULL AND substr(date, 1, 10) >= ?",
    )
    .bind(delta)
    .bind(motorcycle_id)
    .bind(from)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE motorcycles SET saleOdo = saleOdo + ? \
         WHERE id = ? AND saleOdo IS NOT NULL AND soldDate IS NOT NULL \
           AND substr(soldDate, 1, 10) >= ?",
    )
    .bind(delta)
    .bind(motorcycle_id)
    .bind(from)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE motorcycles SET initialOdo = initialOdo + ? \
         WHERE id = ? AND purchaseDate IS NOT NULL AND substr(purchaseDate, 1, 10) >= ?",
    )
    .bind(delta)
    .bind(motorcycle_id)
    .bind(from)
    .execute(&mut **tx)
    .await?;
    Ok(moved)
}

/// A shift must not push any reading below zero (a change whose new
/// instrument showed far more than the old one, dated before low records).
async fn ensure_no_negative_readings(
    tx: &mut Transaction<'_, Sqlite>,
    motorcycle_id: i64,
) -> AppResult<()> {
    let lowest: Option<i64> = sqlx::query_scalar(
        "SELECT MIN(o) FROM ( \
           SELECT odo AS o FROM maintenanceRecords WHERE motorcycleId = ?1 \
           UNION ALL SELECT odo FROM issues WHERE motorcycleId = ?1 \
           UNION ALL SELECT odometer FROM locationRecords WHERE motorcycleId = ?1 \
           UNION ALL SELECT saleOdo FROM motorcycles WHERE id = ?1 \
           UNION ALL SELECT initialOdo FROM motorcycles WHERE id = ?1)",
    )
    .bind(motorcycle_id)
    .fetch_one(&mut **tx)
    .await?;
    if lowest.is_some_and(|v| v < 0) {
        return Err(AppError::BadRequest(
            "This change would make existing odometer readings negative".to_string(),
        ));
    }
    Ok(())
}

fn validate_date(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
        .map(|d| d.format("%Y-%m-%d").to_string())
        .map_err(|_| AppError::BadRequest("date must be YYYY-MM-DD".to_string()))
}

fn validate_reading(name: &str, value: i64) -> AppResult<i64> {
    if value < 0 {
        return Err(AppError::BadRequest(format!("{name} must not be negative")));
    }
    Ok(value)
}

fn validate_kind(raw: Option<&str>) -> AppResult<Option<String>> {
    match raw {
        None => Ok(None),
        Some(k @ ("replacement" | "rollover")) => Ok(Some(k.to_string())),
        Some(_) => Err(AppError::BadRequest(
            "kind must be \"replacement\" or \"rollover\"".to_string(),
        )),
    }
}

async fn find_change(
    pool: &SqlitePool,
    motorcycle_id: i64,
    change_id: i64,
) -> AppResult<OdometerChange> {
    sqlx::query_as::<_, OdometerChange>(
        "SELECT * FROM odometerChanges WHERE id = ? AND motorcycleId = ?",
    )
    .bind(change_id)
    .bind(motorcycle_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Odometer change not found".to_string()))
}

/// Body shared by every response: the full, current list for the motorcycle.
async fn list_body(pool: &SqlitePool, motorcycle_id: i64) -> AppResult<Value> {
    let changes = load_changes(pool, motorcycle_id).await?;
    let total: i64 = changes.iter().map(OdometerChange::offset).sum();
    Ok(json!({
        "odometerChanges": changes_json(&changes),
        "odometerOffset": total,
    }))
}

pub async fn list_odometer_changes(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;
    Ok(Json(list_body(&pool, motorcycle_id).await?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOdometerChangeRequest {
    pub date: String,
    pub reading_before: i64,
    pub reading_after: i64,
    pub kind: Option<String>,
}

pub async fn create_odometer_change(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Json(body): Json<CreateOdometerChangeRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let date = validate_date(&body.date)?;
    let reading_before = validate_reading("readingBefore", body.reading_before)?;
    let reading_after = validate_reading("readingAfter", body.reading_after)?;
    let kind = validate_kind(body.kind.as_deref())?.unwrap_or_else(|| "replacement".to_string());
    let now = sync_now();

    let mut tx = pool.begin().await?;
    let id = sqlx::query(
        "INSERT INTO odometerChanges \
         (motorcycleId, date, readingBefore, readingAfter, kind, createdAt, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(motorcycle_id)
    .bind(&date)
    .bind(reading_before)
    .bind(reading_after)
    .bind(&kind)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    let shifted = shift_records(
        &mut tx,
        motorcycle_id,
        &date,
        reading_before - reading_after,
        &now,
    )
    .await?;
    ensure_no_negative_readings(&mut tx, motorcycle_id).await?;
    tx.commit().await?;

    // Trip distances and consumption are measured between odometer values.
    recalculate_fuel_chain(&pool, motorcycle_id).await?;

    tracing::info!(
        "Odometer change {} recorded for motorcycle {} ({} records shifted)",
        id,
        motorcycle_id,
        shifted
    );
    let change = find_change(&pool, motorcycle_id, id).await?;
    let mut response = list_body(&pool, motorcycle_id).await?;
    response["odometerChange"] = change_json(&change);
    response["shiftedRecords"] = json!(shifted);
    Ok((StatusCode::CREATED, Json(response)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOdometerChangeRequest {
    pub date: Option<String>,
    pub reading_before: Option<i64>,
    pub reading_after: Option<i64>,
    pub kind: Option<String>,
}

pub async fn update_odometer_change(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((motorcycle_id, change_id)): Path<(i64, i64)>,
    Json(body): Json<UpdateOdometerChangeRequest>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;
    let existing = find_change(&pool, motorcycle_id, change_id).await?;

    let date = match body.date.as_deref() {
        Some(raw) => validate_date(raw)?,
        None => existing.date.clone(),
    };
    let reading_before = validate_reading(
        "readingBefore",
        body.reading_before.unwrap_or(existing.reading_before),
    )?;
    let reading_after = validate_reading(
        "readingAfter",
        body.reading_after.unwrap_or(existing.reading_after),
    )?;
    let kind = validate_kind(body.kind.as_deref())?.unwrap_or_else(|| existing.kind.clone());
    let new_offset = reading_before - reading_after;
    let now = sync_now();

    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE odometerChanges SET date = ?, readingBefore = ?, readingAfter = ?, kind = ?, \
         updatedAt = ? WHERE id = ?",
    )
    .bind(&date)
    .bind(reading_before)
    .bind(reading_after)
    .bind(&kind)
    .bind(&now)
    .bind(change_id)
    .execute(&mut *tx)
    .await?;
    // Take the old change back out, then apply the new one. Skipped when the
    // effect on records is identical, so a kind-only edit touches no record.
    let mut shifted = 0;
    if day(&existing.date) != date || existing.offset() != new_offset {
        shift_records(
            &mut tx,
            motorcycle_id,
            &existing.date,
            -existing.offset(),
            &now,
        )
        .await?;
        shifted = shift_records(&mut tx, motorcycle_id, &date, new_offset, &now).await?;
        ensure_no_negative_readings(&mut tx, motorcycle_id).await?;
    }
    tx.commit().await?;

    recalculate_fuel_chain(&pool, motorcycle_id).await?;

    let change = find_change(&pool, motorcycle_id, change_id).await?;
    let mut response = list_body(&pool, motorcycle_id).await?;
    response["odometerChange"] = change_json(&change);
    response["shiftedRecords"] = json!(shifted);
    Ok(Json(response))
}

pub async fn delete_odometer_change(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((motorcycle_id, change_id)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;
    let existing = find_change(&pool, motorcycle_id, change_id).await?;
    let now = sync_now();

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM odometerChanges WHERE id = ?")
        .bind(change_id)
        .execute(&mut *tx)
        .await?;
    let shifted = shift_records(
        &mut tx,
        motorcycle_id,
        &existing.date,
        -existing.offset(),
        &now,
    )
    .await?;
    ensure_no_negative_readings(&mut tx, motorcycle_id).await?;
    tx.commit().await?;

    recalculate_fuel_chain(&pool, motorcycle_id).await?;

    let mut response = list_body(&pool, motorcycle_id).await?;
    response["shiftedRecords"] = json!(shifted);
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(date: &str, before: i64, after: i64) -> OdometerChange {
        OdometerChange {
            id: 0,
            motorcycle_id: 1,
            date: date.to_string(),
            reading_before: before,
            reading_after: after,
            kind: "replacement".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn offset_counts_changes_on_or_before_the_record_date() {
        let changes = [
            change("2010-05-01", 99_999, 0),
            change("2018-11-22", 70_000, 7_000),
        ];
        assert_eq!(offset_at(&changes, "2009-01-01"), 0);
        assert_eq!(offset_at(&changes, "2010-05-01"), 99_999);
        assert_eq!(offset_at(&changes, "2018-11-21"), 99_999);
        assert_eq!(offset_at(&changes, "2018-11-22T08:00:00Z"), 162_999);
    }

    #[test]
    fn total_at_the_latest_change_adds_up_earlier_offsets() {
        let changes = [
            change("2010-05-01", 99_999, 0),
            change("2018-11-22", 70_000, 7_000),
        ];
        assert_eq!(latest_change(&changes, "2009-01-01"), None);
        assert_eq!(
            latest_change(&changes, "2012-01-01"),
            Some(("2010-05-01", 99_999))
        );
        assert_eq!(
            latest_change(&changes, "2020-01-01"),
            Some(("2018-11-22", 169_999))
        );
    }

    #[test]
    fn legacy_value_continues_from_the_previous_reading() {
        // Instrument reading is lifted, a total is kept.
        assert_eq!(resolve_legacy(8_560, 63_000, 71_402), 71_560);
        assert_eq!(resolve_legacy(71_560, 63_000, 71_402), 71_560);
        // Still right once the new instrument has passed the old one's final
        // reading (70'000), where a fixed threshold would give up.
        assert_eq!(resolve_legacy(75_000, 63_000, 137_900), 138_000);
        assert_eq!(resolve_legacy(138_000, 63_000, 137_900), 138_000);
        // A total corrected slightly downwards is not mistaken for a reading.
        assert_eq!(resolve_legacy(71_300, 63_000, 71_396), 71_300);
        // Small offset: 600 km after the previous reading, either way round.
        assert_eq!(resolve_legacy(19_600, 1_000, 20_000), 20_600);
        assert_eq!(resolve_legacy(20_600, 1_000, 20_000), 20_600);
        // New instrument shows more than the old one did (negative offset).
        assert_eq!(resolve_legacy(50_600, -30_000, 20_000), 20_600);
        assert_eq!(resolve_legacy(20_600, -30_000, 20_000), 20_600);
        // No offset in force: untouched.
        assert_eq!(resolve_legacy(8_560, 0, 71_402), 8_560);
    }
}
