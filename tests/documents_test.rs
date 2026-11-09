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
        "INSERT INTO motorcycles (make, model, userId, status) VALUES (?, ?, ?, ?)",
        "Yamaha",
        "MT-07",
        1,
        "active"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Create motorcycle for other user
    sqlx::query!(
        "INSERT INTO motorcycles (make, model, userId, status) VALUES (?, ?, ?, ?)",
        "Honda",
        "CB650R",
        other_user_id,
        "active"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Create sold motorcycle (excluded from the document picker)
    sqlx::query!(
        "INSERT INTO motorcycles (make, model, userId, status) VALUES (?, ?, ?, ?)",
        "Suzuki",
        "SV650",
        1,
        "sold"
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

fn multipart_upload(token: &str, file_name: &str, bytes: &[u8]) -> Request<Body> {
    let boundary = "XBOUNDARYX";
    let mut body = Vec::new();
    body.extend_from_slice(
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nUpload\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    Request::builder()
        .method("POST")
        .uri("/api/documents")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap()
}

/// SEC-01: script-capable uploads (HTML, SVG, XML) are rejected outright.
#[tokio::test]
async fn test_upload_rejects_active_content() {
    let (app, _, token) = setup_test_app().await;
    tokio::fs::create_dir_all("./test_data/documents")
        .await
        .unwrap();

    for (name, bytes) in [
        ("x.html", &b"<script>alert(1)</script>"[..]),
        (
            "x.svg",
            &b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>"[..],
        ),
        ("x.xml", &b"<?xml version=\"1.0\"?><a/>"[..]),
        ("x.txt", &b"\x00\x01binary"[..]),
    ] {
        let r = app
            .clone()
            .oneshot(multipart_upload(&token, name, bytes))
            .await
            .unwrap();
        assert_eq!(
            r.status(),
            StatusCode::BAD_REQUEST,
            "{name} must be rejected"
        );
    }
}

/// SEC-01: the stored extension follows the bytes, not the client's file name,
/// and served files carry nosniff + a sandbox CSP.
#[tokio::test]
async fn test_upload_stored_by_content_and_served_sandboxed() {
    let (app, _, token) = setup_test_app().await;
    tokio::fs::create_dir_all("./test_data/documents")
        .await
        .unwrap();

    let png = b"\x89PNG\r\n\x1a\nnot-really-a-png";
    let r = app
        .clone()
        .oneshot(multipart_upload(&token, "evil.svg", png))
        .await
        .unwrap();
    assert!(r.status().is_success(), "{}", r.status());
    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(r.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let file_path = body["document"]["filePath"].as_str().unwrap().to_string();
    assert!(file_path.ends_with(".png"), "{file_path}");

    let r = app
        .oneshot(
            Request::builder()
                .uri(&file_path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let h = r.headers();
    assert_eq!(h[header::CONTENT_TYPE], "image/png");
    assert_eq!(h[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(h[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap()
        .starts_with("sandbox"));
    assert!(h[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .starts_with("inline"));

    let stored = file_path.trim_start_matches("/documents/");
    let _ = tokio::fs::remove_file(format!("./test_data/documents/{stored}")).await;
}

/// SEC-01: an HTML file uploaded before the allowlist existed is served as a
/// sandboxed download, never rendered inline.
#[tokio::test]
async fn test_legacy_html_document_is_forced_to_download() {
    let (app, pool, token) = setup_test_app().await;
    tokio::fs::create_dir_all("./test_data/documents")
        .await
        .unwrap();
    let filename = "legacy-sec01.html";
    tokio::fs::write(
        format!("./test_data/documents/{filename}"),
        b"<script>alert(1)</script>",
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO documents (title, filePath, ownerId, isPrivate, createdAt, updatedAt) \
         VALUES ('Legacy', ?, 1, 0, '2025-01-01', '2025-01-01')",
    )
    .bind(filename)
    .execute(&pool)
    .await
    .unwrap();

    let r = app
        .oneshot(
            Request::builder()
                .uri(format!("/documents/{filename}"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let h = r.headers();
    assert!(h[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .starts_with("attachment"));
    assert_eq!(h[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(h[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap()
        .starts_with("sandbox"));

    let _ = tokio::fs::remove_file(format!("./test_data/documents/{filename}")).await;
}

/// SEC-03: the session token is no longer accepted as a `?token=` query
/// parameter on the file routes — only as a Bearer header.
#[tokio::test]
async fn test_file_routes_reject_query_token() {
    let (app, _, token) = setup_test_app().await;
    for path in ["/documents/any.pdf", "/previews/any.jpg"] {
        let r = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("{path}?token={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED, "{path}");
    }
}

/// Build a multipart request with arbitrary text fields and an optional file.
fn multipart_fields(
    method: &str,
    uri: &str,
    token: &str,
    fields: &[(&str, &str)],
    file: Option<(&str, &[u8])>,
) -> Request<Body> {
    let boundary = "XBOUNDARYX";
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    if let Some((file_name, bytes)) = file {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap()
}

async fn json_body(r: axum::response::Response) -> Value {
    serde_json::from_slice(
        &axum::body::to_bytes(r.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap()
}

/// docType: optional on create/update, validated, returned in every document
/// payload; requests in the old shape (no docType) keep working.
#[tokio::test]
async fn test_document_type_create_update_and_validation() {
    let (app, _, token) = setup_test_app().await;
    tokio::fs::create_dir_all("./test_data/documents")
        .await
        .unwrap();
    let file = Some(("notes.txt", &b"hello"[..]));
    let mut stored = Vec::new();

    // Old shape: no docType → created, docType null.
    let r = app
        .clone()
        .oneshot(multipart_fields(
            "POST",
            "/api/documents",
            &token,
            &[("title", "Old client")],
            file,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    let body = json_body(r).await;
    assert!(body["document"]["docType"].is_null());
    stored.push(body["document"]["filePath"].as_str().unwrap().to_string());

    // Unknown type → 400.
    let r = app
        .clone()
        .oneshot(multipart_fields(
            "POST",
            "/api/documents",
            &token,
            &[("title", "Bad"), ("docType", "kochbuch")],
            file,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);

    // Valid type → stored and returned.
    let r = app
        .clone()
        .oneshot(multipart_fields(
            "POST",
            "/api/documents",
            &token,
            &[("title", "Plan"), ("docType", "schaltplan")],
            file,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    let body = json_body(r).await;
    assert_eq!(body["document"]["docType"], "schaltplan");
    stored.push(body["document"]["filePath"].as_str().unwrap().to_string());
    let id = body["document"]["id"].as_i64().unwrap();
    let uri = format!("/api/documents/{id}");

    // Update without docType keeps it.
    let r = app
        .clone()
        .oneshot(multipart_fields(
            "PUT",
            &uri,
            &token,
            &[("title", "Plan 2")],
            None,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = json_body(r).await;
    assert_eq!(body["document"]["docType"], "schaltplan");
    assert_eq!(body["document"]["title"], "Plan 2");

    // Update with an invalid type → 400, unchanged.
    let r = app
        .clone()
        .oneshot(multipart_fields(
            "PUT",
            &uri,
            &token,
            &[("docType", "nope")],
            None,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);

    // Update to another type.
    let r = app
        .clone()
        .oneshot(multipart_fields(
            "PUT",
            &uri,
            &token,
            &[("docType", "checkliste")],
            None,
        ))
        .await
        .unwrap();
    assert_eq!(json_body(r).await["document"]["docType"], "checkliste");

    // List carries it.
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/documents")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(r).await;
    let listed = body["docs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == id)
        .unwrap()
        .clone();
    assert_eq!(listed["docType"], "checkliste");

    // Empty value clears it.
    let r = app
        .clone()
        .oneshot(multipart_fields(
            "PUT",
            &uri,
            &token,
            &[("docType", "")],
            None,
        ))
        .await
        .unwrap();
    assert!(json_body(r).await["document"]["docType"].is_null());

    for path in stored {
        let name = path.trim_start_matches("/documents/");
        let _ = tokio::fs::remove_file(format!("./test_data/documents/{name}")).await;
    }
}
