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
    };

    let rp_origin = url::Url::parse("http://localhost:5173").unwrap();
    let builder = webauthn_rs::WebauthnBuilder::new("localhost", &rp_origin).unwrap();
    let webauthn = std::sync::Arc::new(builder.build().unwrap());

    let state = AppState {
        pool: pool.clone(),
        config,
        webauthn,
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
