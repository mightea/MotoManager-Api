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

/// Returns app + pool plus a session token per role: (admin_token, user_token)
/// for users admin (id 1) and alice (id 2).
async fn setup_test_app() -> (axum::Router, sqlx::SqlitePool, String, String) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
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
    (build_app(state), pool, admin_token, user_token)
}

/// Send an authorized request with an optional JSON body; return status + parsed body.
async fn request(
    app: &axum::Router,
    method: Method,
    uri: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {}", token));
    let body = match body {
        Some(v) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(serde_json::to_vec(&v).unwrap())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: Value = if bytes.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&bytes).unwrap_or(json!({}))
    };
    (status, value)
}

async fn impersonate(app: &axum::Router, admin_token: &str, uid: i64) -> (StatusCode, Value) {
    request(
        app,
        Method::POST,
        &format!("/api/admin/users/{uid}/impersonate"),
        admin_token,
        None,
    )
    .await
}

#[tokio::test]
async fn admin_impersonates_user_and_acts_as_them() {
    let (app, pool, admin_token, _user_token) = setup_test_app().await;

    let (status, body) = impersonate(&app, &admin_token, 2).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["username"], "alice");
    let imp_token = body["token"].as_str().unwrap().to_string();

    // The impersonation session resolves to alice and reports the acting admin.
    let (status, body) = request(&app, Method::GET, "/api/auth/me", &imp_token, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["username"], "alice");
    assert_eq!(body["impersonatedBy"]["id"], 1);
    assert_eq!(body["impersonatedBy"]["username"], "admin");

    // A regular session carries no impersonatedBy key (old-contract shape).
    let (_, body) = request(&app, Method::GET, "/api/auth/me", &admin_token, None).await;
    assert!(body.get("impersonatedBy").is_none(), "{body}");

    // The session is short-lived (1 hour, not 14 days) and audit-tagged.
    let (expires_at, impersonator_id): (String, i64) =
        sqlx::query_as("SELECT expiresAt, impersonatorId FROM sessions WHERE token = ?")
            .bind(&imp_token)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(impersonator_id, 1);
    let expires = chrono::DateTime::parse_from_rfc3339(&expires_at).unwrap();
    let hours = (expires.with_timezone(&chrono::Utc) - chrono::Utc::now()).num_hours();
    assert!(hours < 2, "impersonation session must be short-lived");

    // Data endpoints work as the user (alice has no motorcycles yet).
    let (status, _) = request(&app, Method::GET, "/api/motorcycles", &imp_token, None).await;
    assert_eq!(status, StatusCode::OK);

    // Ending impersonation = logging the session out; the token dies with it.
    let (status, _) = request(&app, Method::POST, "/api/auth/logout", &imp_token, None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = request(&app, Method::GET, "/api/auth/me", &imp_token, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn impersonation_guards_and_limits() {
    let (app, _pool, admin_token, user_token) = setup_test_app().await;

    // Non-admins cannot impersonate.
    let (status, _) = impersonate(&app, &user_token, 1).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Impersonating yourself or a missing user is rejected.
    let (status, _) = impersonate(&app, &admin_token, 1).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = impersonate(&app, &admin_token, 999).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, body) = impersonate(&app, &admin_token, 2).await;
    let imp_token = body["token"].as_str().unwrap().to_string();

    // Credential management is blocked while impersonating: password change,
    // passkey registration, and authenticator removal must not be reachable.
    let (status, _) = request(
        &app,
        Method::POST,
        "/api/settings/change-password",
        &imp_token,
        Some(json!({
            "currentPassword": "password123",
            "newPassword": "hijacked123",
            "confirmPassword": "hijacked123"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = request(
        &app,
        Method::GET,
        "/api/auth/passkey/register-options",
        &imp_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = request(
        &app,
        Method::DELETE,
        "/api/settings/authenticators/some-id",
        &imp_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The same routes keep working for regular sessions.
    let (status, _) = request(
        &app,
        Method::POST,
        "/api/settings/change-password",
        &user_token,
        Some(json!({
            "currentPassword": "password123",
            "newPassword": "password1234",
            "confirmPassword": "password1234"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}
