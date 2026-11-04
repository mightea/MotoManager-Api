//! Migration 058 corrects fuel records whose foreign-currency amount was
//! multiplied by Road Trip's rate instead of divided. It must only touch rows
//! that still carry the wrong value, and be safe to run again.

use sqlx::sqlite::SqlitePoolOptions;

const FIX_SQL: &str = include_str!("../migrations/058_fix_roadtrip_currency_conversion.sql");

async fn insert_fuel(
    pool: &sqlx::SqlitePool,
    bike: i64,
    date: &str,
    odo: i64,
    cost: f64,
    currency: &str,
    normalized: f64,
) -> i64 {
    sqlx::query(
        "INSERT INTO maintenanceRecords (date, odo, motorcycleId, type, cost, currency, normalizedCost, updatedAt) \
         VALUES (?, ?, ?, 'fuel', ?, ?, ?, '2026-01-01T00:00:00.000Z')",
    )
    .bind(date)
    .bind(odo)
    .bind(bike)
    .bind(cost)
    .bind(currency)
    .bind(normalized)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn normalized(pool: &sqlx::SqlitePool, id: i64) -> (f64, String) {
    sqlx::query_as("SELECT normalizedCost, updatedAt FROM maintenanceRecords WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn fixes_only_wrongly_converted_roadtrip_fuel() {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let user = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES ('a@b.c', 'u', 'U', 'x', 'user')",
    )
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let bike = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES ('BMW', 'R 100', ?, 0)",
    )
    .bind(user)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    // SEK 231.02 stored as 231.02 × 7.080938 by the old import.
    let wrong = insert_fuel(
        &pool,
        bike,
        "2010-07-10",
        125832,
        231.02,
        "SEK",
        231.02 * 7.080938,
    )
    .await;
    // Same trip, already corrected by hand — must stay as it is.
    let corrected = insert_fuel(&pool, bike, "2010-07-14", 126129, 222.91, "SEK", 31.75).await;
    // Unrelated foreign record entered in the app.
    let other = insert_fuel(&pool, bike, "2019-03-03", 1337, 200.0, "AUD", 105.78).await;

    sqlx::raw_sql(FIX_SQL).execute(&pool).await.unwrap();

    let (value, updated) = normalized(&pool, wrong).await;
    assert!((value - 32.625).abs() < 0.01, "got {value}");
    assert_ne!(updated, "2026-01-01T00:00:00.000Z");
    assert_eq!(
        normalized(&pool, corrected).await,
        (31.75, "2026-01-01T00:00:00.000Z".into())
    );
    assert_eq!(
        normalized(&pool, other).await,
        (105.78, "2026-01-01T00:00:00.000Z".into())
    );

    // Running it again changes nothing.
    sqlx::raw_sql(FIX_SQL).execute(&pool).await.unwrap();
    assert_eq!(normalized(&pool, wrong).await, (value, updated));
}
