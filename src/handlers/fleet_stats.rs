//! `GET /api/stats/fleet`: fleet statistics for the webapp's stats page.
//! All computation lives in `crate::fleet_analysis`; this module only loads
//! the user's rows (with money normalized to the base currency) and shapes
//! them into `FleetInput`.
//!
//! The older `GET /api/stats` stays as it is — the dashboard and the admin
//! server-stats page still read it.

use std::collections::HashMap;

use axum::{extract::State, Json};
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    auth::AuthUser,
    error::AppResult,
    fleet_analysis::{analyze, BikeInput, ExpenseInput, FleetInput, FleetStats, RecordInput},
};

#[derive(sqlx::FromRow)]
struct BikeRow {
    id: i64,
    make: String,
    model: String,
    status: String,
    #[sqlx(rename = "purchaseDate")]
    purchase_date: Option<String>,
    #[sqlx(rename = "soldDate")]
    sold_date: Option<String>,
    #[sqlx(rename = "initialOdo")]
    initial_odo: i64,
    #[sqlx(rename = "purchasePrice")]
    purchase_price: Option<f64>,
    #[sqlx(rename = "salePrice")]
    sale_price: Option<f64>,
}

#[derive(sqlx::FromRow)]
struct RecordRow {
    #[sqlx(rename = "motorcycleId")]
    motorcycle_id: i64,
    date: String,
    odo: i64,
    #[sqlx(rename = "type")]
    record_type: String,
    cost: Option<f64>,
    #[sqlx(rename = "partsCost")]
    parts_cost: Option<f64>,
    #[sqlx(rename = "fuelAmount")]
    fuel_amount: Option<f64>,
    #[sqlx(rename = "fuelConsumption")]
    fuel_consumption: Option<f64>,
}

#[derive(sqlx::FromRow)]
struct ExpenseRow {
    id: i64,
    date: String,
    amount: f64,
}

#[derive(sqlx::FromRow)]
struct ExpenseLinkRow {
    #[sqlx(rename = "expenseId")]
    expense_id: i64,
    #[sqlx(rename = "motorcycleId")]
    motorcycle_id: i64,
}

/// Load everything the analysis needs for one user and run it.
pub async fn compute_fleet_stats(pool: &SqlitePool, user_id: i64) -> AppResult<FleetStats> {
    // Prices: the stored normalized value, else converted with the currency's
    // factor (same rule as the parts inventory).
    let bikes = sqlx::query_as::<_, BikeRow>(
        "SELECT m.id, m.make, m.model, m.status, m.purchaseDate, m.soldDate, m.initialOdo, \
         COALESCE(m.normalizedPurchasePrice, m.purchasePrice * COALESCE(pc.conversionFactor, 1.0)) \
           AS purchasePrice, \
         COALESCE(m.normalizedSalePrice, m.salePrice * COALESCE(sc.conversionFactor, 1.0)) \
           AS salePrice \
         FROM motorcycles m \
         LEFT JOIN currencies pc ON pc.code = m.currencyCode \
         LEFT JOIN currencies sc ON sc.code = m.saleCurrencyCode \
         WHERE m.userId = ? ORDER BY m.id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let records = sqlx::query_as::<_, RecordRow>(
        "SELECT motorcycleId, date, odo, type, COALESCE(normalizedCost, cost) AS cost, \
         partsCost, fuelAmount, fuelConsumption \
         FROM maintenanceRecords \
         WHERE deletedAt IS NULL \
         AND motorcycleId IN (SELECT id FROM motorcycles WHERE userId = ?)",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let expenses = sqlx::query_as::<_, ExpenseRow>(
        "SELECT e.id, e.date, e.amount * COALESCE(c.conversionFactor, 1.0) AS amount \
         FROM expenses e LEFT JOIN currencies c ON c.code = e.currency \
         WHERE e.userId = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let links = sqlx::query_as::<_, ExpenseLinkRow>(
        "SELECT em.expenseId, em.motorcycleId FROM expenseMotorcycles em \
         JOIN expenses e ON e.id = em.expenseId WHERE e.userId = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let mut links_by_expense: HashMap<i64, Vec<i64>> = HashMap::new();
    for l in links {
        links_by_expense
            .entry(l.expense_id)
            .or_default()
            .push(l.motorcycle_id);
    }

    let input = FleetInput {
        today: Utc::now().date_naive(),
        bikes: bikes
            .into_iter()
            .map(|b| BikeInput {
                id: b.id,
                make: b.make,
                model: b.model,
                status: b.status,
                purchase_date: b.purchase_date.filter(|d| !d.is_empty()),
                sold_date: b.sold_date.filter(|d| !d.is_empty()),
                initial_odo: b.initial_odo,
                purchase_price: b.purchase_price,
                sale_price: b.sale_price,
            })
            .collect(),
        records: records
            .into_iter()
            .map(|r| RecordInput {
                motorcycle_id: r.motorcycle_id,
                date: r.date,
                odo: r.odo,
                record_type: r.record_type,
                cost: r.cost.unwrap_or(0.0),
                parts_cost: r.parts_cost.unwrap_or(0.0),
                fuel_amount: r.fuel_amount,
                fuel_consumption: r.fuel_consumption,
            })
            .collect(),
        expenses: expenses
            .into_iter()
            .map(|e| ExpenseInput {
                motorcycle_ids: links_by_expense.remove(&e.id).unwrap_or_default(),
                date: e.date,
                amount: e.amount,
            })
            .collect(),
    };
    Ok(analyze(input))
}

pub async fn get_fleet_stats(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let payload = compute_fleet_stats(&pool, user.id).await?;
    Ok(Json(json!(payload)))
}
