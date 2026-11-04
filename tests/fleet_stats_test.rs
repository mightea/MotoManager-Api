use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use moto_manager_api::{
    auth::{password::hash_password, session::create_session},
    build_app,
    config::Config,
    AppState,
};
use serde_json::Value;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

async fn setup_test_app() -> (axum::Router, sqlx::SqlitePool, String) {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let config = Config {
        database_url: "sqlite::memory:".to_string(),
        port: 3001,
        rp_id: "localhost".to_string(),
        rp_name: "Test".to_string(),
        origin: "http://localhost:5173".to_string(),
        enable_registration: true,
        app_version: "test".to_string(),
        data_dir: "./test_data".to_string(),
        cache_dir: "./cache".to_string(),
        llm_base_url: None,
        llm_model: "test".to_string(),
        llm_api_key: "test".to_string(),
        backup_enabled: false,
        backup_interval_hours: 24,
        backup_keep: 14,
        frontend_version: None,
        mcp_allowed_hosts: Vec::new(),
        public_url: "http://localhost:3001".to_string(),
    };

    let rp_origin = url::Url::parse("http://localhost:5173").unwrap();
    let builder = webauthn_rs::WebauthnBuilder::new("localhost", &rp_origin).unwrap();
    let webauthn = std::sync::Arc::new(builder.build().unwrap());

    let state = AppState {
        pool: pool.clone(),
        config,
        webauthn,
        backup_lock: std::sync::Arc::new(tokio::sync::Mutex::new(())),
    };

    let password_hash = hash_password("password123").unwrap();
    let user_id = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("test@example.com")
    .bind("testuser")
    .bind("Test User")
    .bind(password_hash)
    .bind("user")
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    let token = create_session(&pool, user_id).await.unwrap();

    (build_app(state), pool, token)
}

fn auth(req: Request<Body>, token: &str) -> Request<Body> {
    let (mut parts, body) = req.into_parts();
    parts.headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", token).parse().unwrap(),
    );
    Request::from_parts(parts, body)
}

async fn insert_user(pool: &sqlx::SqlitePool, username: &str) -> i64 {
    sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(format!("{username}@example.com"))
    .bind(username)
    .bind(username)
    .bind(hash_password("password123").unwrap())
    .bind("user")
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn get_fleet(app: &axum::Router, token: &str) -> Value {
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .uri("/api/stats/fleet")
                .body(Body::empty())
                .unwrap(),
            token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn fleet_stats_requires_auth() {
    let (app, _pool, _token) = setup_test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/stats/fleet")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn fleet_stats_aggregates_costs_and_ownership() {
    let (app, pool, token) = setup_test_app().await;
    let user_id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'testuser'")
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO currencies (code, symbol, conversionFactor, createdAt) \
         VALUES ('TST', 'T', 0.5, '2026-01-01')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Bought for 10000 TST (= 5000), sold for 6000 (base currency).
    let bike = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo, purchaseDate, purchasePrice, \
         currencyCode, status, soldDate, salePrice) \
         VALUES ('BMW', 'R 80', ?, 10000, '2020-01-01', 10000, 'TST', 'sold', '2021-01-01', 6000)",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    for (date, odo, kind, cost) in [
        ("2019-06-01", 5000, "repair", 777.0), // previous owner — ignored
        ("2020-07-01", 15000, "service", 300.0),
        ("2020-12-31", 20000, "tire", 200.0),
    ] {
        sqlx::query(
            "INSERT INTO maintenanceRecords (date, odo, motorcycleId, type, cost) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(date)
        .bind(odo)
        .bind(bike)
        .bind(kind)
        .bind(cost)
        .execute(&pool)
        .await
        .unwrap();
    }

    // Insurance of 200 TST (= 100) linked to the bike.
    let expense = sqlx::query(
        "INSERT INTO expenses (userId, date, amount, currency, category) \
         VALUES (?, '2020-03-01', 200, 'TST', 'Versicherung')",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    sqlx::query("INSERT INTO expenseMotorcycles (expenseId, motorcycleId) VALUES (?, ?)")
        .bind(expense)
        .bind(bike)
        .execute(&pool)
        .await
        .unwrap();

    // Another user's data must not leak in.
    let other = insert_user(&pool, "other").await;
    let other_bike = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES ('BMW', 'X', ?, 0)",
    )
    .bind(other)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    sqlx::query(
        "INSERT INTO maintenanceRecords (date, odo, motorcycleId, type, cost) \
         VALUES ('2020-05-01', 9999, ?, 'repair', 5000)",
    )
    .bind(other_bike)
    .execute(&pool)
    .await
    .unwrap();

    let stats = get_fleet(&app, &token).await;

    assert_eq!(stats["motorcycles"].as_array().unwrap().len(), 1);
    let m = &stats["motorcycles"][0];
    assert_eq!(m["distance"], 10000);
    assert_eq!(m["runningCost"], 600.0);
    assert_eq!(m["purchasePrice"], 5000.0);
    assert_eq!(m["salePrice"], 6000.0);
    assert_eq!(m["depreciation"], -1000.0);
    assert_eq!(m["netCost"], -400.0);
    assert_eq!(m["costs"]["fixed"], 100.0);
    assert_eq!(m["ownedUntil"], "2021-01-01");

    let years = stats["years"].as_array().unwrap();
    assert_eq!(years[0]["year"], 2020);
    assert_eq!(years[0]["distance"], 10000);
    assert_eq!(years[0]["costs"]["service"], 300.0);
    assert_eq!(years[0]["costs"]["tires"], 200.0);
    assert_eq!(years[0]["totalCost"], 600.0);
    assert_eq!(years[0]["motorcycleCount"], 1);
    // Sold in 2021: counted in 2021, gone from 2022 on.
    assert_eq!(years[1]["motorcycleCount"], 1);
    assert_eq!(years[2]["motorcycleCount"], 0);
    assert_eq!(stats["totals"]["distance"], 10000);
}

#[tokio::test]
async fn old_stats_endpoint_is_unchanged() {
    let (app, _pool, token) = setup_test_app().await;
    let response = app
        .oneshot(auth(
            Request::builder()
                .uri("/api/stats")
                .body(Body::empty())
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert!(v["stats"]["yearly"].is_array());
    assert!(v["stats"]["overall"].is_object());
}
