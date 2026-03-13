use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::motorcycles::verify_motorcycle_ownership,
};

fn row_to_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": r.get::<i64, _>("id"),
        "motorcycleId": r.get::<i64, _>("motorcycle_id"),
        "name": r.get::<String, _>("name"),
        "surname": r.get::<String, _>("surname"),
        "purchaseDate": r.get::<String, _>("purchase_date"),
        "address": r.get::<Option<String>, _>("address"),
        "city": r.get::<Option<String>, _>("city"),
        "postcode": r.get::<Option<String>, _>("postcode"),
        "country": r.get::<Option<String>, _>("country"),
        "phoneNumber": r.get::<Option<String>, _>("phone_number"),
        "email": r.get::<Option<String>, _>("email"),
        "comments": r.get::<Option<String>, _>("comments"),
        "createdAt": r.get::<String, _>("created_at"),
        "updatedAt": r.get::<String, _>("updated_at"),
    })
}

const SELECT_COLS: &str =
    "id, motorcycle_id, name, surname, purchase_date, address, city, postcode, \
     country, phone_number, email, comments, created_at, updated_at";

pub async fn list_previous_owners(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(motorcycle_id): Path<i64>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let rows = sqlx::query(&format!(
        "SELECT {} FROM previous_owners WHERE motorcycle_id = ? ORDER BY purchase_date DESC, id DESC",
        SELECT_COLS
    ))
    .bind(motorcycle_id)
    .fetch_all(&pool)
    .await?;

    let owners: Vec<Value> = rows.iter().map(row_to_value).collect();
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
        "INSERT INTO previous_owners \
         (motorcycle_id, name, surname, purchase_date, address, city, postcode, country, \
          phone_number, email, comments, created_at, updated_at) \
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

    let row = sqlx::query(&format!(
        "SELECT {} FROM previous_owners WHERE id = ?",
        SELECT_COLS
    ))
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "previousOwner": row_to_value(&row) })),
    ))
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

    let existing = sqlx::query(&format!(
        "SELECT {} FROM previous_owners WHERE id = ? AND motorcycle_id = ?",
        SELECT_COLS
    ))
    .bind(oid)
    .bind(motorcycle_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Previous owner not found".to_string()))?;

    let name = body.name.unwrap_or_else(|| existing.get("name"));
    let surname = body.surname.unwrap_or_else(|| existing.get("surname"));
    let purchase_date = body
        .purchase_date
        .unwrap_or_else(|| existing.get("purchase_date"));
    let address: Option<String> = body.address.or_else(|| existing.get("address"));
    let city: Option<String> = body.city.or_else(|| existing.get("city"));
    let postcode: Option<String> = body.postcode.or_else(|| existing.get("postcode"));
    let country: Option<String> = body.country.or_else(|| existing.get("country"));
    let phone_number: Option<String> = body.phone_number.or_else(|| existing.get("phone_number"));
    let email: Option<String> = body.email.or_else(|| existing.get("email"));
    let comments: Option<String> = body.comments.or_else(|| existing.get("comments"));
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE previous_owners SET \
         name = ?, surname = ?, purchase_date = ?, address = ?, city = ?, postcode = ?, \
         country = ?, phone_number = ?, email = ?, comments = ?, updated_at = ? \
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

    let row = sqlx::query(&format!(
        "SELECT {} FROM previous_owners WHERE id = ?",
        SELECT_COLS
    ))
    .bind(oid)
    .fetch_one(&pool)
    .await?;

    Ok(Json(json!({ "previousOwner": row_to_value(&row) })))
}

pub async fn delete_previous_owner(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path((motorcycle_id, oid)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    verify_motorcycle_ownership(&pool, motorcycle_id, user.id).await?;

    let result =
        sqlx::query("DELETE FROM previous_owners WHERE id = ? AND motorcycle_id = ?")
            .bind(oid)
            .bind(motorcycle_id)
            .execute(&pool)
            .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Previous owner not found".to_string()));
    }

    Ok(Json(json!({ "message": "Previous owner deleted" })))
}
