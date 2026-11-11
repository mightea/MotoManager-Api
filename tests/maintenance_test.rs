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

async fn body_json(response: axum::response::Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn test_fuel_additive_flags_roundtrip() {
    let (app, _pool, token, moto_id) = setup_test_app().await;

    // Create a fuel record with an additive but no lead substitute.
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/motorcycles/{}/maintenance", moto_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "date": "2026-07-17",
                        "odo": 1200,
                        "type": "fuel",
                        "fuelAmount": 14.5,
                        "fuelAdditiveAdded": true,
                        "leadSubstituteAdded": false
                    })
                    .to_string(),
                ))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = body_json(response).await;
    let record = &body["maintenanceRecord"];
    let record_id = record["id"].as_i64().unwrap();
    assert_eq!(record["fuelAdditiveAdded"], json!(true));
    assert_eq!(record["leadSubstituteAdded"], json!(false));

    // Toggle the lead substitute on via update.
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::PUT)
                .uri(format!(
                    "/api/motorcycles/{}/maintenance/{}",
                    moto_id, record_id
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "fuelAdditiveAdded": true,
                        "leadSubstituteAdded": true
                    })
                    .to_string(),
                ))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["maintenanceRecord"]["fuelAdditiveAdded"], json!(true));
    assert_eq!(
        body["maintenanceRecord"]["leadSubstituteAdded"],
        json!(true)
    );

    // An update that omits the flags must preserve them.
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::PUT)
                .uri(format!(
                    "/api/motorcycles/{}/maintenance/{}",
                    moto_id, record_id
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "odo": 1250 }).to_string()))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["maintenanceRecord"]["odo"], json!(1250));
    assert_eq!(body["maintenanceRecord"]["fuelAdditiveAdded"], json!(true));
    assert_eq!(
        body["maintenanceRecord"]["leadSubstituteAdded"],
        json!(true)
    );

    // The list endpoint surfaces the flags too.
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .uri(format!("/api/motorcycles/{}/maintenance", moto_id))
                .body(Body::empty())
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let records = body["maintenanceRecords"].as_array().unwrap();
    let listed = records
        .iter()
        .find(|r| r["id"].as_i64() == Some(record_id))
        .unwrap();
    assert_eq!(listed["fuelAdditiveAdded"], json!(true));
    assert_eq!(listed["leadSubstituteAdded"], json!(true));
}

#[tokio::test]
async fn test_fuel_additive_flags_default_false() {
    let (app, _pool, token, moto_id) = setup_test_app().await;

    // A create that never mentions the flags gets them as false, not null.
    let response = app
        .oneshot(auth(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/motorcycles/{}/maintenance", moto_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "date": "2026-07-17",
                        "odo": 1100,
                        "type": "fuel",
                        "fuelAmount": 12.0
                    })
                    .to_string(),
                ))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = body_json(response).await;
    assert_eq!(body["maintenanceRecord"]["fuelAdditiveAdded"], json!(false));
    assert_eq!(
        body["maintenanceRecord"]["leadSubstituteAdded"],
        json!(false)
    );
}

async fn send(
    app: &axum::Router,
    token: &str,
    method: Method,
    uri: String,
    body: Option<Value>,
) -> Value {
    let mut req = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(b) => {
            req = req.header(header::CONTENT_TYPE, "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(auth(req.body(body).unwrap(), token))
        .await
        .unwrap();
    assert!(response.status().is_success(), "{}", response.status());
    body_json(response).await
}

async fn fuel_stats(pool: &sqlx::SqlitePool, id: i64) -> (Option<f64>, Option<f64>) {
    sqlx::query_as("SELECT tripDistance, fuelConsumption FROM maintenanceRecords WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Consumption depends on the neighbouring fill, so backfilling, correcting
/// or deleting an earlier fill must re-derive the later ones too.
#[tokio::test]
async fn test_fuel_consumption_follows_neighbouring_fills() {
    let (app, pool, token, moto_id) = setup_test_app().await;
    let base = format!("/api/motorcycles/{}/maintenance", moto_id);
    let create = |odo: i64, liters: f64, date: &str| json!({ "date": date, "odo": odo, "type": "fuel", "fuelAmount": liters });

    let old = send(
        &app,
        &token,
        Method::POST,
        base.clone(),
        Some(create(32414, 9.0, "2026-08-01")),
    )
    .await;
    let old_id = old["maintenanceRecord"]["id"].as_i64().unwrap();
    let later = send(
        &app,
        &token,
        Method::POST,
        base.clone(),
        Some(create(33250, 11.34, "2026-10-07")),
    )
    .await;
    let later_id = later["maintenanceRecord"]["id"].as_i64().unwrap();
    assert_eq!(fuel_stats(&pool, later_id).await.0, Some(836.0));

    // Backfill the fill in between: the later fill now measures 250 km.
    let mid = send(
        &app,
        &token,
        Method::POST,
        base.clone(),
        Some(create(33000, 8.4, "2026-09-26")),
    )
    .await;
    let mid_id = mid["maintenanceRecord"]["id"].as_i64().unwrap();
    let (trip, consumption) = fuel_stats(&pool, later_id).await;
    assert_eq!(trip, Some(250.0));
    assert!((consumption.unwrap() - 4.536).abs() < 1e-9);

    // A client echoing a stale tripDistance must not pin it.
    send(
        &app,
        &token,
        Method::PUT,
        format!("{}/{}", base, later_id),
        Some(json!({ "tripDistance": 836.0, "fuelConsumption": 1.36 })),
    )
    .await;
    assert_eq!(fuel_stats(&pool, later_id).await.0, Some(250.0));

    // Deleting the oldest fill makes the 33000 fill the first one: no consumption.
    send(
        &app,
        &token,
        Method::DELETE,
        format!("{}/{}", base, old_id),
        None,
    )
    .await;
    assert_eq!(fuel_stats(&pool, mid_id).await, (None, None));
    assert_eq!(fuel_stats(&pool, later_id).await.0, Some(250.0));

    // Correcting the first fill's odometer re-derives the next one.
    send(
        &app,
        &token,
        Method::PUT,
        format!("{}/{}", base, mid_id),
        Some(json!({ "odo": 33100 })),
    )
    .await;
    assert_eq!(fuel_stats(&pool, later_id).await.0, Some(150.0));
}

/// Consumption runs full tank to full tank: a partial fill gets none, and its
/// litres count towards the next full one.
#[tokio::test]
async fn test_partial_fill_counts_towards_next_full_tank() {
    let (app, pool, token, moto_id) = setup_test_app().await;
    let base = format!("/api/motorcycles/{}/maintenance", moto_id);
    let fill = |odo: i64, liters: f64, date: &str, full: Option<bool>| {
        let mut v = json!({ "date": date, "odo": odo, "type": "fuel", "fuelAmount": liters });
        if let Some(f) = full {
            v["fullTank"] = json!(f);
        }
        v
    };

    let first = send(
        &app,
        &token,
        Method::POST,
        base.clone(),
        Some(fill(10_000, 12.0, "2026-08-01", None)),
    )
    .await;
    // Absent = full tank, for older clients.
    assert_eq!(first["maintenanceRecord"]["fullTank"], json!(true));

    let partial = send(
        &app,
        &token,
        Method::POST,
        base.clone(),
        Some(fill(10_100, 4.0, "2026-08-05", Some(false))),
    )
    .await;
    let partial_id = partial["maintenanceRecord"]["id"].as_i64().unwrap();
    assert_eq!(partial["maintenanceRecord"]["fullTank"], json!(false));

    let full = send(
        &app,
        &token,
        Method::POST,
        base.clone(),
        Some(fill(10_300, 11.0, "2026-08-09", Some(true))),
    )
    .await;
    let full_id = full["maintenanceRecord"]["id"].as_i64().unwrap();

    // The partial fill has a trip but no consumption.
    assert_eq!(fuel_stats(&pool, partial_id).await, (Some(100.0), None));
    // The full fill: (4 + 11) L over 300 km since the last full tank.
    let (trip, consumption) = fuel_stats(&pool, full_id).await;
    assert_eq!(trip, Some(200.0));
    assert!((consumption.unwrap() - 5.0).abs() < 1e-9);

    // Marking the partial fill full splits the stretch in two.
    send(
        &app,
        &token,
        Method::PUT,
        format!("{}/{}", base, partial_id),
        Some(json!({ "fullTank": true })),
    )
    .await;
    assert!((fuel_stats(&pool, partial_id).await.1.unwrap() - 4.0).abs() < 1e-9);
    assert!((fuel_stats(&pool, full_id).await.1.unwrap() - 5.5).abs() < 1e-9);
}

#[tokio::test]
async fn test_update_maintenance_parent_id_ownership_and_scoped_reconciliation() {
    let (app, pool, victim_token, victim_moto_id) = setup_test_app().await;

    let password_hash = hash_password("password123").unwrap();
    let attacker_user_id = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("attacker@example.com")
    .bind("attacker")
    .bind("Attacker")
    .bind(password_hash)
    .bind("user")
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    let attacker_token = create_session(&pool, attacker_user_id).await.unwrap();
    let attacker_moto_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("Honda")
    .bind("CB500")
    .bind(attacker_user_id)
    .bind(500)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    let victim_parent = send(
        &app,
        &victim_token,
        Method::POST,
        format!("/api/motorcycles/{}/maintenance", victim_moto_id),
        Some(json!({
            "date": "2026-08-01",
            "odo": 5000,
            "type": "inspection"
        })),
    )
    .await;
    let victim_mid = victim_parent["maintenanceRecord"]["id"].as_i64().unwrap();

    let attacker_rec = send(
        &app,
        &attacker_token,
        Method::POST,
        format!("/api/motorcycles/{}/maintenance", attacker_moto_id),
        Some(json!({
            "date": "2026-01-01",
            "odo": 1000,
            "type": "chain"
        })),
    )
    .await;
    let attacker_mid = attacker_rec["maintenanceRecord"]["id"].as_i64().unwrap();

    // Attacker cannot link their record to victim's maintenance record via parentId.
    let res = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::PUT)
                .uri(format!(
                    "/api/motorcycles/{}/maintenance/{}",
                    attacker_moto_id, attacker_mid
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "parentId": victim_mid }).to_string()))
                .unwrap(),
            &attacker_token,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // Self-referential parentId is rejected with 400 Bad Request.
    let res = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::PUT)
                .uri(format!(
                    "/api/motorcycles/{}/maintenance/{}",
                    attacker_moto_id, attacker_mid
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "parentId": attacker_mid }).to_string()))
                .unwrap(),
            &attacker_token,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Even if a foreign record points at victim_mid in DB, bundled_items reconciliation
    // must stay scoped to victim_moto_id.
    sqlx::query("UPDATE maintenanceRecords SET parentId = ? WHERE id = ?")
        .bind(victim_mid)
        .bind(attacker_mid)
        .execute(&pool)
        .await
        .unwrap();

    send(
        &app,
        &victim_token,
        Method::PUT,
        format!(
            "/api/motorcycles/{}/maintenance/{}",
            victim_moto_id, victim_mid
        ),
        Some(json!({
            "date": "2026-09-01",
            "odo": 6000,
            "bundledItems": ["chain"]
        })),
    )
    .await;

    let (attacker_date, attacker_odo): (String, i64) =
        sqlx::query_as("SELECT date, odo FROM maintenanceRecords WHERE id = ?")
            .bind(attacker_mid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attacker_date, "2026-01-01");
    assert_eq!(attacker_odo, 1000);

    let victim_children: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM maintenanceRecords WHERE parentId = ? AND motorcycleId = ? AND type = 'chain'",
    )
    .bind(victim_mid)
    .bind(victim_moto_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(victim_children.0, 1);

    // Clearing bundledItems on victim_mid must not delete attacker_mid.
    send(
        &app,
        &victim_token,
        Method::PUT,
        format!(
            "/api/motorcycles/{}/maintenance/{}",
            victim_moto_id, victim_mid
        ),
        Some(json!({
            "bundledItems": []
        })),
    )
    .await;

    let attacker_exists: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM maintenanceRecords WHERE id = ?")
            .bind(attacker_mid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attacker_exists.0, 1);
}

#[tokio::test]
async fn test_create_maintenance_parent_id_ownership_validation() {
    let (app, pool, token, moto_id) = setup_test_app().await;

    let password_hash = hash_password("password123").unwrap();
    let other_user_id = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("other@example.com")
    .bind("otheruser")
    .bind("Other User")
    .bind(password_hash)
    .bind("user")
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    let other_moto_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, ?)",
    )
    .bind("Honda")
    .bind("CB500X")
    .bind(other_user_id)
    .bind(500)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    let other_parent_id = sqlx::query(
        "INSERT INTO maintenanceRecords (date, odo, motorcycleId, type) VALUES (?, ?, ?, ?)",
    )
    .bind("2026-08-01")
    .bind(1000)
    .bind(other_moto_id)
    .bind("service")
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();

    // Attempting to create a maintenance record referencing another motorcycle's parentId must fail with 404.
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/motorcycles/{}/maintenance", moto_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "date": "2026-08-02",
                        "odo": 1200,
                        "type": "chain",
                        "parentId": other_parent_id
                    })
                    .to_string(),
                ))
                .unwrap(),
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Referencing a parent record on the same motorcycle succeeds.
    let own_parent = send(
        &app,
        &token,
        Method::POST,
        format!("/api/motorcycles/{}/maintenance", moto_id),
        Some(json!({
            "date": "2026-08-01",
            "odo": 1100,
            "type": "service"
        })),
    )
    .await;
    let own_parent_id = own_parent["maintenanceRecord"]["id"].as_i64().unwrap();

    let child = send(
        &app,
        &token,
        Method::POST,
        format!("/api/motorcycles/{}/maintenance", moto_id),
        Some(json!({
            "date": "2026-08-02",
            "odo": 1200,
            "type": "chain",
            "parentId": own_parent_id
        })),
    )
    .await;
    assert_eq!(child["maintenanceRecord"]["parentId"], json!(own_parent_id));
}
