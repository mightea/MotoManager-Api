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

/// Any authorized request; optional app-version headers.
fn me_request(token: &str, app: Option<(&str, &str)>) -> Request<Body> {
    let mut builder = Request::builder()
        .uri("/api/auth/me")
        .header(header::AUTHORIZATION, format!("Bearer {}", token));
    if let Some((version, build)) = app {
        builder = builder
            .header("X-App-Version", version)
            .header("X-App-Build", build);
    }
    builder.body(Body::empty()).unwrap()
}

/// Fetch alice's row from the admin user list.
async fn alice_from_admin_list(app: &axum::Router, admin_token: &str) -> Value {
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/admin/users")
                .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = read_json(r).await;
    body["users"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "alice")
        .unwrap()
        .clone()
}

#[tokio::test]
async fn app_version_headers_are_recorded_and_survive_headerless_requests() {
    let (app, admin_token, user_token) = setup_test_app().await;

    // Starts unset.
    let alice = alice_from_admin_list(&app, &admin_token).await;
    assert!(alice["appVersion"].is_null());
    assert!(alice["appBuild"].is_null());

    // A request carrying the headers records them.
    let r = app
        .clone()
        .oneshot(me_request(&user_token, Some(("1.2", "1001"))))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    let alice = alice_from_admin_list(&app, &admin_token).await;
    assert_eq!(alice["appVersion"], "1.2");
    assert_eq!(alice["appBuild"], 1001);

    // A headerless request (webapp, curl) must not clear the stored values.
    let r = app
        .clone()
        .oneshot(me_request(&user_token, None))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    let alice = alice_from_admin_list(&app, &admin_token).await;
    assert_eq!(alice["appVersion"], "1.2");
    assert_eq!(alice["appBuild"], 1001);

    // A newer build overwrites.
    let r = app
        .clone()
        .oneshot(me_request(&user_token, Some(("1.3", "1011"))))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    let alice = alice_from_admin_list(&app, &admin_token).await;
    assert_eq!(alice["appVersion"], "1.3");
    assert_eq!(alice["appBuild"], 1011);
}

#[tokio::test]
async fn malformed_headers_are_ignored() {
    let (app, admin_token, user_token) = setup_test_app().await;

    // Non-numeric build → ignored entirely.
    let r = app
        .clone()
        .oneshot(me_request(&user_token, Some(("1.2", "not-a-number"))))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    let alice = alice_from_admin_list(&app, &admin_token).await;
    assert!(alice["appVersion"].is_null());
    assert!(alice["appBuild"].is_null());
}

#[tokio::test]
async fn own_profile_reflects_the_recording_request_immediately() {
    let (app, _admin_token, user_token) = setup_test_app().await;

    // The extractor records before the handler runs, so the very request that
    // carries the headers already sees them in its own response (additive
    // keys — old clients ignore them).
    let r = app
        .oneshot(me_request(&user_token, Some(("1.2", "1001"))))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = read_json(r).await;
    assert_eq!(body["user"]["appVersion"], "1.2");
    assert_eq!(body["user"]["appBuild"], 1001);
}
