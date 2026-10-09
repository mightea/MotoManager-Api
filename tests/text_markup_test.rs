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

/// POST/PUT a JSON body and return the entity under `key` of the response.
async fn send(
    app: &axum::Router,
    token: &str,
    method: Method,
    uri: String,
    body: Value,
    expected: StatusCode,
    key: &str,
) -> Value {
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(method)
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
            token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), expected, "{}", key);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice::<Value>(&bytes).unwrap()[key].clone()
}

const MARKUP: &str = "[red]**Achtung:**[/red] nur *kalt* anziehen";
const PLAIN: &str = "Achtung: nur kalt anziehen";

/// One free-text field of one entity: how to create a row carrying it and
/// how to address that row for updates.
struct Field {
    key: &'static str,
    plain: &'static str,
    markup: &'static str,
    create_uri: String,
    update_uri: Box<dyn Fn(i64) -> String>,
    base: Value,
    /// Make the n-th create distinct where the entity has a uniqueness rule.
    distinct: Box<dyn Fn(&mut Value, usize)>,
}

fn same(_: &mut Value, _: usize) {}

/// Exercise the full compatibility contract of one field:
/// 1. the previous request shape (no markup key) still works and yields null;
/// 2. markup is stored on create and blank markup is null;
/// 3. an update sending markup replaces it, explicit null clears it;
/// 4. an older client re-sending the unchanged plain text (or omitting it)
///    keeps the markup;
/// 5. an older client changing the plain text drops the stale markup.
async fn check_field(app: &axum::Router, token: &str, field: Field) {
    let mut created_count = 0usize;
    let mut create = |extra: Value| {
        let mut body = field.base.clone();
        (field.distinct)(&mut body, created_count);
        created_count += 1;
        for (k, v) in extra.as_object().unwrap() {
            body[k] = v.clone();
        }
        send(
            app,
            token,
            Method::POST,
            field.create_uri.clone(),
            body,
            StatusCode::CREATED,
            field.key,
        )
    };

    // 1. previous shape
    let old_shape = create(json!({ field.plain: PLAIN })).await;
    assert_eq!(old_shape[field.plain], PLAIN, "{}", field.key);
    assert!(old_shape[field.markup].is_null(), "{}", field.key);

    // 2. markup stored / blank → null
    let blank = create(json!({ field.plain: PLAIN, field.markup: "  " })).await;
    assert!(blank[field.markup].is_null(), "{}", field.key);
    let created = create(json!({ field.plain: PLAIN, field.markup: MARKUP })).await;
    assert_eq!(created[field.markup], MARKUP, "{}", field.key);
    let id = created["id"].as_i64().unwrap();

    let update = |body: Value| {
        send(
            app,
            token,
            Method::PUT,
            (field.update_uri)(id),
            body,
            StatusCode::OK,
            field.key,
        )
    };

    // 4. old client, unchanged text → markup survives; omitted → survives
    let mut same = field.base.clone();
    same[field.plain] = json!(PLAIN);
    let kept = update(same).await;
    assert_eq!(kept[field.markup], MARKUP, "{} unchanged text", field.key);
    let untouched = update(field.base.clone()).await;
    assert_eq!(untouched[field.plain], PLAIN, "{} omitted", field.key);
    assert_eq!(untouched[field.markup], MARKUP, "{} omitted", field.key);

    // 3. new client replaces, explicit null clears
    let mut replace = field.base.clone();
    replace[field.plain] = json!("Neu");
    replace[field.markup] = json!("**Neu**");
    let replaced = update(replace).await;
    assert_eq!(replaced[field.plain], "Neu", "{}", field.key);
    assert_eq!(replaced[field.markup], "**Neu**", "{}", field.key);
    let mut clear = field.base.clone();
    clear[field.markup] = Value::Null;
    let cleared = update(clear).await;
    assert_eq!(cleared[field.plain], "Neu", "{}", field.key);
    assert!(
        cleared[field.markup].is_null(),
        "{} explicit null",
        field.key
    );

    // 5. old client edits the text → stale markup dropped
    let mut restore = field.base.clone();
    restore[field.markup] = json!("**Neu**");
    update(restore).await;
    let mut edit = field.base.clone();
    edit[field.plain] = json!("Neu, 2x");
    let edited = update(edit).await;
    assert_eq!(edited[field.plain], "Neu, 2x", "{}", field.key);
    assert!(edited[field.markup].is_null(), "{} text changed", field.key);
}

#[tokio::test]
async fn maintenance_description_markup() {
    let (app, _pool, token, moto_id) = setup_test_app().await;
    check_field(
        &app,
        &token,
        Field {
            key: "maintenanceRecord",
            plain: "description",
            markup: "descriptionMarkup",
            create_uri: format!("/api/motorcycles/{moto_id}/maintenance"),
            update_uri: Box::new(move |id| format!("/api/motorcycles/{moto_id}/maintenance/{id}")),
            base: json!({ "date": "2026-10-01", "odo": 1200, "type": "repair" }),
            distinct: Box::new(same),
        },
    )
    .await;
}

#[tokio::test]
async fn issue_description_markup() {
    let (app, _pool, token, moto_id) = setup_test_app().await;
    check_field(
        &app,
        &token,
        Field {
            key: "issue",
            plain: "description",
            markup: "descriptionMarkup",
            create_uri: format!("/api/motorcycles/{moto_id}/issues"),
            update_uri: Box::new(move |id| format!("/api/motorcycles/{moto_id}/issues/{id}")),
            base: json!({ "odo": 1200, "title": "Ölverlust" }),
            distinct: Box::new(same),
        },
    )
    .await;
}

#[tokio::test]
async fn expense_description_markup() {
    let (app, _pool, token, moto_id) = setup_test_app().await;
    check_field(
        &app,
        &token,
        Field {
            key: "expense",
            plain: "description",
            markup: "descriptionMarkup",
            create_uri: "/api/expenses".to_string(),
            update_uri: Box::new(|id| format!("/api/expenses/{id}")),
            base: json!({
                "date": "2026-10-01", "amount": 120.0, "currency": "CHF",
                "category": "Versicherung", "motorcycleIds": [moto_id]
            }),
            distinct: Box::new(same),
        },
    )
    .await;
}

#[tokio::test]
async fn part_description_markup() {
    let (app, _pool, token, _moto_id) = setup_test_app().await;
    check_field(
        &app,
        &token,
        Field {
            key: "part",
            plain: "description",
            markup: "descriptionMarkup",
            create_uri: "/api/parts".to_string(),
            update_uri: Box::new(|id| format!("/api/parts/{id}")),
            base: json!({ "partNumber": "11 11 1 234 567", "name": "Ölfilter" }),
            // partNumber + name is unique per user; updates then rename back
            // to the base name, which no other row carries.
            distinct: Box::new(|body, n| body["name"] = json!(format!("Ölfilter {n}"))),
        },
    )
    .await;
}

#[tokio::test]
async fn part_stock_notes_markup() {
    let (app, _pool, token, _moto_id) = setup_test_app().await;
    let part = send(
        &app,
        &token,
        Method::POST,
        "/api/parts".to_string(),
        json!({ "partNumber": "11 11 1 234 567", "name": "Ölfilter" }),
        StatusCode::CREATED,
        "part",
    )
    .await;
    let part_id = part["id"].as_i64().unwrap();
    check_field(
        &app,
        &token,
        Field {
            key: "partStock",
            plain: "notes",
            markup: "notesMarkup",
            create_uri: "/api/part-stocks".to_string(),
            update_uri: Box::new(|id| format!("/api/part-stocks/{id}")),
            base: json!({ "partId": part_id, "quantity": 2 }),
            distinct: Box::new(same),
        },
    )
    .await;
}

#[tokio::test]
async fn previous_owner_comments_markup() {
    let (app, _pool, token, moto_id) = setup_test_app().await;
    check_field(
        &app,
        &token,
        Field {
            key: "previousOwner",
            plain: "comments",
            markup: "commentsMarkup",
            create_uri: format!("/api/motorcycles/{moto_id}/previous-owners"),
            update_uri: Box::new(move |id| {
                format!("/api/motorcycles/{moto_id}/previous-owners/{id}")
            }),
            base: json!({ "name": "Hans", "surname": "Muster" }),
            distinct: Box::new(same),
        },
    )
    .await;
}

#[tokio::test]
async fn public_parts_expose_description_markup() {
    let (app, pool, token, _moto_id) = setup_test_app().await;
    // A second user's public part is what the catalogue lists.
    let other_hash = hash_password("password123").unwrap();
    let other_id = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("other@example.com")
    .bind("other")
    .bind("Other")
    .bind(other_hash)
    .bind("user")
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let other_token = create_session(&pool, other_id).await.unwrap();
    send(
        &app,
        &other_token,
        Method::POST,
        "/api/parts".to_string(),
        json!({
            "partNumber": "11 11 1 234 567", "name": "Ölfilter", "isPublic": true,
            "description": PLAIN, "descriptionMarkup": MARKUP
        }),
        StatusCode::CREATED,
        "part",
    )
    .await;

    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::GET)
                .uri("/api/parts/public")
                .body(Body::empty())
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parts = serde_json::from_slice::<Value>(&bytes).unwrap()["parts"].clone();
    assert_eq!(parts[0]["description"], PLAIN);
    assert_eq!(parts[0]["descriptionMarkup"], MARKUP);
}
