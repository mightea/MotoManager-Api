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

/// Returns the app plus a session token per role: (app, admin_token, user_token).
async fn setup_test_app() -> (axum::Router, String, String) {
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
    (build_app(state), admin_token, user_token)
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn public_get() -> Request<Body> {
    Request::builder()
        .uri("/api/app-upgrade")
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn public_endpoint_defaults_to_zero_without_auth() {
    let (app, _admin_token, _user_token) = setup_test_app().await;

    // No Authorization header at all — the route must stay reachable for
    // logged-out and hard-blocked clients.
    let r = app.oneshot(public_get()).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = read_json(r).await;
    assert_eq!(body["softUpgradeBuild"], 0);
    assert_eq!(body["hardUpgradeBuild"], 0);
}

#[tokio::test]
async fn admin_can_update_builds_and_public_route_reflects_them() {
    let (app, admin_token, _user_token) = setup_test_app().await;

    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/admin/app-upgrade")
                .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "softUpgradeBuild": 1001, "hardUpgradeBuild": 901 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = read_json(r).await;
    assert_eq!(body["appUpgrade"]["softUpgradeBuild"], 1001);
    assert_eq!(body["appUpgrade"]["hardUpgradeBuild"], 901);

    let r = app.clone().oneshot(public_get()).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = read_json(r).await;
    assert_eq!(body["softUpgradeBuild"], 1001);
    assert_eq!(body["hardUpgradeBuild"], 901);

    // Partial update: an omitted field keeps its stored value.
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/admin/app-upgrade")
                .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "hardUpgradeBuild": 1001 }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = read_json(r).await;
    assert_eq!(body["appUpgrade"]["softUpgradeBuild"], 1001);
    assert_eq!(body["appUpgrade"]["hardUpgradeBuild"], 1001);
}

#[tokio::test]
async fn admin_read_endpoint_returns_full_record() {
    let (app, admin_token, _user_token) = setup_test_app().await;

    let r = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/app-upgrade")
                .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = read_json(r).await;
    assert_eq!(body["appUpgrade"]["softUpgradeBuild"], 0);
    assert_eq!(body["appUpgrade"]["hardUpgradeBuild"], 0);
    assert!(body["appUpgrade"]["updatedAt"].is_string());
}

#[tokio::test]
async fn non_admin_cannot_read_or_update_admin_routes() {
    let (app, _admin_token, user_token) = setup_test_app().await;

    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/admin/app-upgrade")
                .header(header::AUTHORIZATION, format!("Bearer {}", user_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);

    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/admin/app-upgrade")
                .header(header::AUTHORIZATION, format!("Bearer {}", user_token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "softUpgradeBuild": 5 }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn negative_build_numbers_are_rejected() {
    let (app, admin_token, _user_token) = setup_test_app().await;

    let r = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/admin/app-upgrade")
                .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "softUpgradeBuild": -1 }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
}
