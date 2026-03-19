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
use sqlx::{sqlite::SqlitePoolOptions, Row};
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
    };

    let rp_origin = url::Url::parse("http://localhost:5173").unwrap();
    let builder = webauthn_rs::WebauthnBuilder::new("localhost", &rp_origin).unwrap();
    let webauthn = std::sync::Arc::new(builder.build().unwrap());

    let state = AppState {
        pool: pool.clone(),
        config,
        webauthn,
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
async fn test_health_check() {
    let (app, _, _) = setup_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
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
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_list_motorcycles_empty() {
    let (app, _, token) = setup_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/motorcycles")
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
    assert!(body["motorcycles"].is_array());
    assert_eq!(body["motorcycles"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_list_motorcycles_unauthorized() {
    let (app, _, _) = setup_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/motorcycles")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_motorcycle_lifecycle() {
    let (app, pool, token) = setup_test_app().await;

    // 1. Seed a motorcycle
    let moto_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("BMW")
    .bind("R1250GS")
    .bind(1) // from setup_test_app
    .bind(1000)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    // 2. List motorcycles
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/motorcycles")
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
    assert_eq!(body["motorcycles"].as_array().unwrap().len(), 1);
    assert_eq!(body["motorcycles"][0]["make"], "BMW");

    // 3. Get specific motorcycle
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/motorcycles/{}", moto_id))
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
    assert_eq!(body["motorcycle"]["model"], "R1250GS");
    assert!(body["torqueSpecs"].is_array());

    // 4. Delete motorcycle
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/motorcycles/{}", moto_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 5. Verify deleted
    let count: i64 = sqlx::query("SELECT COUNT(*) FROM motorcycles WHERE id = ?")
        .bind(moto_id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_motorcycle_deletion_file_cleanup() {
    let (app, pool, token) = setup_test_app().await;

    // 1. Create a dummy image file
    tokio::fs::create_dir_all("./test_data/images")
        .await
        .unwrap();
    tokio::fs::create_dir_all("./cache/resized").await.unwrap();

    let filename = "test_bike.webp";
    tokio::fs::write("./test_data/images/test_bike.webp", b"original")
        .await
        .unwrap();
    tokio::fs::write("./cache/resized/test_bike_400x400.webp", b"resized")
        .await
        .unwrap();

    // 2. Seed motorcycle with that image
    let moto_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo, image) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("Suzuki")
    .bind("DR650")
    .bind(1)
    .bind(10000)
    .bind(filename)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    // 3. Delete motorcycle
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/motorcycles/{}", moto_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 4. Verify files are gone
    assert!(!std::path::Path::new("./test_data/images/test_bike.webp").exists());
    assert!(!std::path::Path::new("./cache/resized/test_bike_400x400.webp").exists());

    // Cleanup
    let _ = tokio::fs::remove_dir_all("./test_data").await;
    let _ = tokio::fs::remove_dir_all("./cache").await;
}
#[tokio::test]
async fn test_issue_lifecycle() {
    let (app, pool, token) = setup_test_app().await;

    // 1. Seed a motorcycle
    let moto_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("Honda")
    .bind("CB500X")
    .bind(1)
    .bind(5000)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    // 2. Create an issue
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/motorcycles/{}/issues", moto_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "odo": 5100,
                        "description": "Strange noise from engine",
                        "priority": "high"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    let issue_id = body["issue"]["id"].as_i64().unwrap();

    // 3. List issues
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/motorcycles/{}/issues", moto_id))
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
    assert_eq!(body["issues"].as_array().unwrap().len(), 1);

    // 4. Update issue
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/motorcycles/{}/issues/{}", moto_id, issue_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "status": "in_progress"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 5. Delete issue
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/motorcycles/{}/issues/{}", moto_id, issue_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_maintenance_lifecycle() {
    let (app, pool, token) = setup_test_app().await;

    // 1. Seed a motorcycle
    let moto_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("Yamaha")
    .bind("MT-07")
    .bind(1)
    .bind(2000)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    // 2. Create a maintenance record
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/motorcycles/{}/maintenance", moto_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "date": "2026-03-14",
                        "odo": 2500,
                        "type": "oil_change",
                        "description": "Regular maintenance"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    let mid = body["maintenanceRecord"]["id"].as_i64().unwrap();

    // 3. List maintenance records
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/motorcycles/{}/maintenance", moto_id))
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
    assert_eq!(body["maintenanceRecords"].as_array().unwrap().len(), 1);

    // 4. Update maintenance record
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/motorcycles/{}/maintenance/{}", moto_id, mid))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "description": "Oil and filter change"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 5. Delete maintenance record
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/motorcycles/{}/maintenance/{}", moto_id, mid))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_get_motorcycle_not_found() {
    let (app, _, token) = setup_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/motorcycles/999")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_home_data() {
    let (app, pool, token) = setup_test_app().await;

    // 1. Seed data
    let moto_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo, firstRegistration) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("Ducati")
    .bind("Monster")
    .bind(1)
    .bind(1000)
    .bind("2020-01-01")
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    // Add an inspection record
    sqlx::query(
        "INSERT INTO maintenanceRecords (motorcycleId, date, odo, type, description) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(moto_id)
    .bind("2024-03-01")
    .bind(2500)
    .bind("inspection")
    .bind("Regular MFK")
    .execute(&pool)
    .await
    .unwrap();

    // Add an issue
    sqlx::query(
        "INSERT INTO issues (motorcycleId, date, odo, description, priority, status) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(moto_id)
    .bind("2026-03-10")
    .bind(3000)
    .bind("Tire low pressure")
    .bind("medium")
    .bind("open")
    .execute(&pool)
    .await
    .unwrap();

    // Add a motorcycle without any maintenance entries
    sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo, firstRegistration) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("Yamaha")
    .bind("Tenere")
    .bind(1)
    .bind(500)
    .bind("2022-05-01")
    .execute(&pool)
    .await
    .unwrap();

    // 2. Fetch home data
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/home")
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

    assert!(body["motorcycles"].is_array());
    let motos = body["motorcycles"].as_array().unwrap();
    assert_eq!(motos.len(), 2);

    // Check Ducati (with inspection)
    let ducati = motos.iter().find(|m| m["make"] == "Ducati").unwrap();
    assert_eq!(ducati["make"], "Ducati");
    assert_eq!(ducati["numberOfIssues"], 1);
    assert_eq!(ducati["odometer"], 3000);
    assert!(ducati["nextInspection"].is_object());
    assert_eq!(ducati["nextInspection"]["dueDateISO"], "2027-03-01");

    // Check Yamaha (without inspection)
    let yamaha = motos.iter().find(|m| m["make"] == "Yamaha").unwrap();
    assert_eq!(yamaha["make"], "Yamaha");
    assert!(yamaha["nextInspection"].is_null());
}

#[tokio::test]
async fn test_home_data_location_logic() {
    let (app, pool, token) = setup_test_app().await;

    // 1. Seed base data
    let moto_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("BMW")
    .bind("R80GS")
    .bind(1)
    .bind(50000)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    let loc1_id = sqlx::query("INSERT INTO locations (name, countryCode, userId) VALUES (?, ?, ?)")
        .bind("Location 1")
        .bind("CH")
        .bind(1)
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();
    let loc2_id = sqlx::query("INSERT INTO locations (name, countryCode, userId) VALUES (?, ?, ?)")
        .bind("Location 2")
        .bind("DE")
        .bind(1)
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();

    // SCENARIO 1: Only maintenance record has location
    sqlx::query("INSERT INTO maintenanceRecords (motorcycleId, date, odo, type, locationId) VALUES (?, ?, ?, ?, ?)")
        .bind(moto_id).bind("2025-01-01").bind(51000).bind("oil_change").bind(loc1_id).execute(&pool).await.unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/home")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let moto = &body["motorcycles"][0];
    assert_eq!(moto["currentLocationId"], loc1_id);
    assert_eq!(moto["currentLocationName"], "Location 1");

    // SCENARIO 2: Add a newer location record
    sqlx::query("INSERT INTO locationRecords (motorcycleId, locationId, date, odometer) VALUES (?, ?, ?, ?)")
        .bind(moto_id).bind(loc2_id).bind("2025-02-01").bind(52000).execute(&pool).await.unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/home")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let moto = &body["motorcycles"][0];
    assert_eq!(moto["currentLocationId"], loc2_id);
    assert_eq!(moto["currentLocationName"], "Location 2");

    // SCENARIO 3: Add an even newer maintenance record with a different location
    sqlx::query("INSERT INTO maintenanceRecords (motorcycleId, date, odo, type, locationId) VALUES (?, ?, ?, ?, ?)")
        .bind(moto_id).bind("2025-03-01").bind(53000).bind("tire").bind(loc1_id).execute(&pool).await.unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/home")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let moto = &body["motorcycles"][0];
    assert_eq!(moto["currentLocationId"], loc1_id);
    assert_eq!(moto["currentLocationName"], "Location 1");
}
