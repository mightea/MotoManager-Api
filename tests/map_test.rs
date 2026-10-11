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

    (build_app(state), pool, token)
}

fn auth(req: Request<Body>, token: &str) -> Request<Body> {
    let (mut parts, body) = req.into_parts();
    parts.headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", token).parse().unwrap(),
    );
    Request::from_parts(parts, body)
}

async fn insert_user(pool: &sqlx::SqlitePool, username: &str) -> i64 {
    sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(format!("{username}@example.com"))
    .bind(username)
    .bind(username)
    .bind(hash_password("password123").unwrap())
    .bind("user")
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn insert_motorcycle(pool: &sqlx::SqlitePool, user_id: i64, model: &str) -> i64 {
    sqlx::query("INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)")
        .bind("BMW")
        .bind(model)
        .bind(user_id)
        .bind(0)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid()
}

async fn insert_location(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    name: &str,
    kind: &str,
    coords: Option<(f64, f64)>,
    country: Option<&str>,
) -> i64 {
    sqlx::query(
        "INSERT INTO locations (name, type, latitude, longitude, countryCode, userId, createdAt) \
         VALUES (?, ?, ?, ?, ?, ?, '2024-01-01T00:00:00Z')",
    )
    .bind(name)
    .bind(kind)
    .bind(coords.map(|c| c.0))
    .bind(coords.map(|c| c.1))
    .bind(country)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn insert_fuel(
    pool: &sqlx::SqlitePool,
    motorcycle_id: i64,
    date: &str,
    odo: i64,
    location_id: Option<i64>,
    liters: f64,
    cost: f64,
) -> i64 {
    sqlx::query(
        "INSERT INTO maintenanceRecords (date, odo, motorcycleId, type, fuelAmount, cost, locationId) \
         VALUES (?, ?, ?, 'fuel', ?, ?, ?)",
    )
    .bind(date)
    .bind(odo)
    .bind(motorcycle_id)
    .bind(liters)
    .bind(cost)
    .bind(location_id)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn get_json(app: axum::Router, token: &str, uri: &str) -> Value {
    let response = app
        .oneshot(auth(
            Request::builder().uri(uri).body(Body::empty()).unwrap(),
            token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn put_location(app: axum::Router, token: &str, id: i64, body: Value) -> (StatusCode, Value) {
    let response = app
        .oneshot(auth(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/locations/{id}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
            token,
        ))
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

/// Zurich garage (home), a station nearby, Milan and Florence; one 3-day tour
/// on the GS, a nearby stop on the second bike, plus an ungeocoded workshop
/// with a service visit and one fuel record without any location.
#[tokio::test]
async fn map_reports_locations_legs_tours_years_and_calendar() {
    let (app, pool, token) = setup_test_app().await;
    let user_id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'testuser'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let gs = insert_motorcycle(&pool, user_id, "R1250GS").await;
    let rt = insert_motorcycle(&pool, user_id, "R1250RT").await;

    let garage = insert_location(
        &pool,
        user_id,
        "Garage",
        "storage",
        Some((47.3769, 8.5417)),
        Some("CH"),
    )
    .await;
    let nearby = insert_location(
        &pool,
        user_id,
        "Tankstelle Nord",
        "fuel_station",
        Some((47.5, 8.9)),
        Some("CH"),
    )
    .await;
    let milan = insert_location(
        &pool,
        user_id,
        "Milano Agip",
        "fuel_station",
        Some((45.4642, 9.19)),
        Some("IT"),
    )
    .await;
    let florence = insert_location(
        &pool,
        user_id,
        "Firenze Q8",
        "fuel_station",
        Some((43.7696, 11.2558)),
        Some("IT"),
    )
    .await;
    let workshop = insert_location(
        &pool,
        user_id,
        "Werkstatt Meier",
        "maintenance_shop",
        None,
        None,
    )
    .await;

    insert_fuel(&pool, gs, "2024-06-01", 1000, Some(nearby), 15.0, 30.0).await;
    insert_fuel(&pool, gs, "2024-06-02", 1250, Some(milan), 14.0, 28.0).await;
    insert_fuel(&pool, gs, "2024-06-03", 1600, Some(florence), 18.0, 36.0).await;
    insert_fuel(
        &pool,
        gs,
        "2024-06-05T09:30:00Z",
        2100,
        Some(nearby),
        20.0,
        40.0,
    )
    .await;
    insert_fuel(&pool, rt, "2024-07-10", 500, Some(nearby), 10.0, 20.0).await;
    insert_fuel(&pool, rt, "2024-07-20", 800, None, 12.0, 24.0).await;
    sqlx::query(
        "INSERT INTO maintenanceRecords (date, odo, motorcycleId, type, cost, locationId, description) \
         VALUES ('2024-04-01', 900, ?, 'service', 450.0, ?, 'Service')",
    )
    .bind(gs)
    .bind(workshop)
    .execute(&pool)
    .await
    .unwrap();

    // Another user's data must not leak in.
    let other = insert_user(&pool, "other").await;
    let other_bike = insert_motorcycle(&pool, other, "F650GS").await;
    let other_loc = insert_location(
        &pool,
        other,
        "Elsewhere",
        "fuel_station",
        Some((48.0, 9.0)),
        Some("DE"),
    )
    .await;
    insert_fuel(
        &pool,
        other_bike,
        "2024-06-01",
        10,
        Some(other_loc),
        5.0,
        10.0,
    )
    .await;

    let map = get_json(app.clone(), &token, "/api/map").await;

    // Motorcycles + home.
    assert_eq!(map["motorcycles"].as_array().unwrap().len(), 2);
    assert_eq!(map["home"]["locationId"], garage);
    assert_eq!(map["home"]["source"], "storage");

    // Locations: only geocoded ones, sorted by name; the other user's is absent.
    let locations = map["locations"].as_array().unwrap();
    assert_eq!(locations.len(), 4);
    assert!(locations.iter().all(|l| l["id"] != other_loc));
    let station = locations.iter().find(|l| l["id"] == nearby).unwrap();
    assert_eq!(station["fuelStops"], 3);
    assert_eq!(station["liters"], 45.0);
    assert_eq!(station["fuelCost"], 90.0);
    assert_eq!(station["countryCode"], "CH");
    assert_eq!(station["firstVisit"], "2024-06-01");
    assert_eq!(station["lastVisit"], "2024-07-10");
    assert_eq!(station["years"]["2024"]["fuelStops"], 3);
    assert_eq!(station["perMotorcycle"].as_array().unwrap().len(), 2);
    assert!(station["distanceFromHomeKm"].as_f64().unwrap() > 20.0);
    let far = locations.iter().find(|l| l["id"] == florence).unwrap();
    assert!(far["distanceFromHomeKm"].as_f64().unwrap() > 400.0);

    // Ungeocoded workshop with its service visit, plus the location-less fuel record.
    assert_eq!(map["ungeocoded"][0]["id"], workshop);
    assert_eq!(map["ungeocoded"][0]["visits"], 1);
    assert_eq!(map["ungeocoded"][0]["type"], "maintenanceShop");
    assert_eq!(map["unlocatedFuelCount"], 1);

    // Legs: three on the GS, one on the RT (odo delta > 0 each).
    let legs = map["legs"].as_array().unwrap();
    assert_eq!(legs.len(), 4);
    let outbound = legs.iter().find(|l| l["toLocationId"] == milan).unwrap();
    assert_eq!(outbound["distanceKm"], 250);
    assert_eq!(outbound["days"], 1);
    assert_eq!(outbound["liters"], 14.0);
    assert_eq!(outbound["from"]["latitude"], 47.5);
    assert!(outbound["straightKm"].as_f64().unwrap() > 200.0);
    assert_eq!(outbound["tourId"], format!("{gs}-2024-06-01"));
    let rt_leg = legs.iter().find(|l| l["motorcycleId"] == rt).unwrap();
    assert!(rt_leg["to"].is_null());
    assert!(rt_leg["tourId"].is_null());

    // Tour: one, 4 stops, 1100 km, CH + IT, farthest Florence.
    let tours = map["tours"].as_array().unwrap();
    assert_eq!(tours.len(), 1);
    let tour = &tours[0];
    assert_eq!(tour["startDate"], "2024-06-01");
    assert_eq!(tour["endDate"], "2024-06-05");
    assert_eq!(tour["days"], 5);
    assert_eq!(tour["stops"], 4);
    assert_eq!(tour["distanceKm"], 1100);
    assert_eq!(tour["liters"], 52.0);
    assert_eq!(tour["countries"], serde_json::json!(["CH", "IT"]));
    assert_eq!(tour["farthestLocationId"], florence);
    assert_eq!(tour["legIds"].as_array().unwrap().len(), 3);

    // Years.
    let years = map["years"].as_array().unwrap();
    assert_eq!(years.len(), 1);
    let y = &years[0];
    assert_eq!(y["year"], 2024);
    assert_eq!(y["distanceKm"], 1400);
    assert_eq!(y["fuelStops"], 6);
    assert_eq!(y["ridingDays"], 6);
    assert_eq!(y["tours"], 1);
    assert_eq!(y["dayTrips"], 0);
    assert_eq!(y["countries"], serde_json::json!(["CH", "IT"]));
    assert_eq!(y["kmByCountry"]["CH"], 500);
    assert_eq!(y["kmByCountry"]["IT"], 600);
    assert_eq!(y["kmByCountry"]["??"], 300);
    assert_eq!(y["seasonStart"], "2024-06-01");
    assert_eq!(y["seasonEnd"], "2024-07-20");
    assert_eq!(y["farthest"]["locationId"], florence);
    assert_eq!(y["perMotorcycle"].as_array().unwrap().len(), 2);

    // Calendar: six fuel dates, km attributed to the day the leg ended.
    let calendar = map["calendar"].as_array().unwrap();
    assert_eq!(calendar.len(), 6);
    let june5 = calendar.iter().find(|d| d["date"] == "2024-06-05").unwrap();
    assert_eq!(june5["km"], 500);
    assert_eq!(june5["stops"], 1);
}

#[tokio::test]
async fn map_is_empty_but_valid_for_a_fresh_user() {
    let (app, _pool, token) = setup_test_app().await;
    let map = get_json(app, &token, "/api/map").await;
    assert!(map["home"].is_null());
    assert_eq!(map["locations"], serde_json::json!([]));
    assert_eq!(map["legs"], serde_json::json!([]));
    assert_eq!(map["tours"], serde_json::json!([]));
    assert_eq!(map["years"], serde_json::json!([]));
    assert_eq!(map["calendar"], serde_json::json!([]));
    assert_eq!(map["unlocatedFuelCount"], 0);
}

/// `countryCode` is additive: the old request shape still works and leaves
/// the stored value alone, the new field round-trips, "" clears it and junk
/// is rejected.
#[tokio::test]
async fn location_country_code_is_optional_validated_and_round_trips() {
    let (app, pool, token) = setup_test_app().await;
    let user_id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'testuser'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let id = insert_location(&pool, user_id, "Shop", "maintenance_shop", None, None).await;

    // Old shape (no countryCode) keeps working and reports countryCode: null.
    let (status, body) = put_location(app.clone(), &token, id, json!({ "name": "Shop 2" })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["location"]["name"], "Shop 2");
    assert!(body["location"]["countryCode"].is_null());

    // Set (lowercase is normalised).
    let (status, body) =
        put_location(app.clone(), &token, id, json!({ "countryCode": "it" })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["location"]["countryCode"], "IT");

    // Absent → preserved.
    let (_, body) = put_location(
        app.clone(),
        &token,
        id,
        json!({ "latitude": 45.0, "longitude": 9.0 }),
    )
    .await;
    assert_eq!(body["location"]["countryCode"], "IT");
    let list = get_json(app.clone(), &token, "/api/locations").await;
    assert_eq!(list["locations"][0]["countryCode"], "IT");

    // Junk is rejected.
    let (status, _) = put_location(app.clone(), &token, id, json!({ "countryCode": "ITA" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // "" clears.
    let (_, body) = put_location(app.clone(), &token, id, json!({ "countryCode": "" })).await;
    assert!(body["location"]["countryCode"].is_null());

    // Create accepts it too.
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method("POST")
                .uri("/api/locations")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "name": "Agip", "type": "fuelStation", "latitude": 45.4, "longitude": 9.1, "countryCode": "it" }).to_string(),
                ))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["location"]["countryCode"], "IT");
}
