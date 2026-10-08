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

async fn create_spec(app: &axum::Router, token: &str, moto_id: i64, body: Value) -> Value {
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/motorcycles/{}/torque-specs", moto_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
            token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice::<Value>(&bytes).unwrap()["torqueSpec"].clone()
}

async fn update_spec(app: &axum::Router, token: &str, moto_id: i64, id: i64, body: Value) -> Value {
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::PUT)
                .uri(format!("/api/motorcycles/{}/torque-specs/{}", moto_id, id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
            token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice::<Value>(&bytes).unwrap()["torqueSpec"].clone()
}

fn formatted() -> Value {
    json!({
        "category": "Motor",
        "name": "Zylinderkopf",
        "torque": 20,
        "description": "Achtung: nur kalt anziehen",
        "descriptionMarkup": "[red]**Achtung:**[/red] nur *kalt* anziehen"
    })
}

#[tokio::test]
async fn test_create_in_previous_shape_has_null_markup() {
    // Older iOS builds know nothing about descriptionMarkup; the previous
    // request shape must still work and the new key must be present (null).
    let (app, _pool, token, moto_id) = setup_test_app().await;
    let spec = create_spec(
        &app,
        &token,
        moto_id,
        json!({ "category": "Motor", "name": "Ölablass", "torque": 30, "description": "plain" }),
    )
    .await;
    assert_eq!(spec["description"], "plain");
    assert!(spec["descriptionMarkup"].is_null());
    assert_eq!(spec["unverified"], false);
}

#[tokio::test]
async fn test_create_stores_markup_and_blank_markup_is_null() {
    let (app, _pool, token, moto_id) = setup_test_app().await;
    let spec = create_spec(&app, &token, moto_id, formatted()).await;
    assert_eq!(
        spec["descriptionMarkup"],
        "[red]**Achtung:**[/red] nur *kalt* anziehen"
    );

    let blank = create_spec(
        &app,
        &token,
        moto_id,
        json!({ "category": "Motor", "name": "x", "torque": 1, "description": "p", "descriptionMarkup": "  " }),
    )
    .await;
    assert!(blank["descriptionMarkup"].is_null());
}

#[tokio::test]
async fn test_update_with_markup_replaces_it() {
    let (app, _pool, token, moto_id) = setup_test_app().await;
    let spec = create_spec(&app, &token, moto_id, formatted()).await;
    let id = spec["id"].as_i64().unwrap();

    let updated = update_spec(
        &app,
        &token,
        moto_id,
        id,
        json!({ "description": "Neu", "descriptionMarkup": "**Neu**" }),
    )
    .await;
    assert_eq!(updated["description"], "Neu");
    assert_eq!(updated["descriptionMarkup"], "**Neu**");

    // Explicit null clears the formatting while keeping the text.
    let cleared = update_spec(
        &app,
        &token,
        moto_id,
        id,
        json!({ "descriptionMarkup": null }),
    )
    .await;
    assert_eq!(cleared["description"], "Neu");
    assert!(cleared["descriptionMarkup"].is_null());
}

#[tokio::test]
async fn test_update_from_old_client_keeps_markup_when_text_unchanged() {
    // An older build re-sends the plain description unchanged (e.g. it edited
    // the torque value): formatting from a newer client must survive.
    let (app, _pool, token, moto_id) = setup_test_app().await;
    let spec = create_spec(&app, &token, moto_id, formatted()).await;
    let id = spec["id"].as_i64().unwrap();

    let updated = update_spec(
        &app,
        &token,
        moto_id,
        id,
        json!({ "torque": 22, "description": "Achtung: nur kalt anziehen" }),
    )
    .await;
    assert_eq!(updated["torque"], 22.0);
    assert_eq!(
        updated["descriptionMarkup"],
        "[red]**Achtung:**[/red] nur *kalt* anziehen"
    );

    // Omitting the description entirely (how the iOS client sends an empty
    // field) also keeps both columns.
    let untouched = update_spec(&app, &token, moto_id, id, json!({ "torque": 23 })).await;
    assert_eq!(untouched["description"], "Achtung: nur kalt anziehen");
    assert_eq!(
        untouched["descriptionMarkup"],
        "[red]**Achtung:**[/red] nur *kalt* anziehen"
    );
}

#[tokio::test]
async fn test_update_from_old_client_clears_markup_when_text_changes() {
    // An older build edits the plain text without knowing about markup: the
    // stale formatting must be dropped so newer clients fall back to plain.
    let (app, _pool, token, moto_id) = setup_test_app().await;
    let spec = create_spec(&app, &token, moto_id, formatted()).await;
    let id = spec["id"].as_i64().unwrap();

    let updated = update_spec(
        &app,
        &token,
        moto_id,
        id,
        json!({ "description": "Achtung: nur kalt anziehen, 2x" }),
    )
    .await;
    assert_eq!(updated["description"], "Achtung: nur kalt anziehen, 2x");
    assert!(updated["descriptionMarkup"].is_null());
}

#[tokio::test]
async fn test_import_copies_markup() {
    let (app, pool, token, moto_id) = setup_test_app().await;
    let spec = create_spec(&app, &token, moto_id, formatted()).await;
    let id = spec["id"].as_i64().unwrap();

    let target_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("BMW")
    .bind("R80")
    .bind(1)
    .bind(0)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::POST)
                .uri(format!(
                    "/api/motorcycles/{}/torque-specs/import",
                    target_id
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "sourceSpecIds": [id] }).to_string()))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let markup: Option<String> =
        sqlx::query_scalar("SELECT descriptionMarkup FROM torqueSpecs WHERE motorcycleId = ?")
            .bind(target_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        markup.as_deref(),
        Some("[red]**Achtung:**[/red] nur *kalt* anziehen")
    );
}
