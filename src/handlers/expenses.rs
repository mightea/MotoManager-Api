use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::Expense,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseRequest {
    pub date: Option<String>,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub interval_months: Option<i64>,
    pub motorcycle_ids: Vec<i64>,
}

pub async fn list_expenses(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    tracing::debug!("Listing expenses for user: {}", user.id);

    // Fetch expenses and their associated motorcycle IDs
    let expenses = sqlx::query_as::<_, Expense>(
        "SELECT * FROM expenses WHERE userId = ? ORDER BY date DESC, id DESC",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    let mut result = Vec::new();
    for expense in expenses {
        let motorcycle_ids = sqlx::query!(
            "SELECT motorcycleId FROM expenseMotorcycles WHERE expenseId = ?",
            expense.id
        )
        .fetch_all(&pool)
        .await?
        .into_iter()
        .map(|r| r.motorcycleId)
        .collect::<Vec<_>>();

        let mut val = serde_json::to_value(&expense).unwrap();
        if let Some(obj) = val.as_object_mut() {
            obj.insert("motorcycleIds".to_string(), json!(motorcycle_ids));
        }
        result.push(val);
    }

    Ok(Json(json!({ "expenses": result })))
}

pub async fn create_expense(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Json(body): Json<ExpenseRequest>,
) -> AppResult<(StatusCode, Json<Value>)> {
    tracing::info!("Creating expense for user: {}", user.id);

    let date = body
        .date
        .ok_or_else(|| AppError::BadRequest("date is required".to_string()))?;
    let amount = body
        .amount
        .ok_or_else(|| AppError::BadRequest("amount is required".to_string()))?;
    let currency = body
        .currency
        .ok_or_else(|| AppError::BadRequest("currency is required".to_string()))?;
    let category = body
        .category
        .ok_or_else(|| AppError::BadRequest("category is required".to_string()))?;

    let mut tx = pool.begin().await?;

    let id = sqlx::query(
        "INSERT INTO expenses (userId, date, amount, currency, category, description, intervalMonths) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(user.id)
    .bind(&date)
    .bind(amount)
    .bind(&currency)
    .bind(&category)
    .bind(&body.description)
    .bind(body.interval_months)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    for mid in body.motorcycle_ids {
        sqlx::query("INSERT INTO expenseMotorcycles (expenseId, motorcycleId) VALUES (?, ?)")
            .bind(id)
            .bind(mid)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    let expense = sqlx::query_as::<_, Expense>("SELECT * FROM expenses WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok((StatusCode::CREATED, Json(json!({ "expense": expense }))))
}

pub async fn update_expense(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<ExpenseRequest>,
) -> AppResult<Json<Value>> {
    tracing::info!("Updating expense ID: {} for user: {}", id, user.id);

    let existing =
        sqlx::query_as::<_, Expense>("SELECT * FROM expenses WHERE id = ? AND userId = ?")
            .bind(id)
            .bind(user.id)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Expense not found".to_string()))?;

    let date = body.date.unwrap_or(existing.date);
    let amount = body.amount.unwrap_or(existing.amount);
    let currency = body.currency.unwrap_or(existing.currency);
    let category = body.category.unwrap_or(existing.category);
    let description = body.description.or(existing.description);
    let interval_months = body.interval_months.or(existing.interval_months);

    let mut tx = pool.begin().await?;

    sqlx::query(
        "UPDATE expenses SET date = ?, amount = ?, currency = ?, category = ?, description = ?, intervalMonths = ?, updatedAt = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(date)
    .bind(amount)
    .bind(currency)
    .bind(category)
    .bind(description)
    .bind(interval_months)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    // Update junction table
    sqlx::query("DELETE FROM expenseMotorcycles WHERE expenseId = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    for mid in body.motorcycle_ids {
        sqlx::query("INSERT INTO expenseMotorcycles (expenseId, motorcycleId) VALUES (?, ?)")
            .bind(id)
            .bind(mid)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    let expense = sqlx::query_as::<_, Expense>("SELECT * FROM expenses WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok(Json(json!({ "expense": expense })))
}

pub async fn delete_expense(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    tracing::info!("Deleting expense ID: {} for user: {}", id, user.id);

    let result = sqlx::query("DELETE FROM expenses WHERE id = ? AND userId = ?")
        .bind(id)
        .bind(user.id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Expense not found".to_string()));
    }

    Ok(Json(json!({ "message": "Expense deleted" })))
}
