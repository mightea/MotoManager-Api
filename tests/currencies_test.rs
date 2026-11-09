use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use moto_manager_api::{
    auth::{password::hash_password, session::create_session},
    build_app,
    config::Config,
    currency_rates::{self, RefreshFailure},
    AppState,
};
use serde_json::{json, Value};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use tower::ServiceExt;

/// Returns the app, its pool and a session token per role: (admin, user).
async fn setup_test_app() -> (axum::Router, SqlitePool, String, String) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
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
    let webauthn = std::sync::Arc::new(
        webauthn_rs::WebauthnBuilder::new("localhost", &rp_origin)
            .unwrap()
            .build()
            .unwrap(),
    );
    let state = AppState {
        pool: pool.clone(),
        config,
        webauthn,
        backup_lock: std::sync::Arc::new(tokio::sync::Mutex::new(())),
    };

    let password_hash = hash_password("password123").unwrap();
    let mut tokens = Vec::new();
    for (email, username, role) in [
        ("admin@example.com", "admin", "admin"),
        ("alice@example.com", "alice", "user"),
    ] {
        let user_id = sqlx::query(
            "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(email)
        .bind(username)
        .bind(username)
        .bind(&password_hash)
        .bind(role)
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();
        tokens.push(create_session(&pool, user_id).await.unwrap());
    }
    let user_token = tokens.pop().unwrap();
    let admin_token = tokens.pop().unwrap();
    (build_app(state), pool, admin_token, user_token)
}

/// Inserts a currency directly (no network fetch) and returns its id.
async fn insert_currency(pool: &SqlitePool, code: &str, factor: f64, rate_at: Option<&str>) -> i64 {
    sqlx::query(
        "INSERT INTO currencies (code, symbol, conversionFactor, rateUpdatedAt) VALUES (?, ?, ?, ?)",
    )
    .bind(code)
    .bind(code)
    .bind(factor)
    .bind(rate_at)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn rate_row(pool: &SqlitePool, code: &str) -> (f64, Option<String>) {
    sqlx::query_as("SELECT conversionFactor, rateUpdatedAt FROM currencies WHERE code = ?")
        .bind(code)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn is_recent(ts: &str) -> bool {
    let ts = chrono::DateTime::parse_from_rfc3339(ts).expect("rateUpdatedAt is RFC 3339");
    (chrono::Utc::now() - ts.with_timezone(&chrono::Utc)).num_seconds() < 60
}

const OLD: &str = "2025-01-01T00:00:00Z";

#[tokio::test]
async fn migration_backfills_rate_updated_at_from_created_at() {
    let (_app, pool, _admin, _user) = setup_test_app().await;
    sqlx::query(
        "INSERT INTO currencies (code, symbol, conversionFactor, createdAt) \
         VALUES ('EUR', '€', 0.93, '2025-10-09 19:55:27'), \
                ('AUD', 'A$', 0.55, '2026-02-23T14:52:53.123+00:00')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Re-run the backfill as the migration does on an existing database.
    sqlx::raw_sql(
        include_str!("../migrations/061_currency_rate_updated_at.sql")
            .split_once("ADD COLUMN rateUpdatedAt TEXT;")
            .unwrap()
            .1,
    )
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        rate_row(&pool, "EUR").await.1.as_deref(),
        Some("2025-10-09T19:55:27Z")
    );
    assert_eq!(
        rate_row(&pool, "AUD").await.1.as_deref(),
        Some("2026-02-23T14:52:53Z")
    );
}

#[tokio::test]
async fn currency_json_carries_rate_updated_at() {
    let (app, pool, admin, _user) = setup_test_app().await;
    insert_currency(&pool, "EUR", 0.93, Some(OLD)).await;

    let r = app
        .oneshot(
            Request::builder()
                .uri("/api/currencies")
                .header(header::AUTHORIZATION, format!("Bearer {admin}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = read_json(r).await;
    let eur = body["currencies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["code"] == "EUR")
        .unwrap();
    assert_eq!(eur["rateUpdatedAt"], OLD);
    // Existing keys stay as they were.
    assert_eq!(eur["conversionFactor"], 0.93);
    assert!(eur["createdAt"].is_string());
}

#[tokio::test]
async fn editing_the_factor_stamps_rate_updated_at_but_renaming_does_not() {
    let (app, pool, admin, _user) = setup_test_app().await;
    let id = insert_currency(&pool, "EUR", 0.93, Some(OLD)).await;

    let put = |body: Value| {
        Request::builder()
            .method("PUT")
            .uri(format!("/api/admin/currencies/{id}"))
            .header(header::AUTHORIZATION, format!("Bearer {admin}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };

    // Old request shape without conversionFactor: label change only.
    let r = app
        .clone()
        .oneshot(put(json!({ "label": "Euro" })))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(read_json(r).await["currency"]["rateUpdatedAt"], OLD);

    // Same factor resubmitted (the webapp edit form always sends it).
    let r = app
        .clone()
        .oneshot(put(json!({ "symbol": "EUR", "conversionFactor": 0.93 })))
        .await
        .unwrap();
    assert_eq!(read_json(r).await["currency"]["rateUpdatedAt"], OLD);

    let r = app
        .clone()
        .oneshot(put(json!({ "conversionFactor": 0.95 })))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = read_json(r).await;
    assert_eq!(body["currency"]["conversionFactor"], 0.95);
    assert!(is_recent(
        body["currency"]["rateUpdatedAt"].as_str().unwrap()
    ));
}

#[tokio::test]
async fn refresh_updates_fetched_rates_and_leaves_failures_untouched() {
    let (_app, pool, _admin, _user) = setup_test_app().await;
    insert_currency(&pool, "EUR", 0.90, Some(OLD)).await;
    insert_currency(&pool, "AUD", 0.50, Some(OLD)).await;
    let (chf_factor, chf_at) = rate_row(&pool, "CHF").await;

    let report = currency_rates::refresh_all(&pool, |code: String| async move {
        match code.as_str() {
            // Frankfurter quotes from=CHF: 1.0937 EUR per CHF.
            "EUR" => Ok(currency_rates::factor_from_quote(1.0937).unwrap()),
            "CHF" => panic!("the base currency is never fetched"),
            _ => Err("rate source unreachable".to_string()),
        }
    })
    .await
    .unwrap();

    assert_eq!(report.updated, vec!["EUR".to_string()]);
    assert_eq!(
        report.failed,
        vec![RefreshFailure {
            code: "AUD".to_string(),
            error: "rate source unreachable".to_string()
        }]
    );

    let (eur_factor, eur_at) = rate_row(&pool, "EUR").await;
    // Stored as CHF per EUR: EUR 100 × factor ≈ CHF 91.
    assert!((eur_factor - 1.0 / 1.0937).abs() < 1e-9);
    assert_eq!((100.0 * eur_factor).round(), 91.0);
    assert!(is_recent(eur_at.as_deref().unwrap()));

    assert_eq!(rate_row(&pool, "AUD").await, (0.50, Some(OLD.to_string())));
    assert_eq!(rate_row(&pool, "CHF").await, (chf_factor, chf_at));
}

#[tokio::test]
async fn refresh_is_due_once_the_newest_rate_is_a_day_old() {
    let (_app, pool, _admin, _user) = setup_test_app().await;
    // Only the base currency: nothing to refresh.
    assert_eq!(
        currency_rates::seconds_until_due(&pool).await.unwrap(),
        None
    );

    insert_currency(&pool, "AUD", 0.5, None).await;
    assert_eq!(
        currency_rates::seconds_until_due(&pool).await.unwrap(),
        Some(0)
    );

    insert_currency(&pool, "EUR", 0.9, Some(OLD)).await;
    assert_eq!(
        currency_rates::seconds_until_due(&pool).await.unwrap(),
        Some(0)
    );

    let fresh = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
    sqlx::query("UPDATE currencies SET rateUpdatedAt = ? WHERE code = 'EUR'")
        .bind(&fresh)
        .execute(&pool)
        .await
        .unwrap();
    let due = currency_rates::seconds_until_due(&pool)
        .await
        .unwrap()
        .unwrap();
    assert!((22 * 3600..=23 * 3600).contains(&due), "due in {due}s");
}

#[test]
fn factor_is_the_reciprocal_of_the_chf_quote() {
    assert_eq!(currency_rates::factor_from_quote(2.0), Some(0.5));
    assert_eq!(currency_rates::factor_from_quote(0.0), None);
    assert_eq!(currency_rates::factor_from_quote(f64::NAN), None);
}

#[tokio::test]
async fn refresh_endpoint_is_admin_only() {
    let (app, _pool, _admin, user) = setup_test_app().await;
    for auth in [None, Some(user)] {
        let mut req = Request::builder()
            .method("POST")
            .uri("/api/admin/currencies/refresh");
        if let Some(token) = auth.as_ref() {
            req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let r = app
            .clone()
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(
            matches!(r.status(), StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN),
            "got {}",
            r.status()
        );
    }
}
