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
use serde_json::{json, Value};
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

    // Create a test user
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

    // Create a session
    let token = create_session(&pool, user_id).await.unwrap();

    (build_app(state), pool, token)
}

#[tokio::test]
async fn test_expense_lifecycle() {
    let (app, pool, token) = setup_test_app().await;

    // Seed motorcycles
    let m1_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("Honda")
    .bind("CBR")
    .bind(1)
    .bind(0)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let m2_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("Yamaha")
    .bind("R1")
    .bind(1)
    .bind(0)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    // 1. Create expense
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/expenses")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "date": "2026-04-12",
                        "amount": 500.0,
                        "currency": "CHF",
                        "category": "Versicherung",
                        "description": "Flottenversicherung",
                        "motorcycleIds": [m1_id, m2_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    let expense_id = body["expense"]["id"].as_i64().unwrap();

    // 2. List expenses
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/expenses")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["expenses"].as_array().unwrap().len(), 1);
    assert_eq!(
        body["expenses"][0]["motorcycleIds"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    // 3. Update expense
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/expenses/{}", expense_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "amount": 600.0,
                        "motorcycleIds": [m1_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 4. Delete expense
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/expenses/{}", expense_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

/// Expenses may only be attributed to the caller's own motorcycles — an id
/// belonging to another user is rejected on create and update.
#[tokio::test]
async fn test_expense_rejects_foreign_motorcycle_ids() {
    let (app, pool, token) = setup_test_app().await;

    let other_user = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("other@example.com")
    .bind("other")
    .bind("Other")
    .bind(hash_password("password123").unwrap())
    .bind("user")
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let foreign_bike = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("Ducati")
    .bind("Monster")
    .bind(other_user)
    .bind(0)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/expenses")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "date": "2026-01-01",
                        "amount": 10.0,
                        "currency": "CHF",
                        "category": "Steuern",
                        "motorcycleIds": [foreign_bike]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM expenses")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);

    // Create a legitimate expense, then try to re-point it at the foreign bike.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/expenses")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "date": "2026-01-01",
                        "amount": 10.0,
                        "currency": "CHF",
                        "category": "Steuern",
                        "motorcycleIds": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: Value = serde_json::from_slice(&bytes).unwrap();
    let expense_id = created["expense"]["id"].as_i64().unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/expenses/{expense_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "motorcycleIds": [foreign_bike] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let linked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM expenseMotorcycles")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(linked, 0);
}
