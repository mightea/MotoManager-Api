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
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert!(body["docs"].is_array());
    assert_eq!(body["docs"].as_array().unwrap().len(), 0);
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
    let response = app
        .clone()
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
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["docs"].as_array().unwrap().len(), 1);
    assert_eq!(body["docs"][0]["title"], "Manual");

    // 3. Delete document (Note: delete_document also tries to delete files from disk)
    // In setup_test_app, data_dir is ./test_data. We should ensure the files exist or mock it.
    // For this test, we'll just check if the handler returns 200 OK after we "mock" the file.

    // Create dummy file
    tokio::fs::create_dir_all("./test_data/documents")
        .await
        .unwrap();
    tokio::fs::write("./test_data/documents/manual.pdf", b"dummy")
        .await
        .unwrap();

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

    // Cleanup: remove only this test's file — ./test_data is shared across
    // concurrently running test binaries (e.g. the part-image upload test),
    // so a remove_dir_all here intermittently breaks their file writes.
    let _ = tokio::fs::remove_file("./test_data/documents/manual.pdf").await;
}

#[tokio::test]
async fn test_list_all_motorcycles_independent_of_user() {
    let (app, pool, token) = setup_test_app().await;

    // Create another user
    let password_hash = hash_password("otherpass").unwrap();
    let other_user_id = sqlx::query(
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

    // Create motorcycle for first user
    sqlx::query!(
        "INSERT INTO motorcycles (make, model, userId, isArchived) VALUES (?, ?, ?, ?)",
        "Yamaha",
        "MT-07",
        1,
        0
    )
    .execute(&pool)
    .await
    .unwrap();

    // Create motorcycle for other user
    sqlx::query!(
        "INSERT INTO motorcycles (make, model, userId, isArchived) VALUES (?, ?, ?, ?)",
        "Honda",
        "CB650R",
        other_user_id,
        0
    )
    .execute(&pool)
    .await
    .unwrap();

    // Create archived motorcycle
    sqlx::query!(
        "INSERT INTO motorcycles (make, model, userId, isArchived) VALUES (?, ?, ?, ?)",
        "Suzuki",
        "SV650",
        1,
        1
    )
    .execute(&pool)
    .await
    .unwrap();

    // List documents
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
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    let all_motorcycles = body["allMotorcycles"].as_array().unwrap();

    // Should have 2 (Yamaha and Honda), Suzuki is archived
    assert_eq!(all_motorcycles.len(), 2);

    // Verify Yamaha (Test User)
    let yamaha = all_motorcycles
        .iter()
        .find(|m| m["make"] == "Yamaha")
        .unwrap();
    assert_eq!(yamaha["ownerName"], "Test User");

    // Verify Honda (Other User)
    let honda = all_motorcycles
        .iter()
        .find(|m| m["make"] == "Honda")
        .unwrap();
    assert_eq!(honda["ownerName"], "Other User");
}

#[tokio::test]
async fn test_serve_document_enforces_privacy() {
    // The raw document file route used to be unauthenticated: anyone with the URL
    // could fetch a "private" document. It now requires auth + ownership.
    let (app, pool, owner_token) = setup_test_app().await;

    // A second user who does NOT own the document.
    let other_hash = hash_password("password123").unwrap();
    let other_id = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("other@example.com")
    .bind("otheruser")
    .bind("Other User")
    .bind(other_hash)
    .bind("user")
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let other_token = create_session(&pool, other_id).await.unwrap();

    // A real private document file on disk, owned by user 1.
    tokio::fs::create_dir_all("./test_data/documents")
        .await
        .unwrap();
    let filename = "priv-doc-test.pdf";
    tokio::fs::write(
        "./test_data/documents/priv-doc-test.pdf",
        b"%PDF-1.4 secret",
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO documents (title, filePath, ownerId, isPrivate, createdAt, updatedAt) \
         VALUES (?, ?, ?, 1, ?, ?)",
    )
    .bind("Secret")
    .bind(filename)
    .bind(1_i64)
    .bind("2025-01-01T00:00:00Z")
    .bind("2025-01-01T00:00:00Z")
    .execute(&pool)
    .await
    .unwrap();

    let build = |token: Option<&str>| {
        let mut b = Request::builder().uri(format!("/documents/{}", filename));
        if let Some(t) = token {
            b = b.header(header::AUTHORIZATION, format!("Bearer {}", t));
        }
        b.body(Body::empty()).unwrap()
    };

    // No token → 401.
    let r = app.clone().oneshot(build(None)).await.unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

    // Authenticated non-owner → 404 (existence masked).
    let r = app
        .clone()
        .oneshot(build(Some(&other_token)))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);

    // Owner → 200 with the bytes.
    let r = app.oneshot(build(Some(&owner_token))).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    let _ = tokio::fs::remove_file("./test_data/documents/priv-doc-test.pdf").await;
}
