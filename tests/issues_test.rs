use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
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

async fn setup_test_app() -> (axum::Router, sqlx::SqlitePool, String, i64) {
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

    let moto_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("BMW")
    .bind("R1250GS")
    .bind(user_id)
    .bind(1000)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    (build_app(state), pool, token, moto_id)
}

fn auth(req: Request<Body>, token: &str) -> Request<Body> {
    let (mut parts, body) = req.into_parts();
    parts.headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", token).parse().unwrap(),
    );
    Request::from_parts(parts, body)
}

#[tokio::test]
async fn test_create_requires_title() {
    let (app, _pool, token, moto_id) = setup_test_app().await;

    let response = app
        .oneshot(auth(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/motorcycles/{}/issues", moto_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "odo": 1200, "description": "details only" }).to_string(),
                ))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    // title is mandatory in the request body — missing → 422 (axum body
    // extraction failure).
    assert!(
        response.status() == StatusCode::UNPROCESSABLE_ENTITY
            || response.status() == StatusCode::BAD_REQUEST,
        "expected 4xx for missing title, got {}",
        response.status()
    );
}

#[tokio::test]
async fn test_create_rejects_blank_title() {
    let (app, _pool, token, moto_id) = setup_test_app().await;

    let response = app
        .oneshot(auth(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/motorcycles/{}/issues", moto_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "odo": 1200, "title": "   " }).to_string(),
                ))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_with_title_only_succeeds() {
    let (app, _pool, token, moto_id) = setup_test_app().await;

    let response = app
        .oneshot(auth(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/motorcycles/{}/issues", moto_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "odo": 1200, "title": "Ölverlust am Motor" }).to_string(),
                ))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["issue"]["title"], "Ölverlust am Motor");
    assert!(body["issue"]["description"].is_null());
}

#[tokio::test]
async fn test_create_with_title_and_description() {
    let (app, _pool, token, moto_id) = setup_test_app().await;

    let response = app
        .oneshot(auth(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/motorcycles/{}/issues", moto_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "odo": 1200,
                        "title": "Ölverlust",
                        "description": "Tropft am Ventildeckel rechts",
                        "priority": "high"
                    })
                    .to_string(),
                ))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["issue"]["title"], "Ölverlust");
    assert_eq!(
        body["issue"]["description"],
        "Tropft am Ventildeckel rechts"
    );
    assert_eq!(body["issue"]["priority"], "high");
}

#[tokio::test]
async fn test_update_preserves_title_when_omitted() {
    let (app, pool, token, moto_id) = setup_test_app().await;

    let issue_id = sqlx::query(
        "INSERT INTO issues (motorcycleId, odo, title, description, priority, status, date) \
         VALUES (?, ?, ?, ?, 'medium', 'new', '2026-01-01')",
    )
    .bind(moto_id)
    .bind(1100_i64)
    .bind("Original Titel")
    .bind(Some("Original Beschreibung"))
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    // PUT with only priority — title must remain.
    let response = app
        .oneshot(auth(
            Request::builder()
                .method(Method::PUT)
                .uri(format!("/api/motorcycles/{}/issues/{}", moto_id, issue_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "priority": "high" }).to_string()))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["issue"]["title"], "Original Titel");
    assert_eq!(body["issue"]["description"], "Original Beschreibung");
    assert_eq!(body["issue"]["priority"], "high");
}

#[tokio::test]
async fn test_update_can_clear_description() {
    let (app, pool, token, moto_id) = setup_test_app().await;

    let issue_id = sqlx::query(
        "INSERT INTO issues (motorcycleId, odo, title, description, priority, status, date) \
         VALUES (?, 1100, 'T', 'D', 'medium', 'new', '2026-01-01')",
    )
    .bind(moto_id)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    let response = app
        .oneshot(auth(
            Request::builder()
                .method(Method::PUT)
                .uri(format!("/api/motorcycles/{}/issues/{}", moto_id, issue_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "description": null }).to_string()))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert!(body["issue"]["description"].is_null());
    assert_eq!(body["issue"]["title"], "T");
}

#[tokio::test]
async fn test_update_rejects_blank_title() {
    let (app, pool, token, moto_id) = setup_test_app().await;

    let issue_id = sqlx::query(
        "INSERT INTO issues (motorcycleId, odo, title, priority, status, date) \
         VALUES (?, 1100, 'T', 'medium', 'new', '2026-01-01')",
    )
    .bind(moto_id)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    let response = app
        .oneshot(auth(
            Request::builder()
                .method(Method::PUT)
                .uri(format!("/api/motorcycles/{}/issues/{}", moto_id, issue_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "title": "" }).to_string()))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
