use std::collections::{HashMap, HashSet};

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
    models::ModelSeries,
};

/// Familie -> Serie -> Modell.
const MAX_DEPTH: usize = 3;

/// Helper: a catalog node is usable by a user when it is a global seed row
/// (userId NULL) or one of their own custom entries.
pub async fn verify_series_accessible(
    pool: &SqlitePool,
    series_id: i64,
    user_id: i64,
) -> AppResult<()> {
    let count: i64 = sqlx::query(
        "SELECT COUNT(*) as cnt FROM modelSeries WHERE id = ? AND (userId IS NULL OR userId = ?)",
    )
    .bind(series_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?
    .get("cnt");
    if count == 0 {
        return Err(AppError::NotFound("Model series not found".to_string()));
    }
    Ok(())
}

/// All catalog nodes visible to a user, as (id -> parentId).
async fn accessible_tree(pool: &SqlitePool, user_id: i64) -> AppResult<HashMap<i64, Option<i64>>> {
    let rows =
        sqlx::query("SELECT id, parentId FROM modelSeries WHERE userId IS NULL OR userId = ?")
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    Ok(rows
        .iter()
        .map(|row| {
            (
                row.get::<i64, _>("id"),
                row.get::<Option<i64>, _>("parentId"),
            )
        })
        .collect())
}

/// 1-based depth of a node (Familie = 1); capped walk so a (rejected) cycle
/// can't loop forever.
fn depth_of(node: i64, tree: &HashMap<i64, Option<i64>>) -> usize {
    let mut depth = 1;
    let mut current = node;
    for _ in 0..MAX_DEPTH * 2 {
        match tree.get(&current).copied().flatten() {
            Some(parent) => {
                depth += 1;
                current = parent;
            }
            None => break,
        }
    }
    depth
}

/// Height of a node's subtree (1 = leaf).
fn subtree_height(node: i64, tree: &HashMap<i64, Option<i64>>) -> usize {
    let mut children_of: HashMap<i64, Vec<i64>> = HashMap::new();
    for (id, parent) in tree {
        if let Some(parent) = parent {
            children_of.entry(*parent).or_default().push(*id);
        }
    }
    fn walk(node: i64, children_of: &HashMap<i64, Vec<i64>>, budget: usize) -> usize {
        if budget == 0 {
            return 1;
        }
        1 + children_of
            .get(&node)
            .map(|children| {
                children
                    .iter()
                    .map(|child| walk(*child, children_of, budget - 1))
                    .max()
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }
    walk(node, &children_of, MAX_DEPTH * 2)
}

/// The node ids a bike/part linked to `series_id` is compatible with:
/// ancestors-or-self plus all descendants — a part linked to a Familie fits
/// every Modell below it, and a part linked to a Modell also matches a bike
/// assigned to the surrounding Serie or Familie.
pub async fn compatible_series_ids(
    pool: &SqlitePool,
    series_id: i64,
    user_id: i64,
) -> AppResult<Vec<i64>> {
    let tree = accessible_tree(pool, user_id).await?;
    let mut matches: HashSet<i64> = HashSet::new();

    // Ancestors + self.
    let mut current = Some(series_id);
    for _ in 0..MAX_DEPTH * 2 {
        let Some(id) = current else { break };
        matches.insert(id);
        current = tree.get(&id).copied().flatten();
    }

    // Descendants (breadth-first over the small tree).
    let mut frontier = vec![series_id];
    for _ in 0..MAX_DEPTH * 2 {
        let next: Vec<i64> = tree
            .iter()
            .filter(|(id, parent)| {
                parent.map(|p| frontier.contains(&p)) == Some(true) && !matches.contains(id)
            })
            .map(|(id, _)| *id)
            .collect();
        if next.is_empty() {
            break;
        }
        matches.extend(next.iter().copied());
        frontier = next;
    }

    Ok(matches.into_iter().collect())
}

pub async fn list_model_series(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let series = sqlx::query_as::<_, ModelSeries>(
        "SELECT * FROM modelSeries WHERE userId IS NULL OR userId = ? \
         ORDER BY manufacturer ASC, name ASC",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(json!({ "modelSeries": series })))
}

/// Normalize a user-supplied type-code list: 4 alphanumeric chars starting
/// with a digit (classic "0502" and modern "0A01" style), comma-separated,
/// uppercased, deduplicated. Returns None when nothing valid remains.
fn normalize_type_codes(raw: Option<String>) -> Option<String> {
    let raw = raw?;
    let mut codes: Vec<String> = Vec::new();
    for part in raw.split(',') {
        let code = part.trim().to_uppercase();
        let valid = code.len() == 4
            && code.chars().all(|c| c.is_ascii_alphanumeric())
            && code.chars().next().is_some_and(|c| c.is_ascii_digit());
        if valid && !codes.contains(&code) {
            codes.push(code);
        }
    }
    if codes.is_empty() {
        None
    } else {
        Some(codes.join(","))
    }
}

/// Parse a comma-separated "start-end" range list into numeric pairs.
fn parse_frame_ranges(raw: &str) -> Vec<(u64, u64)> {
    raw.split(',')
        .filter_map(|part| {
            let (start, end) = part.trim().split_once('-')?;
            let start: u64 = start.trim().parse().ok()?;
            let end: u64 = end.trim().parse().ok()?;
            (start <= end).then_some((start, end))
        })
        .collect()
}

/// Normalize a user-supplied frame-range list; returns None when nothing
/// valid remains.
fn normalize_frame_ranges(raw: Option<String>) -> Option<String> {
    let raw = raw?;
    let ranges = parse_frame_ranges(&raw);
    if ranges.is_empty() {
        None
    } else {
        Some(
            ranges
                .iter()
                .map(|(start, end)| format!("{}-{}", start, end))
                .collect::<Vec<_>>()
                .join(","),
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateModelSeriesRequest {
    pub name: String,
    pub manufacturer: Option<String>,
    /// Parent node (Familie or Serie); absent = new root-level Familie.
    pub parent_id: Option<i64>,
    /// Comma-separated 4-digit BMW type codes for VIN decoding.
    pub type_codes: Option<String>,
    /// Comma-separated "start-end" frame-number ranges (pre-1981 bikes).
    pub frame_ranges: Option<String>,
}

pub async fn create_model_series(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreateModelSeriesRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }
    let manufacturer = body
        .manufacturer
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "BMW".to_string());

    if let Some(parent_id) = body.parent_id {
        verify_series_accessible(&pool, parent_id, user.id).await?;
        let tree = accessible_tree(&pool, user.id).await?;
        if depth_of(parent_id, &tree) >= MAX_DEPTH {
            return Err(AppError::BadRequest(
                "Maximum catalog depth is Familie > Serie > Modell".to_string(),
            ));
        }
    }

    // Idempotent create: an equal global or own entry under the same parent is
    // returned instead of erroring, so retries stay safe.
    if let Some(existing) = sqlx::query_as::<_, ModelSeries>(
        "SELECT * FROM modelSeries WHERE manufacturer = ? AND name = ? \
         AND (userId IS NULL OR userId = ?) \
         AND ((parentId IS NULL AND ? IS NULL) OR parentId = ?)",
    )
    .bind(&manufacturer)
    .bind(&name)
    .bind(user.id)
    .bind(body.parent_id)
    .bind(body.parent_id)
    .fetch_optional(&pool)
    .await?
    {
        return Ok((StatusCode::OK, Json(json!({ "modelSeries": existing }))));
    }

    let id = sqlx::query(
        "INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes, frameRanges, userId) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&name)
    .bind(&manufacturer)
    .bind(body.parent_id)
    .bind(normalize_type_codes(body.type_codes))
    .bind(normalize_frame_ranges(body.frame_ranges))
    .bind(user.id)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    let series = sqlx::query_as::<_, ModelSeries>("SELECT * FROM modelSeries WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok((StatusCode::CREATED, Json(json!({ "modelSeries": series }))))
}

/// Distinguish "field absent" from "explicit null" (move to root level).
fn double_option<'de, D>(deserializer: D) -> Result<Option<Option<i64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    Ok(Some(Option::<i64>::deserialize(deserializer)?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateModelSeriesRequest {
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    /// Absent = keep, null = move to root level, value = re-parent.
    #[serde(default, deserialize_with = "double_option")]
    pub parent_id: Option<Option<i64>>,
    /// Absent = keep, null/empty = clear, value = replace.
    #[serde(default, deserialize_with = "double_option_string")]
    pub type_codes: Option<Option<String>>,
    /// Absent = keep, null/empty = clear, value = replace.
    #[serde(default, deserialize_with = "double_option_string")]
    pub frame_ranges: Option<Option<String>>,
}

/// String twin of `double_option`.
fn double_option_string<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    Ok(Some(Option::<String>::deserialize(deserializer)?))
}

pub async fn update_model_series(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(sid): Path<i64>,
    Json(body): Json<UpdateModelSeriesRequest>,
) -> AppResult<Json<Value>> {
    // Only own custom entries are editable; global and foreign rows are masked.
    let existing =
        sqlx::query_as::<_, ModelSeries>("SELECT * FROM modelSeries WHERE id = ? AND userId = ?")
            .bind(sid)
            .bind(user.id)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Model series not found".to_string()))?;

    let name = body.name.unwrap_or(existing.name);
    let manufacturer = body.manufacturer.unwrap_or(existing.manufacturer);

    let parent_id = match body.parent_id {
        None => existing.parent_id,
        Some(None) => None,
        Some(Some(parent_id)) => {
            verify_series_accessible(&pool, parent_id, user.id).await?;
            Some(parent_id)
        }
    };

    if let Some(parent_id) = parent_id {
        let tree = accessible_tree(&pool, user.id).await?;
        // Cycle check: the new parent must not sit inside this node's subtree.
        let mut current = Some(parent_id);
        for _ in 0..MAX_DEPTH * 2 {
            let Some(id) = current else { break };
            if id == sid {
                return Err(AppError::BadRequest(
                    "A catalog entry cannot be its own ancestor".to_string(),
                ));
            }
            current = tree.get(&id).copied().flatten();
        }
        // The whole subtree must still fit within the depth cap.
        if depth_of(parent_id, &tree) + subtree_height(sid, &tree) > MAX_DEPTH {
            return Err(AppError::BadRequest(
                "Maximum catalog depth is Familie > Serie > Modell".to_string(),
            ));
        }
    }

    let type_codes = match body.type_codes {
        None => existing.type_codes,
        Some(raw) => normalize_type_codes(raw),
    };
    let frame_ranges = match body.frame_ranges {
        None => existing.frame_ranges,
        Some(raw) => normalize_frame_ranges(raw),
    };

    sqlx::query(
        "UPDATE modelSeries SET name = ?, manufacturer = ?, parentId = ?, typeCodes = ?, \
         frameRanges = ? WHERE id = ? AND userId = ?",
    )
    .bind(&name)
    .bind(&manufacturer)
    .bind(parent_id)
    .bind(&type_codes)
    .bind(&frame_ranges)
    .bind(sid)
    .bind(user.id)
    .execute(&pool)
    .await?;

    let series = sqlx::query_as::<_, ModelSeries>("SELECT * FROM modelSeries WHERE id = ?")
        .bind(sid)
        .fetch_one(&pool)
        .await?;

    Ok(Json(json!({ "modelSeries": series })))
}

// ---------------------------------------------------------------------------
// VIN decoding
// ---------------------------------------------------------------------------

/// ISO 3779 check-digit transliteration (I, O, Q are never used in VINs).
fn transliterate(c: char) -> Option<u32> {
    Some(match c {
        '0'..='9' => c as u32 - '0' as u32,
        'A' | 'J' => 1,
        'B' | 'K' | 'S' => 2,
        'C' | 'L' | 'T' => 3,
        'D' | 'M' | 'U' => 4,
        'E' | 'N' | 'V' => 5,
        'F' | 'W' => 6,
        'G' | 'P' | 'X' => 7,
        'H' | 'Y' => 8,
        'R' | 'Z' => 9,
        _ => return None,
    })
}

/// Position-9 check digit per ISO 3779. Not every ECE-market VIN carries a
/// valid one, so this is surfaced as a hint rather than used for rejection.
fn check_digit_valid(chars: &[char]) -> bool {
    const WEIGHTS: [u32; 17] = [8, 7, 6, 5, 4, 3, 2, 10, 0, 9, 8, 7, 6, 5, 4, 3, 2];
    let mut sum = 0;
    for (i, &c) in chars.iter().enumerate() {
        match transliterate(c) {
            Some(value) => sum += value * WEIGHTS[i],
            None => return false,
        }
    }
    let remainder = sum % 11;
    let expected = if remainder == 10 {
        'X'
    } else {
        char::from_digit(remainder, 10).unwrap()
    };
    chars[8] == expected
}

/// Model-year letter at position 10. The letter cycle repeats from 2010, but
/// this catalog targets the classic range, so the 1980-2009 reading is used.
fn decode_model_year(c: char) -> Option<i64> {
    Some(match c {
        'A' => 1980,
        'B' => 1981,
        'C' => 1982,
        'D' => 1983,
        'E' => 1984,
        'F' => 1985,
        'G' => 1986,
        'H' => 1987,
        'J' => 1988,
        'K' => 1989,
        'L' => 1990,
        'M' => 1991,
        'N' => 1992,
        'P' => 1993,
        'R' => 1994,
        'S' => 1995,
        'T' => 1996,
        'V' => 1997,
        'W' => 1998,
        'X' => 1999,
        'Y' => 2000,
        '1' => 2001,
        '2' => 2002,
        '3' => 2003,
        '4' => 2004,
        '5' => 2005,
        '6' => 2006,
        '7' => 2007,
        '8' => 2008,
        '9' => 2009,
        _ => return None,
    })
}

#[derive(Debug, Deserialize)]
pub struct VinQuery {
    pub vin: String,
}

/// Decode a BMW Motorrad VIN or a pre-1981 frame number.
///
/// 17-char VINs: characters 4-7 carry the 4-digit type code (Baumuster),
/// which maps to a catalog entry via `typeCodes`. 6-7 digit numbers are
/// treated as classic frame numbers and matched against `frameRanges`.
/// In both cases the deepest matching node wins (Modell > Serie > Familie).
pub async fn decode_vin(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Query(query): Query<VinQuery>,
) -> AppResult<Json<Value>> {
    let vin: String = query
        .vin
        .trim()
        .to_uppercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();

    // Classic frame numbers: pre-1981 bikes AND ECE-market bikes into the
    // 90s carry 6-8 digit serials (often with leading zeros), sometimes
    // stamped with the model designation appended — "0103596 K75S". Take the
    // leading digit run and tolerate a short alphanumeric suffix.
    let leading_digits: String = vin.chars().take_while(|c| c.is_ascii_digit()).collect();
    let suffix_len = vin.len() - leading_digits.len();
    if vin.len() != 17 && (6..=8).contains(&leading_digits.len()) && suffix_len <= 6 {
        let frame_number: u64 = leading_digits
            .parse()
            .map_err(|_| AppError::BadRequest("Invalid frame number".to_string()))?;
        let vin = leading_digits;

        let candidates = sqlx::query_as::<_, ModelSeries>(
            "SELECT * FROM modelSeries WHERE (userId IS NULL OR userId = ?) \
             AND frameRanges IS NOT NULL",
        )
        .bind(user.id)
        .fetch_all(&pool)
        .await?;

        let matching: Vec<ModelSeries> = candidates
            .into_iter()
            .filter(|candidate| {
                candidate
                    .frame_ranges
                    .as_deref()
                    .map(parse_frame_ranges)
                    .unwrap_or_default()
                    .iter()
                    .any(|(start, end)| (*start..=*end).contains(&frame_number))
            })
            .collect();

        let matched = if matching.is_empty() {
            None
        } else {
            let tree = accessible_tree(&pool, user.id).await?;
            matching
                .into_iter()
                .max_by_key(|candidate| depth_of(candidate.id, &tree))
        };

        return Ok(Json(json!({
            "vin": vin,
            "kind": "frameNumber",
            "isBmw": matched.is_some(),
            "typeCode": Value::Null,
            "modelYear": Value::Null,
            "checkDigitValid": Value::Null,
            "match": matched,
        })));
    }

    if vin.len() != 17 || !vin.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(AppError::BadRequest(
            "Input must be a 17-character VIN or a 6-8 digit frame number".to_string(),
        ));
    }
    let chars: Vec<char> = vin.chars().collect();
    let is_bmw = vin.starts_with("WB");
    let type_code: String = chars[3..7].iter().collect();
    let model_year = decode_model_year(chars[9]);
    let check_ok = check_digit_valid(&chars);

    let mut matched: Option<ModelSeries> = None;
    if is_bmw {
        let candidates = sqlx::query_as::<_, ModelSeries>(
            "SELECT * FROM modelSeries WHERE (userId IS NULL OR userId = ?) \
             AND typeCodes IS NOT NULL \
             AND (',' || REPLACE(typeCodes, ' ', '') || ',') LIKE ?",
        )
        .bind(user.id)
        .bind(format!("%,{},%", type_code))
        .fetch_all(&pool)
        .await?;

        if !candidates.is_empty() {
            let tree = accessible_tree(&pool, user.id).await?;
            matched = candidates
                .into_iter()
                .max_by_key(|candidate| depth_of(candidate.id, &tree));
        }
    }

    Ok(Json(json!({
        "vin": vin,
        "kind": "vin",
        "isBmw": is_bmw,
        "typeCode": type_code,
        "modelYear": model_year,
        "checkDigitValid": check_ok,
        "match": matched,
    })))
}

pub async fn delete_model_series(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(sid): Path<i64>,
) -> AppResult<Json<Value>> {
    // Users curate the catalog: global seed entries are deletable too (the
    // guards below keep anything in use safe). Foreign users' custom entries
    // stay masked as NotFound.
    let deletable: i64 = sqlx::query(
        "SELECT COUNT(*) as cnt FROM modelSeries WHERE id = ? AND (userId IS NULL OR userId = ?)",
    )
    .bind(sid)
    .bind(user.id)
    .fetch_one(&pool)
    .await?
    .get("cnt");
    if deletable == 0 {
        return Err(AppError::NotFound("Model series not found".to_string()));
    }

    // Deleting cascades through the subtree (a family takes its unused series
    // and models with it), but refuses when anything in the subtree is still
    // in use — by any user's parts or motorcycles — or belongs to another user.
    let rows = sqlx::query("SELECT id, parentId, userId FROM modelSeries")
        .fetch_all(&pool)
        .await?;
    let all: Vec<(i64, Option<i64>, Option<i64>)> = rows
        .iter()
        .map(|row| {
            (
                row.get::<i64, _>("id"),
                row.get::<Option<i64>, _>("parentId"),
                row.get::<Option<i64>, _>("userId"),
            )
        })
        .collect();

    // Subtree ids with their depth below the deleted node (0 = the node).
    let mut subtree: Vec<(i64, usize)> = vec![(sid, 0)];
    let mut frontier = vec![sid];
    for depth in 1..=MAX_DEPTH * 2 {
        let next: Vec<i64> = all
            .iter()
            .filter(|(_, parent, _)| parent.map(|p| frontier.contains(&p)) == Some(true))
            .map(|(id, _, _)| *id)
            .filter(|id| !subtree.iter().any(|(seen, _)| seen == id))
            .collect();
        if next.is_empty() {
            break;
        }
        subtree.extend(next.iter().map(|id| (*id, depth)));
        frontier = next;
    }

    for (id, _) in &subtree {
        let owner = all
            .iter()
            .find(|(node_id, _, _)| node_id == id)
            .and_then(|(_, _, owner)| *owner);
        if owner.is_some() && owner != Some(user.id) {
            return Err(AppError::BadRequest(
                "Model series contains entries of other users".to_string(),
            ));
        }
        let part_refs: i64 =
            sqlx::query("SELECT COUNT(*) as cnt FROM partSeriesCompat WHERE seriesId = ?")
                .bind(id)
                .fetch_one(&pool)
                .await?
                .get("cnt");
        let moto_refs: i64 =
            sqlx::query("SELECT COUNT(*) as cnt FROM motorcycles WHERE seriesId = ?")
                .bind(id)
                .fetch_one(&pool)
                .await?
                .get("cnt");
        if part_refs > 0 || moto_refs > 0 {
            return Err(AppError::BadRequest(
                "Model series is still referenced by parts or motorcycles".to_string(),
            ));
        }
    }

    // Leaves first: the self-referencing parentId FK forbids removing a parent
    // while children still point at it.
    subtree.sort_by(|a, b| b.1.cmp(&a.1));
    let mut tx = pool.begin().await?;
    for (id, _) in &subtree {
        sqlx::query("DELETE FROM modelSeries WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    Ok(Json(json!({ "message": "Model series deleted" })))
}
