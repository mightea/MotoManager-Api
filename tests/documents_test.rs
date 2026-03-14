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
use serde_json::Value;
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
async fn test_list_documents_empty() {
    let (app, _, token) = setup_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/documents")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert!(body["documents"].is_array());
    assert_eq!(body["documents"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_document_lifecycle() {
    let (app, pool, token) = setup_test_app().await;

    // 1. Seed a document
    let doc_id = sqlx::query(
        "INSERT INTO documents (title, filePath, ownerId, isPrivate) VALUES (?, ?, ?, ?)",
    )
    .bind("Manual")
    .bind("manual.pdf")
    .bind(1) // test user
    .bind(0)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    // 2. List documents
    let response = app.clone()
        .oneshot(
            Request::builder()
                .uri("/api/documents")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["documents"].as_array().unwrap().len(), 1);
    assert_eq!(body["documents"][0]["title"], "Manual");

    // 3. Delete document (Note: delete_document also tries to delete files from disk)
    // In setup_test_app, data_dir is ./test_data. We should ensure the files exist or mock it.
    // For this test, we'll just check if the handler returns 200 OK after we "mock" the file.
    
    // Create dummy file
    tokio::fs::create_dir_all("./test_data/documents").await.unwrap();
    tokio::fs::write("./test_data/documents/manual.pdf", b"dummy").await.unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/documents/{}", doc_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // cleanup
    let _ = tokio::fs::remove_dir_all("./test_data").await;
}
