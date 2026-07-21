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
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use tower::ServiceExt;

async fn setup_test_app() -> (axum::Router, String, i64) {
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

    let user_id = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("owner@example.com")
    .bind("owner")
    .bind("Owner")
    .bind(hash_password("password123").unwrap())
    .bind("user")
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let token = create_session(&pool, user_id).await.unwrap();
    let motorcycle_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("BMW")
    .bind("R 80 G/S")
    .bind(user_id)
    .bind(1_000)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    (build_app(state), token, motorcycle_id)
}

async fn json_request(
    app: &axum::Router,
    token: &str,
    method: Method,
    uri: String,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

async fn create_owner(
    app: &axum::Router,
    token: &str,
    motorcycle_id: i64,
    name: &str,
    purchase_date: Option<&str>,
) -> Value {
    let (status, body) = json_request(
        app,
        token,
        Method::POST,
        format!("/api/motorcycles/{motorcycle_id}/previous-owners"),
        json!({
            "name": name,
            "surname": "Test",
            "purchaseDate": purchase_date,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["previousOwner"].clone()
}

#[tokio::test]
async fn nullable_purchase_date_and_manual_order_round_trip() {
    let (app, token, motorcycle_id) = setup_test_app().await;
    let first = create_owner(&app, &token, motorcycle_id, "First", Some("1990")).await;
    let second = create_owner(&app, &token, motorcycle_id, "Second", None).await;

    assert_eq!(first["sortOrder"], 0);
    assert_eq!(second["sortOrder"], 1);
    assert!(second["purchaseDate"].is_null());

    let first_id = first["id"].as_i64().unwrap();
    let second_id = second["id"].as_i64().unwrap();
    let (status, reordered) = json_request(
        &app,
        &token,
        Method::PUT,
        format!("/api/motorcycles/{motorcycle_id}/previous-owners/order"),
        json!({ "ownerIds": [second_id, first_id] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reordered}");
    assert_eq!(reordered["previousOwners"][0]["id"], second_id);
    assert_eq!(reordered["previousOwners"][0]["sortOrder"], 0);
    assert_eq!(reordered["previousOwners"][1]["id"], first_id);
    assert_eq!(reordered["previousOwners"][1]["sortOrder"], 1);

    let (status, detail) = json_request(
        &app,
        &token,
        Method::GET,
        format!("/api/motorcycles/{motorcycle_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["previousOwners"][0]["id"], second_id);

    let (status, cleared) = json_request(
        &app,
        &token,
        Method::PUT,
        format!("/api/motorcycles/{motorcycle_id}/previous-owners/{first_id}"),
        json!({ "purchaseDate": null }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    assert!(cleared["previousOwner"]["purchaseDate"].is_null());

    let (status, renamed) = json_request(
        &app,
        &token,
        Method::PUT,
        format!("/api/motorcycles/{motorcycle_id}/previous-owners/{first_id}"),
        json!({ "name": "Renamed" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{renamed}");
    assert!(renamed["previousOwner"]["purchaseDate"].is_null());
}

#[tokio::test]
async fn reorder_requires_the_complete_unique_owner_set() {
    let (app, token, motorcycle_id) = setup_test_app().await;
    let first = create_owner(&app, &token, motorcycle_id, "First", None).await;
    let second = create_owner(&app, &token, motorcycle_id, "Second", None).await;
    let first_id = first["id"].as_i64().unwrap();
    let second_id = second["id"].as_i64().unwrap();

    for owner_ids in [
        json!([first_id]),
        json!([first_id, first_id]),
        json!([first_id, 999_999]),
    ] {
        let (status, _) = json_request(
            &app,
            &token,
            Method::PUT,
            format!("/api/motorcycles/{motorcycle_id}/previous-owners/order"),
            json!({ "ownerIds": owner_ids }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    let (status, listed) = json_request(
        &app,
        &token,
        Method::GET,
        format!("/api/motorcycles/{motorcycle_id}/previous-owners"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["previousOwners"][0]["id"], first_id);
    assert_eq!(listed["previousOwners"][1]["id"], second_id);
}

#[tokio::test]
async fn migration_preserves_the_legacy_date_order() {
    let pool: SqlitePool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE motorcycles (id INTEGER PRIMARY KEY);\
         CREATE TABLE previousOwners (\
           id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, motorcycleId INTEGER NOT NULL,\
           name TEXT NOT NULL, surname TEXT NOT NULL, purchaseDate TEXT NOT NULL,\
           address TEXT, city TEXT, postcode TEXT, country TEXT, phoneNumber TEXT,\
           email TEXT, comments TEXT, createdAt TEXT NOT NULL, updatedAt TEXT NOT NULL\
         );\
         INSERT INTO motorcycles (id) VALUES (1);\
         INSERT INTO previousOwners\
           (id, motorcycleId, name, surname, purchaseDate, createdAt, updatedAt) VALUES\
           (1, 1, 'Oldest', 'Owner', '1990', 'now', 'now'),\
           (2, 1, 'Same date lower id', 'Owner', '2020', 'now', 'now'),\
           (3, 1, 'Newest', 'Owner', '2020', 'now', 'now');",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::raw_sql(include_str!("../migrations/045_previous_owner_order.sql"))
        .execute(&pool)
        .await
        .unwrap();

    let rows: Vec<(i64, i64)> =
        sqlx::query_as("SELECT id, sortOrder FROM previousOwners ORDER BY sortOrder")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(rows, vec![(3, 0), (2, 1), (1, 2)]);

    sqlx::query("UPDATE previousOwners SET purchaseDate = NULL WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
}
