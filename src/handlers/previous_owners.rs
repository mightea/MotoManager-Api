use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Deserializer};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::HashSet;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::motorcycles::verify_motorcycle_ownership,
    models::PreviousOwner,
};

pub async fn list_previous_owners(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let owners = sqlx::query_as::<_, PreviousOwner>(
        "SELECT * FROM previousOwners WHERE motorcycleId = ? ORDER BY sortOrder ASC, id ASC",
    )
    .bind(motorcycle_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(json!({ "previousOwners": owners })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePreviousOwnerRequest {
    pub name: String,
    pub surname: String,
    pub purchase_date: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub postcode: Option<String>,
    pub country: Option<String>,
    pub phone_number: Option<String>,
    pub email: Option<String>,
    pub comments: Option<String>,
}

pub async fn create_previous_owner(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Json(body): Json<CreatePreviousOwnerRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let now = Utc::now().to_rfc3339();
    let purchase_date = normalize_optional_text(body.purchase_date);
    let mut tx = pool.begin().await?;
    let sort_order: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sortOrder), -1) + 1 FROM previousOwners WHERE motorcycleId = ?",
    )
    .bind(motorcycle_id)
    .fetch_one(&mut *tx)
    .await?;

    let id = sqlx::query(
        "INSERT INTO previousOwners \
         (motorcycleId, name, surname, purchaseDate, sortOrder, address, city, postcode, country, \
          phoneNumber, email, comments, createdAt, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(motorcycle_id)
    .bind(&body.name)
    .bind(&body.surname)
    .bind(&purchase_date)
    .bind(sort_order)
    .bind(&body.address)
    .bind(&body.city)
    .bind(&body.postcode)
    .bind(&body.country)
    .bind(&body.phone_number)
    .bind(&body.email)
    .bind(&body.comments)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    let owner = sqlx::query_as::<_, PreviousOwner>("SELECT * FROM previousOwners WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(json!({ "previousOwner": owner }))))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePreviousOwnerRequest {
    pub name: Option<String>,
    pub surname: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    purchase_date: NullableField<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub postcode: Option<String>,
    pub country: Option<String>,
    pub phone_number: Option<String>,
    pub email: Option<String>,
    pub comments: Option<String>,
}

pub async fn update_previous_owner(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((motorcycle_id, oid)): Path<(i64, i64)>,
    Json(body): Json<UpdatePreviousOwnerRequest>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let existing = sqlx::query_as::<_, PreviousOwner>(
        "SELECT * FROM previousOwners WHERE id = ? AND motorcycleId = ?",
    )
    .bind(oid)
    .bind(motorcycle_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Previous owner not found".to_string()))?;

    let name = body.name.unwrap_or(existing.name);
    let surname = body.surname.unwrap_or(existing.surname);
    let purchase_date = match body.purchase_date {
        NullableField::Missing => existing.purchase_date,
        NullableField::Null => None,
        NullableField::Value(value) => normalize_optional_text(Some(value)),
    };
    let address = body.address.or(existing.address);
    let city = body.city.or(existing.city);
    let postcode = body.postcode.or(existing.postcode);
    let country = body.country.or(existing.country);
    let phone_number = body.phone_number.or(existing.phone_number);
    let email = body.email.or(existing.email);
    let comments = body.comments.or(existing.comments);
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE previousOwners SET \
         name = ?, surname = ?, purchaseDate = ?, address = ?, city = ?, postcode = ?, \
         country = ?, phoneNumber = ?, email = ?, comments = ?, updatedAt = ? \
         WHERE id = ?",
    )
    .bind(&name)
    .bind(&surname)
    .bind(&purchase_date)
    .bind(&address)
    .bind(&city)
    .bind(&postcode)
    .bind(&country)
    .bind(&phone_number)
    .bind(&email)
    .bind(&comments)
    .bind(&now)
    .bind(oid)
    .execute(&pool)
    .await?;

    let owner = sqlx::query_as::<_, PreviousOwner>("SELECT * FROM previousOwners WHERE id = ?")
        .bind(oid)
        .fetch_one(&pool)
        .await?;

    Ok(Json(json!({ "previousOwner": owner })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderPreviousOwnersRequest {
    pub owner_ids: Vec<i64>,
}

/// Replace the complete previous-owner order in one transaction. Requiring the
/// exact ID set prevents a stale client from accidentally hiding or moving an
/// owner belonging to another motorcycle.
pub async fn reorder_previous_owners(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
    Json(body): Json<ReorderPreviousOwnersRequest>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let existing_ids: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM previousOwners WHERE motorcycleId = ?")
            .bind(motorcycle_id)
            .fetch_all(&pool)
            .await?;
    let requested_ids: HashSet<i64> = body.owner_ids.iter().copied().collect();
    let existing_id_set: HashSet<i64> = existing_ids.iter().copied().collect();

    if requested_ids.len() != body.owner_ids.len()
        || body.owner_ids.len() != existing_ids.len()
        || requested_ids != existing_id_set
    {
        return Err(AppError::BadRequest(
            "Owner order must contain every previous owner exactly once".to_string(),
        ));
    }

    let now = Utc::now().to_rfc3339();
    let mut tx = pool.begin().await?;
    for (sort_order, owner_id) in body.owner_ids.iter().enumerate() {
        sqlx::query(
            "UPDATE previousOwners SET sortOrder = ?, updatedAt = ? \
             WHERE id = ? AND motorcycleId = ?",
        )
        .bind(sort_order as i64)
        .bind(&now)
        .bind(owner_id)
        .bind(motorcycle_id)
        .execute(&mut *tx)
        .await?;
    }

    let owners = sqlx::query_as::<_, PreviousOwner>(
        "SELECT * FROM previousOwners WHERE motorcycleId = ? ORDER BY sortOrder ASC, id ASC",
    )
    .bind(motorcycle_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Json(json!({ "previousOwners": owners })))
}

pub async fn delete_previous_owner(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((motorcycle_id, oid)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let result = sqlx::query("DELETE FROM previousOwners WHERE id = ? AND motorcycleId = ?")
        .bind(oid)
        .bind(motorcycle_id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Previous owner not found".to_string()));
    }

    Ok(Json(json!({ "message": "Previous owner deleted" })))
}

#[derive(Debug, Default)]
enum NullableField<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

fn deserialize_nullable_field<'de, D, T>(deserializer: D) -> Result<NullableField<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(match Option::<T>::deserialize(deserializer)? {
        Some(value) => NullableField::Value(value),
        None => NullableField::Null,
    })
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
