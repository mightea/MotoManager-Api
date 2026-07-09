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
    };

    let rp_origin = url::Url::parse("http://localhost:5173").unwrap();
    let builder = webauthn_rs::WebauthnBuilder::new("localhost", &rp_origin).unwrap();
    let webauthn = std::sync::Arc::new(builder.build().unwrap());

    let state = AppState {
        pool: pool.clone(),
        config,
        webauthn,
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
async fn test_get_returns_null_when_not_recorded() {
    let (app, _pool, token, moto_id) = setup_test_app().await;

    let response = app
        .oneshot(auth(
            Request::builder()
                .uri(format!("/api/motorcycles/{}/tire-pressure", moto_id))
                .body(Body::empty())
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
    assert!(body["tirePressure"].is_null());
}

#[tokio::test]
async fn test_upsert_creates_then_updates() {
    let (app, pool, token, moto_id) = setup_test_app().await;

    // Insert.
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::PUT)
                .uri(format!("/api/motorcycles/{}/tire-pressure", moto_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "frontBar": 2.3,
                        "rearBar": 2.5,
                        "sidecarBar": null,
                        "preferredUnit": "bar"
                    })
                    .to_string(),
                ))
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
    assert_eq!(body["tirePressure"]["frontBar"], 2.3);
    assert_eq!(body["tirePressure"]["rearBar"], 2.5);
    assert!(body["tirePressure"]["sidecarBar"].is_null());
    assert_eq!(body["tirePressure"]["preferredUnit"], "bar");

    // Update same row (UNIQUE constraint → ON CONFLICT UPDATE).
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::PUT)
                .uri(format!("/api/motorcycles/{}/tire-pressure", moto_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "frontBar": 2.5,
                        "rearBar": 2.9,
                        "sidecarBar": 2.0,
                        "preferredUnit": "psi"
                    })
                    .to_string(),
                ))
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
    assert_eq!(body["tirePressure"]["frontBar"], 2.5);
    assert_eq!(body["tirePressure"]["rearBar"], 2.9);
    assert_eq!(body["tirePressure"]["sidecarBar"], 2.0);
    assert_eq!(body["tirePressure"]["preferredUnit"], "psi");

    // Exactly one row exists.
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tirePressures WHERE motorcycleId = ?")
            .bind(moto_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_delete_returns_404_when_missing() {
    let (app, _pool, token, moto_id) = setup_test_app().await;

    let response = app
        .oneshot(auth(
            Request::builder()
                .method(Method::DELETE)
                .uri(format!("/api/motorcycles/{}/tire-pressure", moto_id))
                .body(Body::empty())
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_after_upsert_clears_row() {
    let (app, pool, token, moto_id) = setup_test_app().await;

    // Seed.
    sqlx::query(
        "INSERT INTO tirePressures \
           (motorcycleId, frontBar, rearBar, sidecarBar, preferredUnit, createdAt, updatedAt) \
         VALUES (?, ?, ?, NULL, 'bar', datetime('now'), datetime('now'))",
    )
    .bind(moto_id)
    .bind(2.3_f64)
    .bind(2.5_f64)
    .execute(&pool)
    .await
    .unwrap();

    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::DELETE)
                .uri(format!("/api/motorcycles/{}/tire-pressure", moto_id))
                .body(Body::empty())
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tirePressures WHERE motorcycleId = ?")
            .bind(moto_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_upsert_rejects_unknown_unit() {
    let (app, _pool, token, moto_id) = setup_test_app().await;

    let response = app
        .oneshot(auth(
            Request::builder()
                .method(Method::PUT)
                .uri(format!("/api/motorcycles/{}/tire-pressure", moto_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "frontBar": 2.3,
                        "rearBar": 2.5,
                        "sidecarBar": null,
                        "preferredUnit": "kPa"
                    })
                    .to_string(),
                ))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_other_user_cannot_touch_pressure() {
    let (app, pool, _token, moto_id) = setup_test_app().await;

    // Create a second user and token.
    let password_hash = hash_password("password123").unwrap();
    let user2_id = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("other@example.com")
    .bind("other")
    .bind("Other User")
    .bind(password_hash)
    .bind("user")
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let token2 = create_session(&pool, user2_id).await.unwrap();

    let response = app
        .oneshot(auth(
            Request::builder()
                .uri(format!("/api/motorcycles/{}/tire-pressure", moto_id))
                .body(Body::empty())
                .unwrap(),
            &token2,
        ))
        .await
        .unwrap();
    // verify_motorcycle_ownership returns NotFound when the user doesn't own
    // the motorcycle — same behaviour as torque-specs / other handlers.
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
