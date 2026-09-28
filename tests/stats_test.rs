use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use chrono::Datelike;
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

async fn insert_motorcycle(pool: &sqlx::SqlitePool, user_id: i64) {
    sqlx::query("INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)")
        .bind("BMW")
        .bind("R1250GS")
        .bind(user_id)
        .bind(1000)
        .execute(pool)
        .await
        .unwrap();
}

async fn get_stats(app: axum::Router, token: &str) -> Value {
    let response = app
        .oneshot(auth(
            Request::builder()
                .uri("/api/stats")
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

/// The webapp's server-stats page reads `avgMotoPerUser`/`avgDocsPerUser`
/// from the response root; they must exist and reflect the global ratios,
/// including motorcycles and documents owned by other users.
#[tokio::test]
async fn test_per_user_averages_cover_all_users() {
    let (app, pool, token) = setup_test_app().await;

    let other = insert_user(&pool, "second").await;
    // 3 motorcycles across 2 users -> 1.5 each.
    insert_motorcycle(&pool, 1).await;
    insert_motorcycle(&pool, other).await;
    insert_motorcycle(&pool, other).await;
    // 1 document across 2 users -> 0.5 each.
    sqlx::query("INSERT INTO documents (title, filePath, ownerId) VALUES (?, ?, ?)")
        .bind("Fahrzeugausweis")
        .bind("doc.pdf")
        .bind(other)
        .execute(&pool)
        .await
        .unwrap();

    let body = get_stats(app, &token).await;

    assert_eq!(body["stats"]["global"]["users"], 2);
    assert_eq!(body["stats"]["global"]["motorcycles"], 3);
    assert_eq!(body["stats"]["global"]["documents"], 1);
    assert_eq!(body["avgMotoPerUser"], 1.5);
    assert_eq!(body["avgDocsPerUser"], 0.5);
}

#[tokio::test]
async fn test_per_user_averages_zero_without_data() {
    let (app, _pool, token) = setup_test_app().await;

    let body = get_stats(app, &token).await;

    assert_eq!(body["stats"]["global"]["users"], 1);
    assert_eq!(body["avgMotoPerUser"], 0.0);
    assert_eq!(body["avgDocsPerUser"], 0.0);
}

async fn insert_bike_bought(pool: &sqlx::SqlitePool, purchase_date: &str, initial_odo: i64) -> i64 {
    sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo, purchaseDate) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("BMW")
    .bind("R80GS")
    .bind(1)
    .bind(initial_odo)
    .bind(purchase_date)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn insert_service(pool: &sqlx::SqlitePool, motorcycle_id: i64, date: &str, odo: i64) {
    sqlx::query(
        "INSERT INTO maintenanceRecords (motorcycleId, type, date, odo) VALUES (?, 'service', ?, ?)",
    )
    .bind(motorcycle_id)
    .bind(date)
    .bind(odo)
    .execute(pool)
    .await
    .unwrap();
}

fn year_distance(body: &Value, year: i64) -> i64 {
    body["stats"]["yearly"]
        .as_array()
        .unwrap()
        .iter()
        .find(|y| y["year"] == year)
        .map(|y| y["distance"].as_i64().unwrap())
        .unwrap_or_else(|| panic!("no yearly entry for {year}"))
}

/// Services logged by a previous owner earlier in the purchase year sit below
/// the odometer at purchase; they must not pull that year negative (#154).
#[tokio::test]
async fn test_yearly_distance_ignores_records_before_purchase() {
    let (app, pool, token) = setup_test_app().await;

    let bike = insert_bike_bought(&pool, "2022-06-01", 12_000).await;
    insert_service(&pool, bike, "2022-02-01", 10_000).await; // previous owner
    insert_service(&pool, bike, "2023-05-01", 15_000).await;

    let body = get_stats(app, &token).await;

    assert_eq!(year_distance(&body, 2022), 0);
    assert_eq!(year_distance(&body, 2023), 3_000);
    assert_eq!(body["stats"]["overall"]["totalDistance"], 3_000);
}

/// An odometer reading lower than an earlier year's (typo, late entry) must
/// not produce a negative year or inflate the following one.
#[tokio::test]
async fn test_yearly_distance_never_negative_for_lower_reading() {
    let (app, pool, token) = setup_test_app().await;

    let bike = insert_bike_bought(&pool, "2021-01-01", 10_000).await;
    insert_service(&pool, bike, "2022-06-01", 15_000).await;
    insert_service(&pool, bike, "2023-06-01", 14_000).await;
    insert_service(&pool, bike, "2024-06-01", 16_000).await;

    let body = get_stats(app, &token).await;

    assert_eq!(year_distance(&body, 2022), 5_000);
    assert_eq!(year_distance(&body, 2023), 0);
    assert_eq!(year_distance(&body, 2024), 1_000);
    let years = body["stats"]["yearly"].as_array().unwrap();
    assert!(years.iter().all(|y| y["distance"].as_i64().unwrap() >= 0));
}

/// A bike bought this year whose only reading this year predates the
/// purchase has not been ridden yet — in the stats and on the dashboard.
#[tokio::test]
async fn test_km_this_year_ignores_previous_owner_records() {
    let (app, pool, token) = setup_test_app().await;
    let year = chrono::Utc::now().year();

    let fresh = insert_bike_bought(&pool, &format!("{year}-01-02"), 12_000).await;
    insert_service(&pool, fresh, &format!("{year}-01-01"), 10_000).await;

    let ridden = insert_bike_bought(&pool, &format!("{year}-01-02"), 12_000).await;
    insert_service(&pool, ridden, &format!("{}-06-01", year - 1), 10_000).await;
    insert_service(&pool, ridden, &format!("{year}-01-03"), 13_000).await;

    let body = get_stats(app.clone(), &token).await;
    let this_year = |body: &Value, id: i64| {
        body["motorcycles"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == id)
            .unwrap()["odometerThisYear"]
            .as_i64()
            .unwrap()
    };
    assert_eq!(this_year(&body, fresh), 0);
    assert_eq!(this_year(&body, ridden), 1_000);
    assert_eq!(body["stats"]["totalKmThisYear"], 1_000);

    let response = app
        .oneshot(auth(
            Request::builder()
                .uri("/api/home")
                .body(Body::empty())
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let home: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(this_year(&home, fresh), 0);
    assert_eq!(this_year(&home, ridden), 1_000);
    assert_eq!(home["stats"]["totalKmThisYear"], 1_000);
}
