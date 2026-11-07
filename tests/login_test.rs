//! Login accepts the identifier with stray whitespace (iOS autofill/keyboard
//! suggestions can append a space that the user never sees).

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use moto_manager_api::{auth::password::hash_password, build_app, config::Config, AppState};
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

async fn setup_test_app() -> axum::Router {
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

    sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES ('alice@example.com', 'alice', 'Alice', ?, 'user')",
    )
    .bind(hash_password("password123").unwrap())
    .execute(&pool)
    .await
    .unwrap();
    build_app(state)
}

async fn login_status(app: &axum::Router, identifier: &str, password: &str) -> StatusCode {
    let body = serde_json::json!({ "identifier": identifier, "password": password });
    app.clone()
        .oneshot(
            Request::post("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .header("X-Forwarded-For", "127.0.0.1")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn login_trims_whitespace_around_the_identifier() {
    let app = setup_test_app().await;
    for identifier in ["alice", "alice ", " alice", "alice\n", "alice@example.com "] {
        assert_eq!(
            login_status(&app, identifier, "password123").await,
            StatusCode::OK,
            "identifier {identifier:?}"
        );
    }
}

#[tokio::test]
async fn login_still_rejects_wrong_credentials() {
    let app = setup_test_app().await;
    assert_eq!(
        login_status(&app, "alice", "wrong").await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login_status(&app, "bob", "password123").await,
        StatusCode::UNAUTHORIZED
    );
    // The password is not trimmed: a space may be part of it.
    assert_eq!(
        login_status(&app, "alice", "password123 ").await,
        StatusCode::UNAUTHORIZED
    );
}
