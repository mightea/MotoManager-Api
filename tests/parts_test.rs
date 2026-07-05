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
    };

    let rp_origin = url::Url::parse("http://localhost:5173").unwrap();
    let builder = webauthn_rs::WebauthnBuilder::new("localhost", &rp_origin).unwrap();
    let webauthn = std::sync::Arc::new(builder.build().unwrap());

    let state = AppState {
        pool: pool.clone(),
        config,
        webauthn,
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

async fn create_second_user(pool: &sqlx::SqlitePool) -> (i64, String) {
    let password_hash = hash_password("password456").unwrap();
    let user_id = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("second@example.com")
    .bind("seconduser")
    .bind("Second User")
    .bind(password_hash)
    .bind("user")
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let token = create_session(pool, user_id).await.unwrap();
    (user_id, token)
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

async fn seed_series_id(pool: &sqlx::SqlitePool, name: &str) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT id FROM modelSeries WHERE name = ? AND userId IS NULL")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn seed_motorcycle(pool: &sqlx::SqlitePool, user_id: i64) -> i64 {
    sqlx::query("INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)")
        .bind("BMW")
        .bind("R 1150 GS")
        .bind(user_id)
        .bind(1000)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid()
}

#[tokio::test]
async fn test_parts_lifecycle() {
    let (app, pool, token) = setup_test_app().await;
    let series_gs = seed_series_id(&pool, "R 1150 GS").await;
    let series_r = seed_series_id(&pool, "R 1100 GS").await;

    // Create a part with fitment.
    let (status, body) = request(
        &app,
        Method::POST,
        "/api/parts",
        &token,
        Some(json!({
            "partNumber": "11 42 7 673 541",
            "name": "Ölfilter",
            "seriesIds": [series_gs, series_r],
            "clientId": "part-client-1"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let part_id = body["part"]["id"].as_i64().unwrap();
    assert_eq!(body["part"]["manufacturer"], "BMW");
    assert_eq!(body["part"]["onHand"], 0);
    assert_eq!(body["part"]["stockCount"], 0);
    assert_eq!(
        body["part"]["seriesIds"].as_array().unwrap().len(),
        2,
        "{body}"
    );

    // Add stock: 3 pieces with price and purchase date.
    let (status, body) = request(
        &app,
        Method::POST,
        "/api/part-stocks",
        &token,
        Some(json!({
            "partId": part_id,
            "quantity": 3,
            "price": 14.90,
            "currency": "CHF",
            "purchaseDate": "2026-06-01",
            "clientId": "stock-client-1"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let stock_id = body["partStock"]["id"].as_i64().unwrap();

    // List shows derived on-hand.
    let (status, body) = request(&app, Method::GET, "/api/parts", &token, None).await;
    assert_eq!(status, StatusCode::OK);
    let parts = body["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0]["onHand"], 3);
    assert_eq!(parts[0]["stockCount"], 1);

    // Consume 2.
    let (status, body) = request(
        &app,
        Method::POST,
        "/api/part-consumptions",
        &token,
        Some(json!({ "partId": part_id, "quantity": 2, "date": "2026-06-15" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (_, body) = request(&app, Method::GET, "/api/parts", &token, None).await;
    assert_eq!(body["parts"][0]["onHand"], 1);

    // Update the part (rename + shrink fitment).
    let (status, body) = request(
        &app,
        Method::PUT,
        &format!("/api/parts/{part_id}"),
        &token,
        Some(json!({ "name": "Ölfilter Mahle", "seriesIds": [series_gs] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["part"]["name"], "Ölfilter Mahle");
    assert_eq!(body["part"]["seriesIds"].as_array().unwrap().len(), 1);

    // Delete the part: soft cascade to stock + consumption.
    let (status, _) = request(
        &app,
        Method::DELETE,
        &format!("/api/parts/{part_id}"),
        &token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, body) = request(&app, Method::GET, "/api/parts", &token, None).await;
    assert_eq!(body["parts"].as_array().unwrap().len(), 0);

    // ?since from epoch surfaces all three tombstones.
    let (_, body) = request(&app, Method::GET, "/api/parts?since=0", &token, None).await;
    let tombstoned = body["parts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| !p["deletedAt"].is_null())
        .count();
    assert_eq!(tombstoned, 1);

    let (_, body) = request(&app, Method::GET, "/api/part-stocks?since=0", &token, None).await;
    assert_eq!(body["partStocks"][0]["id"], stock_id);
    assert!(!body["partStocks"][0]["deletedAt"].is_null());

    let (_, body) = request(
        &app,
        Method::GET,
        "/api/part-consumptions?since=0",
        &token,
        None,
    )
    .await;
    assert!(!body["partConsumptions"][0]["deletedAt"].is_null());
}

#[tokio::test]
async fn test_part_clientid_idempotency() {
    let (app, _pool, token) = setup_test_app().await;

    let payload = json!({
        "partNumber": "34 11 2 335 465",
        "name": "Bremsbeläge vorn",
        "clientId": "idempotent-client-1"
    });
    let (status1, body1) = request(&app, Method::POST, "/api/parts", &token, Some(payload.clone())).await;
    let (status2, body2) = request(&app, Method::POST, "/api/parts", &token, Some(payload)).await;

    assert_eq!(status1, StatusCode::CREATED);
    assert_eq!(status2, StatusCode::CREATED);
    assert_eq!(body1["part"]["id"], body2["part"]["id"]);

    let (_, body) = request(&app, Method::GET, "/api/parts", &token, None).await;
    assert_eq!(body["parts"].as_array().unwrap().len(), 1);

    // Same identity without clientId is rejected as a duplicate.
    let (status, _) = request(
        &app,
        Method::POST,
        "/api/parts",
        &token,
        Some(json!({ "partNumber": "34 11 2 335 465", "name": "Bremsbeläge vorn" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_consumption_rejects_overdraw() {
    let (app, _pool, token) = setup_test_app().await;

    let (_, body) = request(
        &app,
        Method::POST,
        "/api/parts",
        &token,
        Some(json!({ "partNumber": "PN-1", "name": "Zündkerze" })),
    )
    .await;
    let part_id = body["part"]["id"].as_i64().unwrap();

    let (_, _) = request(
        &app,
        Method::POST,
        "/api/part-stocks",
        &token,
        Some(json!({ "partId": part_id, "quantity": 1 })),
    )
    .await;

    // Consuming more than on-hand is rejected.
    let (status, body) = request(
        &app,
        Method::POST,
        "/api/part-consumptions",
        &token,
        Some(json!({ "partId": part_id, "quantity": 2 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // Exactly on-hand is fine.
    let (status, body) = request(
        &app,
        Method::POST,
        "/api/part-consumptions",
        &token,
        Some(json!({ "partId": part_id, "quantity": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let consumption_id = body["partConsumption"]["id"].as_i64().unwrap();

    // Now empty: another consumption is rejected.
    let (status, _) = request(
        &app,
        Method::POST,
        "/api/part-consumptions",
        &token,
        Some(json!({ "partId": part_id, "quantity": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Growing an existing consumption past on-hand is rejected too.
    let (status, _) = request(
        &app,
        Method::PUT,
        &format!("/api/part-consumptions/{consumption_id}"),
        &token,
        Some(json!({ "quantity": 2 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Deleting the consumption restores stock by derivation.
    let (status, _) = request(
        &app,
        Method::DELETE,
        &format!("/api/part-consumptions/{consumption_id}"),
        &token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = request(&app, Method::GET, "/api/parts", &token, None).await;
    assert_eq!(body["parts"][0]["onHand"], 1);
}

#[tokio::test]
async fn test_maintenance_delete_restores_stock() {
    let (app, pool, token) = setup_test_app().await;
    let moto_id = seed_motorcycle(&pool, 1).await;

    // Repair record via API.
    let (status, body) = request(
        &app,
        Method::POST,
        &format!("/api/motorcycles/{moto_id}/maintenance"),
        &token,
        Some(json!({ "date": "2026-06-20", "odo": 42000, "type": "general" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let record_id = body["maintenanceRecord"]["id"].as_i64().unwrap();

    let (_, body) = request(
        &app,
        Method::POST,
        "/api/parts",
        &token,
        Some(json!({ "partNumber": "PN-2", "name": "Luftfilter" })),
    )
    .await;
    let part_id = body["part"]["id"].as_i64().unwrap();
    request(
        &app,
        Method::POST,
        "/api/part-stocks",
        &token,
        Some(json!({ "partId": part_id, "quantity": 2 })),
    )
    .await;

    // Consume against the repair; date defaults to the record's date.
    let (status, body) = request(
        &app,
        Method::POST,
        "/api/part-consumptions",
        &token,
        Some(json!({ "partId": part_id, "quantity": 2, "maintenanceRecordId": record_id })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["partConsumption"]["date"], "2026-06-20");

    let (_, body) = request(&app, Method::GET, "/api/parts", &token, None).await;
    assert_eq!(body["parts"][0]["onHand"], 0);

    // Deleting the repair tombstones the consumption and restores on-hand.
    let (status, _) = request(
        &app,
        Method::DELETE,
        &format!("/api/motorcycles/{moto_id}/maintenance/{record_id}"),
        &token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, body) = request(&app, Method::GET, "/api/parts", &token, None).await;
    assert_eq!(body["parts"][0]["onHand"], 2);

    let (_, body) = request(
        &app,
        Method::GET,
        "/api/part-consumptions?since=0",
        &token,
        None,
    )
    .await;
    let consumptions = body["partConsumptions"].as_array().unwrap();
    assert_eq!(consumptions.len(), 1);
    assert!(!consumptions[0]["deletedAt"].is_null());
}

#[tokio::test]
async fn test_public_visibility() {
    let (app, pool, token_a) = setup_test_app().await;
    let (_user_b, token_b) = create_second_user(&pool).await;
    let series = seed_series_id(&pool, "K 75").await;

    // User A: one public part with stock, one private part.
    let (_, body) = request(
        &app,
        Method::POST,
        "/api/parts",
        &token_a,
        Some(json!({
            "partNumber": "PUB-1",
            "name": "Tachowelle",
            "isPublic": true,
            "seriesIds": [series]
        })),
    )
    .await;
    let public_part_id = body["part"]["id"].as_i64().unwrap();
    request(
        &app,
        Method::POST,
        "/api/part-stocks",
        &token_a,
        Some(json!({
            "partId": public_part_id,
            "quantity": 3,
            "price": 49.0,
            "currency": "CHF",
            "purchaseDate": "2026-01-01"
        })),
    )
    .await;
    request(
        &app,
        Method::POST,
        "/api/parts",
        &token_a,
        Some(json!({ "partNumber": "PRIV-1", "name": "Geheimteil" })),
    )
    .await;

    // User B browses public parts: sees only the public one, with availability.
    let (status, body) = request(&app, Method::GET, "/api/parts/public", &token_b, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let parts = body["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 1);
    let p = &parts[0];
    assert_eq!(p["partNumber"], "PUB-1");
    assert_eq!(p["ownerName"], "testuser");
    assert_eq!(p["hasStock"], true);
    assert_eq!(p["totalQuantity"], 3);
    assert_eq!(p["seriesIds"][0], series);
    // Whitelist projection: no price/purchase/location keys may leak.
    let obj = p.as_object().unwrap();
    assert!(!obj.contains_key("price"));
    assert!(!obj.contains_key("purchaseDate"));
    assert!(!obj.contains_key("storageLocationId"));

    // The owner's own public parts are excluded from their browse view.
    let (_, body) = request(&app, Method::GET, "/api/parts/public", &token_a, None).await;
    assert_eq!(body["parts"].as_array().unwrap().len(), 0);

    // Search filter matches.
    let (_, body) = request(
        &app,
        Method::GET,
        "/api/parts/public?query=Tacho",
        &token_b,
        None,
    )
    .await;
    assert_eq!(body["parts"].as_array().unwrap().len(), 1);
    let (_, body) = request(
        &app,
        Method::GET,
        "/api/parts/public?query=Nichts",
        &token_b,
        None,
    )
    .await;
    assert_eq!(body["parts"].as_array().unwrap().len(), 0);

    // B cannot see or mutate A's parts through the owner endpoints (masked 404).
    let (_, body) = request(&app, Method::GET, "/api/parts", &token_b, None).await;
    assert_eq!(body["parts"].as_array().unwrap().len(), 0);
    let (status, _) = request(
        &app,
        Method::PUT,
        &format!("/api/parts/{public_part_id}"),
        &token_b,
        Some(json!({ "name": "Hijacked" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = request(
        &app,
        Method::POST,
        "/api/part-stocks",
        &token_b,
        Some(json!({ "partId": public_part_id, "quantity": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_storage_location_hierarchy() {
    let (app, _pool, token) = setup_test_app().await;

    // Garage > Regal A > Kiste 3
    let (_, body) = request(
        &app,
        Method::POST,
        "/api/storage-locations",
        &token,
        Some(json!({ "name": "Garage" })),
    )
    .await;
    let garage = body["storageLocation"]["id"].as_i64().unwrap();
    let (_, body) = request(
        &app,
        Method::POST,
        "/api/storage-locations",
        &token,
        Some(json!({ "name": "Regal A", "parentId": garage })),
    )
    .await;
    let regal = body["storageLocation"]["id"].as_i64().unwrap();
    let (_, body) = request(
        &app,
        Method::POST,
        "/api/storage-locations",
        &token,
        Some(json!({ "name": "Kiste 3", "parentId": regal })),
    )
    .await;
    let kiste = body["storageLocation"]["id"].as_i64().unwrap();

    // Cycle rejection: Garage cannot move under Kiste 3.
    let (status, _) = request(
        &app,
        Method::PUT,
        &format!("/api/storage-locations/{garage}"),
        &token,
        Some(json!({ "parentId": kiste })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Stock stored in the middle node.
    let (_, body) = request(
        &app,
        Method::POST,
        "/api/parts",
        &token,
        Some(json!({ "partNumber": "PN-3", "name": "Dichtung" })),
    )
    .await;
    let part_id = body["part"]["id"].as_i64().unwrap();
    let (_, body) = request(
        &app,
        Method::POST,
        "/api/part-stocks",
        &token,
        Some(json!({ "partId": part_id, "quantity": 1, "storageLocationId": regal })),
    )
    .await;
    let stock_id = body["partStock"]["id"].as_i64().unwrap();
    let stock_updated_at = body["partStock"]["updatedAt"].as_str().unwrap().to_string();

    // Delete the middle node: child reparents to Garage, stock detaches.
    let (status, _) = request(
        &app,
        Method::DELETE,
        &format!("/api/storage-locations/{regal}"),
        &token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, body) = request(&app, Method::GET, "/api/storage-locations", &token, None).await;
    let locations = body["storageLocations"].as_array().unwrap();
    assert_eq!(locations.len(), 2);
    let kiste_row = locations
        .iter()
        .find(|l| l["id"].as_i64() == Some(kiste))
        .unwrap();
    assert_eq!(kiste_row["parentId"].as_i64(), Some(garage));

    let (_, body) = request(&app, Method::GET, "/api/part-stocks", &token, None).await;
    let stock_row = body["partStocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"].as_i64() == Some(stock_id))
        .unwrap()
        .clone();
    assert!(stock_row["storageLocationId"].is_null());
    // updatedAt was bumped so offline clients pull the detachment.
    assert!(stock_row["updatedAt"].as_str().unwrap() > stock_updated_at.as_str());
}

#[tokio::test]
async fn test_storage_location_place_link() {
    let (app, pool, token) = setup_test_app().await;

    // A workshop place from the existing locations entity.
    let place_id = sqlx::query(
        "INSERT INTO locations (name, userId, type) VALUES (?, ?, 'storage')",
    )
    .bind("Garage Zuhause")
    .bind(1)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    // Root-level storage location can link to the place.
    let (status, body) = request(
        &app,
        Method::POST,
        "/api/storage-locations",
        &token,
        Some(json!({ "name": "Regal A", "locationId": place_id })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let root_id = body["storageLocation"]["id"].as_i64().unwrap();
    assert_eq!(body["storageLocation"]["locationId"].as_i64(), Some(place_id));

    // A nested location must not carry its own place link.
    let (status, _) = request(
        &app,
        Method::POST,
        "/api/storage-locations",
        &token,
        Some(json!({ "name": "Kiste 1", "parentId": root_id, "locationId": place_id })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Explicit null clears the link; absent leaves it untouched.
    let (_, body) = request(
        &app,
        Method::PUT,
        &format!("/api/storage-locations/{root_id}"),
        &token,
        Some(json!({ "name": "Regal A1" })),
    )
    .await;
    assert_eq!(body["storageLocation"]["locationId"].as_i64(), Some(place_id));
    let (_, body) = request(
        &app,
        Method::PUT,
        &format!("/api/storage-locations/{root_id}"),
        &token,
        Some(json!({ "locationId": null })),
    )
    .await;
    assert!(body["storageLocation"]["locationId"].is_null());

    // Nesting a linked root under a parent clears its place link.
    let (_, body) = request(
        &app,
        Method::POST,
        "/api/storage-locations",
        &token,
        Some(json!({ "name": "Keller", "locationId": place_id })),
    )
    .await;
    let linked_id = body["storageLocation"]["id"].as_i64().unwrap();
    let (_, body) = request(
        &app,
        Method::PUT,
        &format!("/api/storage-locations/{linked_id}"),
        &token,
        Some(json!({ "parentId": root_id })),
    )
    .await;
    assert!(body["storageLocation"]["locationId"].is_null(), "{body}");
}

#[tokio::test]
async fn test_model_series_scoping() {
    let (app, pool, token_a) = setup_test_app().await;
    let (_user_b, token_b) = create_second_user(&pool).await;

    // Global seeds are visible to everyone.
    let (_, body_a) = request(&app, Method::GET, "/api/model-series", &token_a, None).await;
    let global_count = body_a["modelSeries"].as_array().unwrap().len();
    assert!(global_count > 30, "seed list expected, got {global_count}");

    // A creates a custom series; only A sees it.
    let (status, body) = request(
        &app,
        Method::POST,
        "/api/model-series",
        &token_a,
        Some(json!({ "name": "XSR 700", "manufacturer": "Yamaha" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let custom_id = body["modelSeries"]["id"].as_i64().unwrap();

    let (_, body) = request(&app, Method::GET, "/api/model-series", &token_a, None).await;
    assert_eq!(body["modelSeries"].as_array().unwrap().len(), global_count + 1);
    let (_, body) = request(&app, Method::GET, "/api/model-series", &token_b, None).await;
    assert_eq!(body["modelSeries"].as_array().unwrap().len(), global_count);

    // Idempotent re-create returns the existing row.
    let (status, body) = request(
        &app,
        Method::POST,
        "/api/model-series",
        &token_a,
        Some(json!({ "name": "XSR 700", "manufacturer": "Yamaha" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["modelSeries"]["id"].as_i64(), Some(custom_id));

    // B cannot edit or delete A's custom series (masked 404).
    let (status, _) = request(
        &app,
        Method::PUT,
        &format!("/api/model-series/{custom_id}"),
        &token_b,
        Some(json!({ "name": "Stolen" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Users curate the catalog: an unused global leaf can be removed…
    let unused_global = seed_series_id(&pool, "R 45").await;
    let (status, _) = request(
        &app,
        Method::DELETE,
        &format!("/api/model-series/{unused_global}"),
        &token_a,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = request(&app, Method::GET, "/api/model-series", &token_b, None).await;
    assert!(body["modelSeries"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| s["name"] != "R 45"));

    // …but a global family whose subtree is referenced anywhere is protected:
    // link a part to the child "K 75", then try to delete the whole family.
    let k75 = seed_series_id(&pool, "K 75").await;
    request(
        &app,
        Method::POST,
        "/api/parts",
        &token_a,
        Some(json!({ "partNumber": "PN-K75", "name": "Tachoritzel", "seriesIds": [k75] })),
    )
    .await;
    let familie = seed_series_id(&pool, "K-Modelle 3-Zyl.").await;
    let (status, _) = request(
        &app,
        Method::DELETE,
        &format!("/api/model-series/{familie}"),
        &token_a,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Referenced custom series cannot be deleted.
    request(
        &app,
        Method::POST,
        "/api/parts",
        &token_a,
        Some(json!({
            "partNumber": "PN-4",
            "name": "Kettensatz",
            "manufacturer": "Yamaha",
            "seriesIds": [custom_id]
        })),
    )
    .await;
    let (status, _) = request(
        &app,
        Method::DELETE,
        &format!("/api/model-series/{custom_id}"),
        &token_a,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // B cannot attach A's custom series to their own part.
    let (status, _) = request(
        &app,
        Method::POST,
        "/api/parts",
        &token_b,
        Some(json!({ "partNumber": "PN-5", "name": "Fremdteil", "seriesIds": [custom_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_part_image_lifecycle() {
    let (app, pool, token) = setup_test_app().await;
    let (_user_b, token_b) = create_second_user(&pool).await;

    let (_, body) = request(
        &app,
        Method::POST,
        "/api/parts",
        &token,
        Some(json!({ "partNumber": "IMG-1", "name": "Ventildeckel", "isPublic": true })),
    )
    .await;
    let part_id = body["part"]["id"].as_i64().unwrap();
    let updated_before = body["part"]["updatedAt"].as_str().unwrap().to_string();

    // Upload a (tiny fake) JPEG via multipart.
    let boundary = "moto-test-boundary";
    let multipart_body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"part.jpg\"\r\nContent-Type: image/jpeg\r\n\r\nfakejpegbytes\r\n--{boundary}--\r\n"
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/parts/{part_id}/image"))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(multipart_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let image_url = body["part"]["image"].as_str().unwrap().to_string();
    assert!(image_url.starts_with("/images/"), "{image_url}");
    // The upload must bump updatedAt so the change syncs to offline clients.
    assert!(body["part"]["updatedAt"].as_str().unwrap() > updated_before.as_str());

    // Image shows in the owner list and in the public browse of another user.
    let (_, body) = request(&app, Method::GET, "/api/parts", &token, None).await;
    assert_eq!(body["parts"][0]["image"].as_str().unwrap(), image_url);
    let (_, body) = request(&app, Method::GET, "/api/parts/public", &token_b, None).await;
    assert_eq!(body["parts"][0]["image"].as_str().unwrap(), image_url);

    // Foreign users cannot upload (masked 404).
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/parts/{part_id}/image"))
                .header(header::AUTHORIZATION, format!("Bearer {}", token_b))
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(format!(
                    "--{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"x.jpg\"\r\nContent-Type: image/jpeg\r\n\r\nx\r\n--{boundary}--\r\n"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Delete clears the column (the image-delete handler also removes the
    // file from ./test_data, so no directory cleanup is needed here — and a
    // remove_dir_all would race with other test binaries sharing that dir).
    let (status, body) = request(
        &app,
        Method::DELETE,
        &format!("/api/parts/{part_id}/image"),
        &token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["part"]["image"].is_null());
}

#[tokio::test]
async fn test_model_series_hierarchy() {
    let (app, pool, token) = setup_test_app().await;

    // Seeded hierarchy: Familie "R-Modelle 2V (1978-1996)" contains the
    // re-parented "R 80 GS" and the Serie "R 80 GS, R 100 GS, PD (90-95)"
    // which contains Modell "R 100 GS (ECE, 04/1990-07/1996)".
    let familie = seed_series_id(&pool, "R-Modelle 2V (1978-1996)").await;
    let serie = seed_series_id(&pool, "R 80 GS, R 100 GS, PD (90-95)").await;
    let modell = seed_series_id(&pool, "R 100 GS (ECE, 04/1990-07/1996)").await;
    let other_familie = seed_series_id(&pool, "K-Modelle 3-Zyl.").await;

    // Depth cap: a child under a Modell (depth 3) is rejected.
    let (status, _) = request(
        &app,
        Method::POST,
        "/api/model-series",
        &token,
        Some(json!({ "name": "Zu tief", "parentId": modell })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Custom child under a global Serie is fine and carries the parent.
    let (status, body) = request(
        &app,
        Method::POST,
        "/api/model-series",
        &token,
        Some(json!({ "name": "R 80 GS PD (CH) (ECE, 08/1991-12/1995)", "parentId": serie })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let custom_id = body["modelSeries"]["id"].as_i64().unwrap();
    assert_eq!(body["modelSeries"]["parentId"].as_i64(), Some(serie));

    // Re-parenting to inside the own subtree is rejected; to root works.
    let (status, _) = request(
        &app,
        Method::PUT,
        &format!("/api/model-series/{custom_id}"),
        &token,
        Some(json!({ "parentId": custom_id })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, body) = request(
        &app,
        Method::PUT,
        &format!("/api/model-series/{custom_id}"),
        &token,
        Some(json!({ "parentId": null })),
    )
    .await;
    assert!(body["modelSeries"]["parentId"].is_null(), "{body}");

    // Hierarchy-aware compatibility: bike on the Serie level.
    let moto_id = seed_motorcycle(&pool, 1).await;
    sqlx::query("UPDATE motorcycles SET seriesId = ? WHERE id = ?")
        .bind(serie)
        .bind(moto_id)
        .execute(&pool)
        .await
        .unwrap();

    // Part linked to the Familie (ancestor) fits; part linked to the Modell
    // (descendant) fits; part linked to another Familie does not.
    for (part_number, series_id) in [
        ("HIER-FAM", familie),
        ("HIER-MOD", modell),
        ("HIER-OTHER", other_familie),
    ] {
        request(
            &app,
            Method::POST,
            "/api/parts",
            &token,
            Some(json!({ "partNumber": part_number, "name": part_number, "seriesIds": [series_id] })),
        )
        .await;
    }

    let (_, body) = request(
        &app,
        Method::GET,
        &format!("/api/parts?motorcycleId={moto_id}"),
        &token,
        None,
    )
    .await;
    let numbers: Vec<String> = body["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["partNumber"].as_str().unwrap().to_string())
        .collect();
    assert!(numbers.contains(&"HIER-FAM".to_string()), "{numbers:?}");
    assert!(numbers.contains(&"HIER-MOD".to_string()), "{numbers:?}");
    assert!(!numbers.contains(&"HIER-OTHER".to_string()), "{numbers:?}");

    // Deleting a node cascades through its (unused) subtree.
    let (_, body) = request(
        &app,
        Method::POST,
        "/api/model-series",
        &token,
        Some(json!({ "name": "Eigene Familie" })),
    )
    .await;
    let own_family = body["modelSeries"]["id"].as_i64().unwrap();
    let (_, body) = request(
        &app,
        Method::POST,
        "/api/model-series",
        &token,
        Some(json!({ "name": "Eigene Serie", "parentId": own_family })),
    )
    .await;
    let own_series = body["modelSeries"]["id"].as_i64().unwrap();
    let (status, _) = request(
        &app,
        Method::DELETE,
        &format!("/api/model-series/{own_family}"),
        &token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = request(&app, Method::GET, "/api/model-series", &token, None).await;
    let remaining_ids: Vec<i64> = body["modelSeries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_i64().unwrap())
        .collect();
    assert!(!remaining_ids.contains(&own_family));
    assert!(!remaining_ids.contains(&own_series));
}

#[tokio::test]
async fn test_vin_decode() {
    let (app, _pool, token) = setup_test_app().await;

    // K 100 RS, ECE type code 0513 (USA variant), model year letter F = 1985.
    // Deepest match wins: the Modell "K 100 RS 83 (0502,0503,0513) (ECE)"
    // sits below the K589 Serie which carries the same code.
    let (status, body) = request(
        &app,
        Method::GET,
        "/api/vin/decode?vin=WB1051300F0042335",
        &token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["isBmw"], true);
    assert_eq!(body["typeCode"], "0513");
    assert_eq!(body["modelYear"], 1985);
    assert_eq!(
        body["match"]["name"].as_str().unwrap(),
        "K 100 RS 83 (0502,0503,0513) (ECE)",
        "{body}"
    );

    // Monolever R 80: code 0453 hits the seeded Modell below the Serie.
    let (_, body) = request(
        &app,
        Method::GET,
        "/api/vin/decode?vin=WB1045300H6123456",
        &token,
        None,
    )
    .await;
    assert_eq!(
        body["match"]["name"].as_str().unwrap(),
        "R 80 (ECE, 03/1984-01/1995)",
        "{body}"
    );
    assert_eq!(body["modelYear"], 1987);

    // Unknown type code: BMW VIN but no catalog match.
    let (_, body) = request(
        &app,
        Method::GET,
        "/api/vin/decode?vin=WB1999900F0042335",
        &token,
        None,
    )
    .await;
    assert_eq!(body["isBmw"], true);
    assert!(body["match"].is_null());

    // Non-BMW WMI: decoded structurally, but never matched.
    let (_, body) = request(
        &app,
        Method::GET,
        "/api/vin/decode?vin=JYA045300F0042335",
        &token,
        None,
    )
    .await;
    assert_eq!(body["isBmw"], false);
    assert!(body["match"].is_null());

    // Malformed VIN is rejected.
    let (status, _) = request(&app, Method::GET, "/api/vin/decode?vin=WB105", &token, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_since_delta() {
    let (app, _pool, token) = setup_test_app().await;

    let (_, body) = request(
        &app,
        Method::POST,
        "/api/parts",
        &token,
        Some(json!({ "partNumber": "PN-6", "name": "Blinkerglas" })),
    )
    .await;
    let part_id = body["part"]["id"].as_i64().unwrap();
    let created_updated_at = body["part"]["updatedAt"].as_str().unwrap().to_string();

    // Cursor at the creation timestamp: nothing newer.
    let uri = format!("/api/parts?since={}", created_updated_at.replace(':', "%3A"));
    let (_, body) = request(&app, Method::GET, &uri, &token, None).await;
    assert_eq!(body["parts"].as_array().unwrap().len(), 0);

    // An update moves the row past the cursor.
    request(
        &app,
        Method::PUT,
        &format!("/api/parts/{part_id}"),
        &token,
        Some(json!({ "description": "rechts" })),
    )
    .await;
    let (_, body) = request(&app, Method::GET, &uri, &token, None).await;
    assert_eq!(body["parts"].as_array().unwrap().len(), 1);
    assert!(body["parts"][0]["deletedAt"].is_null());

    // Soft delete: the tombstone is visible via ?since but hidden from plain lists.
    request(
        &app,
        Method::DELETE,
        &format!("/api/parts/{part_id}"),
        &token,
        None,
    )
    .await;
    let (_, body) = request(&app, Method::GET, &uri, &token, None).await;
    assert_eq!(body["parts"].as_array().unwrap().len(), 1);
    assert!(!body["parts"][0]["deletedAt"].is_null());
    let (_, body) = request(&app, Method::GET, "/api/parts", &token, None).await;
    assert_eq!(body["parts"].as_array().unwrap().len(), 0);
}
