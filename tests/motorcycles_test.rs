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
use sqlx::{sqlite::SqlitePoolOptions, Row};
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
        origin: "http://localhost:3001".to_string(),
        enable_registration: true,
        app_version: "test".to_string(),
        data_dir: "./test_data".to_string(),
    };

    let state = AppState {
        pool: pool.clone(),
        config,
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
async fn test_health_check() {
    let (app, _, _) = setup_test_app().await;

    let response = app
        .oneshot(Request::builder().uri("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_list_motorcycles_empty() {
    let (app, _, token) = setup_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/motorcycles")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert!(body["motorcycles"].is_array());
    assert_eq!(body["motorcycles"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_list_motorcycles_unauthorized() {
    let (app, _, _) = setup_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/motorcycles")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_motorcycle_lifecycle() {
    let (app, pool, token) = setup_test_app().await;

    // 1. Seed a motorcycle
    let moto_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("BMW")
    .bind("R1250GS")
    .bind(1) // from setup_test_app
    .bind(1000)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    // 2. List motorcycles
    let response = app.clone()
        .oneshot(
            Request::builder()
                .uri("/api/motorcycles")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["motorcycles"].as_array().unwrap().len(), 1);
    assert_eq!(body["motorcycles"][0]["make"], "BMW");

    // 3. Get specific motorcycle
    let response = app.clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/motorcycles/{}", moto_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["motorcycle"]["model"], "R1250GS");
    assert!(body["torqueSpecs"].is_array());

    // 4. Delete motorcycle
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/motorcycles/{}", moto_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 5. Verify deleted
    let count: i64 = sqlx::query("SELECT COUNT(*) FROM motorcycles WHERE id = ?")
        .bind(moto_id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_issue_lifecycle() {
    let (app, pool, token) = setup_test_app().await;

    // 1. Seed a motorcycle
    let moto_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("Honda")
    .bind("CB500X")
    .bind(1)
    .bind(5000)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    // 2. Create an issue
    let response = app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/motorcycles/{}/issues", moto_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "odo": 5100,
                    "description": "Strange noise from engine",
                    "priority": "high"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    let issue_id = body["issue"]["id"].as_i64().unwrap();

    // 3. List issues
    let response = app.clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/motorcycles/{}/issues", moto_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["issues"].as_array().unwrap().len(), 1);

    // 4. Update issue
    let response = app.clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/motorcycles/{}/issues/{}", moto_id, issue_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "status": "in_progress"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 5. Delete issue
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/motorcycles/{}/issues/{}", moto_id, issue_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_maintenance_lifecycle() {
    let (app, pool, token) = setup_test_app().await;

    // 1. Seed a motorcycle
    let moto_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("Yamaha")
    .bind("MT-07")
    .bind(1)
    .bind(2000)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    // 2. Create a maintenance record
    let response = app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/motorcycles/{}/maintenance", moto_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "date": "2026-03-14",
                    "odo": 2500,
                    "type": "oil_change",
                    "description": "Regular maintenance"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    let mid = body["maintenanceRecord"]["id"].as_i64().unwrap();

    // 3. List maintenance records
    let response = app.clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/motorcycles/{}/maintenance", moto_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["maintenanceRecords"].as_array().unwrap().len(), 1);

    // 4. Update maintenance record
    let response = app.clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/motorcycles/{}/maintenance/{}", moto_id, mid))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "description": "Oil and filter change"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 5. Delete maintenance record
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/motorcycles/{}/maintenance/{}", moto_id, mid))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    }

#[tokio::test]
async fn test_get_motorcycle_not_found() {
    let (app, _, token) = setup_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/motorcycles/999")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
