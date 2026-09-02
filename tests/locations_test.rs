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
use serde_json::{json, Value};
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
    .bind("alice@example.com")
    .bind("alice")
    .bind("Alice")
    .bind(password_hash)
    .bind("user")
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    let token = create_session(&pool, user_id).await.unwrap();
    (build_app(state), pool, token)
}

/// Create a second user, return their bearer token.
async fn create_other_user(pool: &sqlx::SqlitePool) -> String {
    let password_hash = hash_password("password123").unwrap();
    let user_id = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("bob@example.com")
    .bind("bob")
    .bind("Bob")
    .bind(password_hash)
    .bind("user")
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid();
    create_session(pool, user_id).await.unwrap()
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_location_crud_with_type_and_coords() {
    let (app, _pool, token) = setup_test_app().await;

    // Create with type + coords
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/locations")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Home Garage",
                        "type": "storage",
                        "latitude": 47.3769,
                        "longitude": 8.5417
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = read_json(response).await;
    let lid = body["location"]["id"].as_i64().unwrap();
    assert_eq!(body["location"]["type"], "storage");
    assert_eq!(body["location"]["latitude"], 47.3769);
    assert_eq!(body["location"]["longitude"], 8.5417);

    // Update: change type, drop nothing
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/locations/{}", lid))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "type": "maintenanceShop" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["location"]["type"], "maintenanceShop");
    assert_eq!(body["location"]["name"], "Home Garage");
    assert_eq!(body["location"]["latitude"], 47.3769); // preserved
    assert!(body["location"]["updatedAt"].is_string());

    // Delete
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/locations/{}", lid))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_location_coord_validation() {
    let (app, _pool, token) = setup_test_app().await;

    let post = |payload: Value| {
        let app = app.clone();
        let token = token.clone();
        async move {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/locations")
                    .header(header::AUTHORIZATION, format!("Bearer {}", token))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap()
        }
    };

    // Latitude out of range
    let r = post(json!({
        "name": "Bad Lat",
        "type": "other",
        "latitude": 95.0,
        "longitude": 0.0
    }))
    .await;
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);

    // Longitude out of range
    let r = post(json!({
        "name": "Bad Lon",
        "type": "other",
        "latitude": 0.0,
        "longitude": -200.0
    }))
    .await;
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);

    // Half-pair (latitude without longitude)
    let r = post(json!({
        "name": "Half",
        "type": "other",
        "latitude": 10.0
    }))
    .await;
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);

    // No coords at all — accepted
    let r = post(json!({
        "name": "Bare",
        "type": "other"
    }))
    .await;
    assert_eq!(r.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_location_type_filter() {
    let (app, _pool, token) = setup_test_app().await;

    for (name, t) in [
        ("Home", "storage"),
        ("Shop", "maintenanceShop"),
        ("Pump", "fuelStation"),
    ] {
        let r = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/locations")
                    .header(header::AUTHORIZATION, format!("Bearer {}", token))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({"name": name, "type": t})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::CREATED);
    }

    // Filter to storage + fuelStation only
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/locations?types=storage,fuelStation")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = read_json(r).await;
    let names: Vec<&str> = body["locations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Home", "Pump"]);

    // Unknown type → 400
    let r = app
        .oneshot(
            Request::builder()
                .uri("/api/locations?types=spaceport")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_location_user_isolation() {
    let (app, pool, alice_token) = setup_test_app().await;
    let bob_token = create_other_user(&pool).await;

    // Alice creates a location
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/locations")
                .header(header::AUTHORIZATION, format!("Bearer {}", alice_token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "Alice Garage", "type": "storage"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = read_json(r).await;
    let alice_lid = body["location"]["id"].as_i64().unwrap();

    // Bob lists locations — should be empty
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/locations")
                .header(header::AUTHORIZATION, format!("Bearer {}", bob_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = read_json(r).await;
    assert!(body["locations"].as_array().unwrap().is_empty());

    // Bob cannot update Alice's location
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/locations/{}", alice_lid))
                .header(header::AUTHORIZATION, format!("Bearer {}", bob_token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "Pwned"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);

    // Bob cannot delete Alice's location
    let r = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/locations/{}", alice_lid))
                .header(header::AUTHORIZATION, format!("Bearer {}", bob_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_maintenance_rejects_foreign_location_id() {
    let (app, pool, alice_token) = setup_test_app().await;
    let _bob_token = create_other_user(&pool).await;

    // Alice has a motorcycle
    let moto_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("Honda")
    .bind("CB500")
    .bind(1) // alice
    .bind(0)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    // Bob has a location (userId = 2)
    let bob_lid = sqlx::query("INSERT INTO locations (name, type, userId) VALUES (?, ?, ?)")
        .bind("Bob Shop")
        .bind("maintenance_shop")
        .bind(2)
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();

    // Alice tries to attach a maintenance record to Bob's location — should fail
    let r = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/motorcycles/{}/maintenance", moto_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", alice_token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "date": "2026-05-25",
                        "odo": 1000,
                        "type": "oil_change",
                        "locationId": bob_lid
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_location_proximity_search() {
    let (app, _pool, token) = setup_test_app().await;

    async fn create_station(
        app: &axum::Router,
        token: &str,
        name: &str,
        coords: Option<(f64, f64)>,
    ) {
        let mut body = json!({ "name": name, "type": "fuelStation" });
        if let Some((lat, lon)) = coords {
            body["latitude"] = json!(lat);
            body["longitude"] = json!(lon);
        }
        let r = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/locations")
                    .header(header::AUTHORIZATION, format!("Bearer {}", token))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::CREATED);
    }

    // Zürich HB; ~1.5 km away; and one without coordinates.
    create_station(&app, &token, "Near", Some((47.3769, 8.5417))).await;
    create_station(&app, &token, "Far", Some((47.3880, 8.5300))).await;
    create_station(&app, &token, "NoCoords", None).await;

    let names = |body: &Value| -> Vec<String> {
        body["locations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["name"].as_str().unwrap().to_string())
            .collect()
    };

    // 300 m radius around "Near" → only "Near" (Far too far, NoCoords excluded).
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/locations?types=fuelStation&lat=47.3769&lon=8.5417&radius=300")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(names(&read_json(r).await), vec!["Near"]);

    // Wider radius → both, nearest first.
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/locations?lat=47.3769&lon=8.5417&radius=5000")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(names(&read_json(r).await), vec!["Near", "Far"]);

    // Half-pair coordinates → 400.
    let r = app
        .oneshot(
            Request::builder()
                .uri("/api/locations?lat=47.3769")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
}

/// Helper: create a location via the API and return its id.
async fn create_location_via_api(
    app: &axum::Router,
    token: &str,
    name: &str,
    location_type: &str,
) -> i64 {
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/locations")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": name, "type": location_type})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    read_json(r).await["location"]["id"].as_i64().unwrap()
}

#[tokio::test]
async fn test_location_merge_reassigns_references_and_deletes_duplicates() {
    let (app, pool, token) = setup_test_app().await;

    let (user_id,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE username = 'alice'")
        .fetch_one(&pool)
        .await
        .unwrap();

    // A motorcycle to hang the maintenance/location records off of.
    let moto_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("BMW")
    .bind("R80")
    .bind(user_id)
    .bind(0)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    let canonical_id = create_location_via_api(&app, &token, "Shell Zürich", "fuelStation").await;
    let dup_id = create_location_via_api(&app, &token, "Shell Zürich", "fuelStation").await;

    // A fuel record and a "bike is here" marker, both pointing at the duplicate.
    let rec_id = sqlx::query(
        "INSERT INTO maintenanceRecords (date, odo, motorcycleId, type, locationId) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("2026-07-14")
    .bind(1000)
    .bind(moto_id)
    .bind("fuel")
    .bind(dup_id)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    sqlx::query("INSERT INTO locationRecords (motorcycleId, locationId, date) VALUES (?, ?, ?)")
        .bind(moto_id)
        .bind(dup_id)
        .bind("2026-07-14")
        .execute(&pool)
        .await
        .unwrap();

    // Merge the duplicate into the canonical location.
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/locations/merge")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "canonicalId": canonical_id,
                        "duplicateIds": [dup_id],
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(read_json(r).await["merged"], 1);

    // Duplicate is gone, canonical survives.
    let (dup_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM locations WHERE id = ?")
        .bind(dup_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(dup_count, 0);
    let (canon_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM locations WHERE id = ?")
        .bind(canonical_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(canon_count, 1);

    // The fuel record kept its link — now to the canonical station (not NULL).
    let (rec_loc,): (Option<i64>,) =
        sqlx::query_as("SELECT locationId FROM maintenanceRecords WHERE id = ?")
            .bind(rec_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rec_loc, Some(canonical_id));

    // The location marker was reassigned, not deleted.
    let (marker_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM locationRecords WHERE locationId = ?")
            .bind(canonical_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(marker_count, 1);
}

#[tokio::test]
async fn test_location_merge_rejects_foreign_and_empty() {
    let (app, pool, token) = setup_test_app().await;
    let other_token = create_other_user(&pool).await;

    let canonical_id = create_location_via_api(&app, &token, "Alice Canonical", "storage").await;
    let dup_id = create_location_via_api(&app, &token, "Alice Dup", "storage").await;
    let bob_id = create_location_via_api(&app, &other_token, "Bob Place", "storage").await;

    let merge = |tok: String, payload: Value| {
        let app = app.clone();
        async move {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/locations/merge")
                    .header(header::AUTHORIZATION, format!("Bearer {}", tok))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap()
        }
    };

    // A duplicate owned by another user → 404, and nothing is deleted.
    let r = merge(
        token.clone(),
        json!({ "canonicalId": canonical_id, "duplicateIds": [bob_id] }),
    )
    .await;
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
    let (bob_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM locations WHERE id = ?")
        .bind(bob_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(bob_count, 1);

    // Only the canonical id in the duplicate set → nothing to merge → 400.
    let r = merge(
        token.clone(),
        json!({ "canonicalId": canonical_id, "duplicateIds": [canonical_id] }),
    )
    .await;
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);

    // The valid duplicate is untouched by the failed attempts above.
    let (dup_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM locations WHERE id = ?")
        .bind(dup_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(dup_count, 1);
}

#[tokio::test]
async fn test_delete_location_detaches_storage_location() {
    let (app, pool, token) = setup_test_app().await;

    let (user_id,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE username = 'alice'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let place_id = create_location_via_api(&app, &token, "Home Garage", "storage").await;

    // A part storage container anchored to that place.
    let storage_id =
        sqlx::query("INSERT INTO storageLocations (userId, name, locationId) VALUES (?, ?, ?)")
            .bind(user_id)
            .bind("Shelf A")
            .bind(place_id)
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();

    // Deleting the place must not trip the FK check.
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/locations/{}", place_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    // The container survives with its place link detached.
    let (loc,): (Option<i64>,) =
        sqlx::query_as("SELECT locationId FROM storageLocations WHERE id = ?")
            .bind(storage_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(loc, None);
}
