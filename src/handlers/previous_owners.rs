use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

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
        "SELECT * FROM previousOwners WHERE motorcycleId = ? ORDER BY purchaseDate DESC, id DESC",
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
    pub purchase_date: String,
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

    let id = sqlx::query(
        "INSERT INTO previousOwners \
         (motorcycleId, name, surname, purchaseDate, address, city, postcode, country, \
          phoneNumber, email, comments, createdAt, updatedAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(motorcycle_id)
    .bind(&body.name)
    .bind(&body.surname)
    .bind(&body.purchase_date)
    .bind(&body.address)
    .bind(&body.city)
    .bind(&body.postcode)
    .bind(&body.country)
    .bind(&body.phone_number)
    .bind(&body.email)
    .bind(&body.comments)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await?
    .last_insert_rowid();

    let owner = sqlx::query_as::<_, PreviousOwner>("SELECT * FROM previousOwners WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok((StatusCode::CREATED, Json(json!({ "previousOwner": owner }))))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePreviousOwnerRequest {
    pub name: Option<String>,
    pub surname: Option<String>,
    pub purchase_date: Option<String>,
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
    let purchase_date = body.purchase_date.unwrap_or(existing.purchase_date);
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
