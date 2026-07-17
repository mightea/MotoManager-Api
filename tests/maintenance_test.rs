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

#[tokio::test]
async fn test_fuel_additive_flags_roundtrip() {
    let (app, _pool, token, moto_id) = setup_test_app().await;

    // Create a fuel record with an additive but no lead substitute.
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/motorcycles/{}/maintenance", moto_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "date": "2026-07-17",
                        "odo": 1200,
                        "type": "fuel",
                        "fuelAmount": 14.5,
                        "fuelAdditiveAdded": true,
                        "leadSubstituteAdded": false
                    })
                    .to_string(),
                ))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = body_json(response).await;
    let record = &body["maintenanceRecord"];
    let record_id = record["id"].as_i64().unwrap();
    assert_eq!(record["fuelAdditiveAdded"], json!(true));
    assert_eq!(record["leadSubstituteAdded"], json!(false));

    // Toggle the lead substitute on via update.
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::PUT)
                .uri(format!(
                    "/api/motorcycles/{}/maintenance/{}",
                    moto_id, record_id
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "fuelAdditiveAdded": true,
                        "leadSubstituteAdded": true
                    })
                    .to_string(),
                ))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["maintenanceRecord"]["fuelAdditiveAdded"], json!(true));
    assert_eq!(
        body["maintenanceRecord"]["leadSubstituteAdded"],
        json!(true)
    );

    // An update that omits the flags must preserve them.
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::PUT)
                .uri(format!(
                    "/api/motorcycles/{}/maintenance/{}",
                    moto_id, record_id
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "odo": 1250 }).to_string()))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["maintenanceRecord"]["odo"], json!(1250));
    assert_eq!(body["maintenanceRecord"]["fuelAdditiveAdded"], json!(true));
    assert_eq!(
        body["maintenanceRecord"]["leadSubstituteAdded"],
        json!(true)
    );

    // The list endpoint surfaces the flags too.
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .uri(format!("/api/motorcycles/{}/maintenance", moto_id))
                .body(Body::empty())
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let records = body["maintenanceRecords"].as_array().unwrap();
    let listed = records
        .iter()
        .find(|r| r["id"].as_i64() == Some(record_id))
        .unwrap();
    assert_eq!(listed["fuelAdditiveAdded"], json!(true));
    assert_eq!(listed["leadSubstituteAdded"], json!(true));
}

#[tokio::test]
async fn test_fuel_additive_flags_default_false() {
    let (app, _pool, token, moto_id) = setup_test_app().await;

    // A create that never mentions the flags gets them as false, not null.
    let response = app
        .oneshot(auth(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/motorcycles/{}/maintenance", moto_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "date": "2026-07-17",
                        "odo": 1100,
                        "type": "fuel",
                        "fuelAmount": 12.0
                    })
                    .to_string(),
                ))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = body_json(response).await;
    assert_eq!(body["maintenanceRecord"]["fuelAdditiveAdded"], json!(false));
    assert_eq!(
        body["maintenanceRecord"]["leadSubstituteAdded"],
        json!(false)
    );
}
