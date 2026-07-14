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
