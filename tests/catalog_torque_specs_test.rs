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

struct Ctx {
    app: axum::Router,
    pool: sqlx::SqlitePool,
    /// Family › Series › Model (global catalog rows).
    family: i64,
    series: i64,
    model: i64,
    /// Unrelated global node.
    other: i64,
}

async fn setup() -> Ctx {
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

    let node = |name: &'static str, parent: Option<i64>| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO modelSeries (name, manufacturer, parentId) VALUES (?, 'Testmarke', ?)",
            )
            .bind(name)
            .bind(parent)
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid()
        }
    };
    let family = node("Familie T", None).await;
    let series = node("Serie T", Some(family)).await;
    let model = node("Modell T", Some(series)).await;
    let other = node("Andere T", None).await;

    Ctx {
        app: build_app(state),
        pool,
        family,
        series,
        model,
        other,
    }
}

async fn user(ctx: &Ctx, username: &str, role: &str) -> (i64, String) {
    let id = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(format!("{username}@example.com"))
    .bind(username)
    .bind(username)
    .bind(hash_password("password123").unwrap())
    .bind(role)
    .execute(&ctx.pool)
    .await
    .unwrap()
    .last_insert_rowid();
    (id, create_session(&ctx.pool, id).await.unwrap())
}

async fn bike(ctx: &Ctx, user_id: i64, series_id: Option<i64>) -> i64 {
    sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo, seriesId) VALUES ('BMW', 'Test', ?, 0, ?)",
    )
    .bind(user_id)
    .bind(series_id)
    .execute(&ctx.pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn spec(ctx: &Ctx, bike_id: i64, category: &str, name: &str, torque: f64) {
    sqlx::query(
        "INSERT INTO torqueSpecs (motorcycleId, category, name, torque, toolSize, descriptionMarkup, createdAt) \
         VALUES (?, ?, ?, ?, 'SW 13', '**fest**', '2026-01-01T00:00:00.000Z')",
    )
    .bind(bike_id)
    .bind(category)
    .bind(name)
    .bind(torque)
    .execute(&ctx.pool)
    .await
    .unwrap();
}

async fn call(
    ctx: &Ctx,
    method: Method,
    uri: String,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    let body = match body {
        Some(b) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let response = ctx
        .app
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn share(ctx: &Ctx, token: &str, bike_id: i64) -> (StatusCode, Value) {
    call(
        ctx,
        Method::POST,
        format!("/api/motorcycles/{bike_id}/torque-specs/share-catalog"),
        token,
        None,
    )
    .await
}

async fn catalog(ctx: &Ctx, token: &str, sid: i64) -> Value {
    let (status, body) = call(
        ctx,
        Method::GET,
        format!("/api/model-series/{sid}/torque-specs"),
        token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    body
}

#[tokio::test]
async fn share_copies_specs_and_lists_with_contributor() {
    let ctx = setup().await;
    let (alice, alice_token) = user(&ctx, "alice", "user").await;
    let (_, bob_token) = user(&ctx, "bob", "user").await;
    let bike_id = bike(&ctx, alice, Some(ctx.series)).await;
    spec(&ctx, bike_id, "Motor", "Zylinderkopf", 20.0).await;
    spec(&ctx, bike_id, "Fahrwerk", "Achsmutter", 100.0).await;

    let (status, body) = share(&ctx, &alice_token, bike_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["count"], 2);
    assert_eq!(body["seriesId"], ctx.series);

    // Visible to another user of the instance.
    let list = catalog(&ctx, &bob_token, ctx.series).await;
    let specs = list["torqueSpecs"].as_array().unwrap();
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[0]["contributorUsername"], "alice");
    assert_eq!(specs[0]["seriesName"], "Serie T");
    assert_eq!(specs[0]["descriptionMarkup"], "**fest**");
    assert!(specs[0].get("email").is_none());
    let contributions = list["contributions"].as_array().unwrap();
    assert_eq!(contributions.len(), 1);
    assert_eq!(contributions[0]["count"], 2);
    assert_eq!(contributions[0]["isOwn"], false);
    assert_eq!(
        catalog(&ctx, &alice_token, ctx.series).await["contributions"][0]["isOwn"],
        true
    );
}

#[tokio::test]
async fn reshare_replaces_previous_contribution() {
    let ctx = setup().await;
    let (alice, token) = user(&ctx, "alice", "user").await;
    let bike_id = bike(&ctx, alice, Some(ctx.series)).await;
    spec(&ctx, bike_id, "Motor", "Zylinderkopf", 20.0).await;
    share(&ctx, &token, bike_id).await;

    spec(&ctx, bike_id, "Motor", "Ölablass", 25.0).await;
    let (status, _) = share(&ctx, &token, bike_id).await;
    assert_eq!(status, StatusCode::OK);

    let list = catalog(&ctx, &token, ctx.series).await;
    assert_eq!(list["torqueSpecs"].as_array().unwrap().len(), 2);
    assert_eq!(list["contributions"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn share_requires_series_and_specs() {
    let ctx = setup().await;
    let (alice, token) = user(&ctx, "alice", "user").await;
    let no_series = bike(&ctx, alice, None).await;
    spec(&ctx, no_series, "Motor", "Zylinderkopf", 20.0).await;
    assert_eq!(
        share(&ctx, &token, no_series).await.0,
        StatusCode::BAD_REQUEST
    );

    let empty = bike(&ctx, alice, Some(ctx.series)).await;
    assert_eq!(share(&ctx, &token, empty).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn share_foreign_bike_is_not_found() {
    let ctx = setup().await;
    let (alice, _) = user(&ctx, "alice", "user").await;
    let (_, bob_token) = user(&ctx, "bob", "user").await;
    let bike_id = bike(&ctx, alice, Some(ctx.series)).await;
    spec(&ctx, bike_id, "Motor", "Zylinderkopf", 20.0).await;
    assert_eq!(
        share(&ctx, &bob_token, bike_id).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn list_matches_ancestors_and_descendants_only() {
    let ctx = setup().await;
    let (alice, token) = user(&ctx, "alice", "user").await;
    let on_model = bike(&ctx, alice, Some(ctx.model)).await;
    spec(&ctx, on_model, "Motor", "Modell-Wert", 10.0).await;
    share(&ctx, &token, on_model).await;
    let on_family = bike(&ctx, alice, Some(ctx.family)).await;
    spec(&ctx, on_family, "Motor", "Familien-Wert", 11.0).await;
    share(&ctx, &token, on_family).await;
    let on_other = bike(&ctx, alice, Some(ctx.other)).await;
    spec(&ctx, on_other, "Motor", "Fremd", 12.0).await;
    share(&ctx, &token, on_other).await;

    // The Serie sees its Familie (ancestor) and Modell (descendant).
    let names: Vec<String> = catalog(&ctx, &token, ctx.series).await["torqueSpecs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"Modell-Wert".to_string()));
    assert!(names.contains(&"Familien-Wert".to_string()));
    assert_eq!(
        catalog(&ctx, &token, ctx.other).await["torqueSpecs"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // Per-node counts on the catalog list.
    let (_, list) = call(
        &ctx,
        Method::GET,
        "/api/model-series".to_string(),
        &token,
        None,
    )
    .await;
    assert_eq!(list["torqueSpecCounts"][ctx.model.to_string()], 1);
    assert!(list["torqueSpecCounts"]
        .get(ctx.series.to_string())
        .is_none());
}

#[tokio::test]
async fn import_copies_as_plain_specs_and_skips_duplicates() {
    let ctx = setup().await;
    let (alice, alice_token) = user(&ctx, "alice", "user").await;
    let (bob, bob_token) = user(&ctx, "bob", "user").await;
    let source = bike(&ctx, alice, Some(ctx.series)).await;
    spec(&ctx, source, "Motor", "Zylinderkopf", 20.0).await;
    spec(&ctx, source, "Motor", "Ölablass", 25.0).await;
    share(&ctx, &alice_token, source).await;

    let target = bike(&ctx, bob, Some(ctx.model)).await;
    spec(&ctx, target, "Motor", "Zylinderkopf", 20.0).await;
    let ids: Vec<i64> = catalog(&ctx, &bob_token, ctx.model).await["torqueSpecs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);

    let (status, body) = call(
        &ctx,
        Method::POST,
        format!("/api/motorcycles/{target}/torque-specs/import-catalog"),
        &bob_token,
        Some(json!({ "catalogSpecIds": ids })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["count"], 1);
    assert_eq!(body["skipped"], 1);

    let row: (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT name, toolSize, descriptionMarkup, updatedAt, clientId FROM torqueSpecs \
         WHERE motorcycleId = ? AND name = 'Ölablass'",
    )
    .bind(target)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(row.1.as_deref(), Some("SW 13"));
    assert_eq!(row.2.as_deref(), Some("**fest**"));
    assert!(row.3.is_some(), "updatedAt set so the sync pull sees it");
    assert!(
        row.4.is_none(),
        "clientId left to the client like the own-bike import"
    );

    // Importing again only skips.
    let (_, again) = call(
        &ctx,
        Method::POST,
        format!("/api/motorcycles/{target}/torque-specs/import-catalog"),
        &bob_token,
        Some(json!({ "catalogSpecIds": ids })),
    )
    .await;
    assert_eq!(again["count"], 0);
    assert_eq!(again["skipped"], 2);
}

#[tokio::test]
async fn import_into_foreign_bike_is_not_found() {
    let ctx = setup().await;
    let (alice, alice_token) = user(&ctx, "alice", "user").await;
    let (_, bob_token) = user(&ctx, "bob", "user").await;
    let bike_id = bike(&ctx, alice, Some(ctx.series)).await;
    let (status, _) = call(
        &ctx,
        Method::POST,
        format!("/api/motorcycles/{bike_id}/torque-specs/import-catalog"),
        &bob_token,
        Some(json!({ "catalogSpecIds": [1] })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let _ = alice_token;
}

#[tokio::test]
async fn only_owner_or_admin_removes_contribution() {
    let ctx = setup().await;
    let (alice, alice_token) = user(&ctx, "alice", "user").await;
    let (_, bob_token) = user(&ctx, "bob", "user").await;
    let (_, admin_token) = user(&ctx, "root", "admin").await;
    let bike_id = bike(&ctx, alice, Some(ctx.series)).await;
    spec(&ctx, bike_id, "Motor", "Zylinderkopf", 20.0).await;
    share(&ctx, &alice_token, bike_id).await;

    let uri = |q: &str| format!("/api/model-series/{}/torque-specs{q}", ctx.series);
    // Another user can't remove alice's rows, and has none of their own.
    assert_eq!(
        call(
            &ctx,
            Method::DELETE,
            uri(&format!("?userId={alice}")),
            &bob_token,
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&ctx, Method::DELETE, uri(""), &bob_token, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    // The admin can.
    assert_eq!(
        call(
            &ctx,
            Method::DELETE,
            uri(&format!("?userId={alice}")),
            &admin_token,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(catalog(&ctx, &alice_token, ctx.series).await["torqueSpecs"]
        .as_array()
        .unwrap()
        .is_empty());

    // The owner removes their own.
    share(&ctx, &alice_token, bike_id).await;
    assert_eq!(
        call(&ctx, Method::DELETE, uri(""), &alice_token, None)
            .await
            .0,
        StatusCode::OK
    );
    assert!(catalog(&ctx, &alice_token, ctx.series).await["torqueSpecs"]
        .as_array()
        .unwrap()
        .is_empty());
}
