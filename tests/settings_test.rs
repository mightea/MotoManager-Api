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

async fn setup_test_app() -> (axum::Router, String) {
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
    let user_id = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("alice@example.com")
    .bind("alice")
    .bind("Alice")
    .bind(password_hash)
    .bind("user")
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let token = create_session(&pool, user_id).await.unwrap();
    (build_app(state), token)
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_settings_final_drive_gearbox_oil_interval_roundtrip() {
    let (app, token) = setup_test_app().await;

    // Defaults: the new interval seeds to 2 years, km empty.
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/settings")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = read_json(r).await;
    assert_eq!(body["settings"]["finalDriveGearboxOilInterval"], 2);
    assert!(body["settings"]["finalDriveGearboxOilKmInterval"].is_null());
    // Minimum km/year seeds to 150.
    assert_eq!(body["settings"]["minKmPerYear"], 150);

    // Update the new field (plus a neighbour to prove bind order is intact).
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/settings")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "finalDriveGearboxOilInterval": 5,
                        "finalDriveGearboxOilKmInterval": 40000,
                        "engineOilInterval": 3,
                        "minKmPerYear": 500,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = read_json(r).await;
    assert_eq!(body["settings"]["finalDriveGearboxOilInterval"], 5);
    assert_eq!(body["settings"]["finalDriveGearboxOilKmInterval"], 40000);
    assert_eq!(body["settings"]["engineOilInterval"], 3);
    assert_eq!(body["settings"]["minKmPerYear"], 500);
    // Neighbouring columns kept their defaults — no bind-order bleed.
    assert_eq!(body["settings"]["finalDriveOilInterval"], 2);
    assert_eq!(body["settings"]["forkOilInterval"], 4);
}
