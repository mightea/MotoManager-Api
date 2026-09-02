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

async fn body_json(response: axum::response::Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

fn post_detail(moto_id: i64, payload: Value) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(format!("/api/motorcycles/{}/details", moto_id))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap()
}

#[tokio::test]
async fn test_detail_crud_roundtrip() {
    let (app, _pool, token, moto_id) = setup_test_app().await;

    // Create.
    let response = app
        .clone()
        .oneshot(auth(
            post_detail(
                moto_id,
                json!({ "title": "Zündkerzen", "value": "NGK BP7ES" }),
            ),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = body_json(response).await;
    let detail_id = body["motorcycleDetail"]["id"].as_i64().unwrap();
    assert_eq!(body["motorcycleDetail"]["title"], json!("Zündkerzen"));
    assert_eq!(body["motorcycleDetail"]["value"], json!("NGK BP7ES"));

    // Update the value only; the title must be preserved.
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::PUT)
                .uri(format!(
                    "/api/motorcycles/{}/details/{}",
                    moto_id, detail_id
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "value": "NGK BPR7ES" }).to_string()))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["motorcycleDetail"]["title"], json!("Zündkerzen"));
    assert_eq!(body["motorcycleDetail"]["value"], json!("NGK BPR7ES"));

    // List shows it.
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .uri(format!("/api/motorcycles/{}/details", moto_id))
                .body(Body::empty())
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let details = body["motorcycleDetails"].as_array().unwrap();
    assert_eq!(details.len(), 1);

    // Soft delete hides it from the plain list.
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::DELETE)
                .uri(format!(
                    "/api/motorcycles/{}/details/{}",
                    moto_id, detail_id
                ))
                .body(Body::empty())
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .uri(format!("/api/motorcycles/{}/details", moto_id))
                .body(Body::empty())
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    let body = body_json(response).await;
    assert_eq!(body["motorcycleDetails"].as_array().unwrap().len(), 0);

    // ...but the tombstone is visible through the ?since delta.
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .uri(format!("/api/motorcycles/{}/details?since=0", moto_id))
                .body(Body::empty())
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    let body = body_json(response).await;
    let details = body["motorcycleDetails"].as_array().unwrap();
    assert_eq!(details.len(), 1);
    assert!(!details[0]["deletedAt"].is_null());

    // Deleting again 404s.
    let response = app
        .oneshot(auth(
            Request::builder()
                .method(Method::DELETE)
                .uri(format!(
                    "/api/motorcycles/{}/details/{}",
                    moto_id, detail_id
                ))
                .body(Body::empty())
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_detail_client_id_idempotency() {
    let (app, _pool, token, moto_id) = setup_test_app().await;

    let payload = json!({
        "title": "Batterie",
        "value": "Gel 12V 19Ah",
        "clientId": "11111111-2222-3333-4444-555555555555"
    });

    let response = app
        .clone()
        .oneshot(auth(post_detail(moto_id, payload.clone()), &token))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let first_id = body_json(response).await["motorcycleDetail"]["id"]
        .as_i64()
        .unwrap();

    // Retried create with the same clientId returns the same row.
    let response = app
        .clone()
        .oneshot(auth(post_detail(moto_id, payload), &token))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let second_id = body_json(response).await["motorcycleDetail"]["id"]
        .as_i64()
        .unwrap();
    assert_eq!(first_id, second_id);

    let response = app
        .oneshot(auth(
            Request::builder()
                .uri(format!("/api/motorcycles/{}/details", moto_id))
                .body(Body::empty())
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    let body = body_json(response).await;
    assert_eq!(body["motorcycleDetails"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_detail_validation_and_ownership() {
    let (app, pool, token, moto_id) = setup_test_app().await;

    // Empty title / value rejected.
    let response = app
        .clone()
        .oneshot(auth(
            post_detail(moto_id, json!({ "title": " ", "value": "x" })),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = app
        .clone()
        .oneshot(auth(
            post_detail(moto_id, json!({ "title": "x", "value": "" })),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Another user cannot touch this motorcycle's details.
    let password_hash = hash_password("password456").unwrap();
    let other_user = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("other@example.com")
    .bind("otheruser")
    .bind("Other User")
    .bind(password_hash)
    .bind("user")
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let other_token = create_session(&pool, other_user).await.unwrap();

    let response = app
        .oneshot(auth(
            post_detail(moto_id, json!({ "title": "Hack", "value": "x" })),
            &other_token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
