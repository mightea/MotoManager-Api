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
use serde_json::Value;
use sqlx::sqlite::SqlitePoolOptions;
use std::str::FromStr;
use tower::ServiceExt;

async fn setup_test_app() -> (axum::Router, sqlx::SqlitePool, String) {
    let options = sqlx::sqlite::SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
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

    (build_app(state), pool, token)
}

async fn upload(
    app: &axum::Router,
    token: &str,
    filename: &str,
    bytes: &[u8],
) -> (StatusCode, Value) {
    let boundary = "ocr-test-boundary";
    let mut body = format!(
        "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{f}\"\r\n\
         Content-Type: application/octet-stream\r\n\r\n",
        b = boundary,
        f = filename
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/part-imports/parse")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(body))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn tesseract_available() -> bool {
    let command = std::env::var("TESSERACT_CMD").unwrap_or_else(|_| "tesseract".to_string());
    let found = std::process::Command::new(command)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    // CI installs Tesseract — a missing binary there must fail, not skip.
    assert!(
        found || std::env::var("CI").is_err(),
        "tesseract missing in CI"
    );
    found
}

#[tokio::test]
async fn rejects_files_that_are_neither_pdf_nor_image() {
    let (app, _pool, token) = setup_test_app().await;
    let (status, body) = upload(&app, &token, "notes.txt", b"just some text").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("weder PDF noch Bild"));
}

/// A photographed/scanned invoice (table rows cropped from a real Huggett
/// paper invoice) goes through server-side OCR and the layout parser.
#[tokio::test]
async fn parses_scanned_invoice_image_via_ocr() {
    if !tesseract_available() {
        eprintln!("skipping: tesseract not installed");
        return;
    }
    let (app, _pool, token) = setup_test_app().await;
    let (status, body) = upload(
        &app,
        &token,
        "scan.png",
        include_bytes!("fixtures/huggett_scan_rows.png"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", body);
    assert_eq!(body["textSource"], "ocr");
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{}", body);
    assert_eq!(items[0]["partNumber"], "83 30 0 401 758");
    assert_eq!(items[0]["quantity"], 1);
    assert_eq!(items[0]["lineTotal"], 51.62);
}
