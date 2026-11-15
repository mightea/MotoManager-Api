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
        mcp_allowed_hosts: Vec::new(),
        public_url: "http://localhost:3001".to_string(),
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

const BIKE: &str = "/api/motorcycles";

async fn add_maintenance(
    app: &axum::Router,
    token: &str,
    motorcycle_id: i64,
    body: Value,
) -> Value {
    let (status, body) = json_request(
        app,
        token,
        Method::POST,
        format!("{BIKE}/{motorcycle_id}/maintenance"),
        body,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["maintenanceRecord"].clone()
}

async fn record_odos(app: &axum::Router, token: &str, motorcycle_id: i64) -> Vec<(String, i64)> {
    let (status, body) = json_request(
        app,
        token,
        Method::GET,
        format!("{BIKE}/{motorcycle_id}/maintenance"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let mut rows: Vec<(String, i64)> = body["maintenanceRecords"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["date"].as_str().unwrap().to_string(),
                r["odo"].as_i64().unwrap(),
            )
        })
        .collect();
    rows.sort();
    rows
}

async fn create_change(
    app: &axum::Router,
    token: &str,
    motorcycle_id: i64,
    body: Value,
) -> (StatusCode, Value) {
    json_request(
        app,
        token,
        Method::POST,
        format!("{BIKE}/{motorcycle_id}/odometer-changes"),
        body,
    )
    .await
}

fn swap() -> Value {
    json!({ "date": "2018-11-22", "readingBefore": 70_000, "readingAfter": 7_000 })
}

/// Old unit ran to 70'000, new one started at 7'000: records read off the new
/// instrument are raised by 63'000, earlier ones stay.
#[tokio::test]
async fn recording_a_change_lifts_later_records_to_the_total() {
    let (app, token, id) = setup_test_app().await;
    add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2007-03-15", "odo": 68_420, "type": "repair" }),
    )
    .await;
    add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2018-11-22", "odo": 7_000, "type": "repair" }),
    )
    .await;
    add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2023-02-02", "odo": 8_402, "type": "repair" }),
    )
    .await;

    let (status, body) = create_change(&app, &token, id, swap()).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["odometerChange"]["offset"], 63_000);
    assert_eq!(body["odometerChange"]["kind"], "replacement");
    assert_eq!(body["odometerOffset"], 63_000);
    assert_eq!(body["shiftedRecords"], 2);

    assert_eq!(
        record_odos(&app, &token, id).await,
        vec![
            ("2007-03-15".to_string(), 68_420),
            ("2018-11-22".to_string(), 70_000),
            ("2023-02-02".to_string(), 71_402),
        ]
    );
}

#[tokio::test]
async fn shifted_records_are_returned_by_an_incremental_pull() {
    let (app, token, id) = setup_test_app().await;
    add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2023-02-02", "odo": 8_402, "type": "repair" }),
    )
    .await;
    let cursor = "2999-01-01T00:00:00.000Z";
    let (_, before) = json_request(
        &app,
        &token,
        Method::GET,
        format!("{BIKE}/{id}/maintenance?since={cursor}"),
        Value::Null,
    )
    .await;
    assert_eq!(before["maintenanceRecords"].as_array().unwrap().len(), 0);

    let since = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    create_change(&app, &token, id, swap()).await;

    let (_, after) = json_request(
        &app,
        &token,
        Method::GET,
        format!("{BIKE}/{id}/maintenance?since={since}"),
        Value::Null,
    )
    .await;
    let rows = after["maintenanceRecords"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["odo"], 71_402);
}

#[tokio::test]
async fn editing_and_deleting_a_change_rebases_the_records() {
    let (app, token, id) = setup_test_app().await;
    add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2015-06-01", "odo": 69_000, "type": "repair" }),
    )
    .await;
    add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2023-02-02", "odo": 8_402, "type": "repair" }),
    )
    .await;
    let (_, created) = create_change(&app, &token, id, swap()).await;
    let change_id = created["odometerChange"]["id"].as_i64().unwrap();

    // The old unit actually showed 72'000 when it came off.
    let (status, body) = json_request(
        &app,
        &token,
        Method::PUT,
        format!("{BIKE}/{id}/odometer-changes/{change_id}"),
        json!({ "readingBefore": 72_000 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["odometerChange"]["offset"], 65_000);
    assert_eq!(record_odos(&app, &token, id).await[1].1, 73_402);

    // A kind-only edit moves nothing.
    let (_, body) = json_request(
        &app,
        &token,
        Method::PUT,
        format!("{BIKE}/{id}/odometer-changes/{change_id}"),
        json!({ "kind": "rollover" }),
    )
    .await;
    assert_eq!(body["shiftedRecords"], 0);
    assert_eq!(record_odos(&app, &token, id).await[1].1, 73_402);

    let (status, body) = json_request(
        &app,
        &token,
        Method::DELETE,
        format!("{BIKE}/{id}/odometer-changes/{change_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["odometerChanges"].as_array().unwrap().len(), 0);
    assert_eq!(
        record_odos(&app, &token, id).await,
        vec![
            ("2015-06-01".to_string(), 69_000),
            ("2023-02-02".to_string(), 8_402),
        ]
    );
}

#[tokio::test]
async fn new_records_are_stored_as_totals_whatever_the_client_knew() {
    let (app, token, id) = setup_test_app().await;
    create_change(&app, &token, id, swap()).await;

    // Up-to-date client: instrument 8'560 + its offset 63'000.
    let current = add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2026-11-15", "odo": 71_560, "odoOffset": 63_000, "type": "repair" }),
    )
    .await;
    assert_eq!(current["odo"], 71_560);

    // Client that has not synced the change yet sends the raw reading.
    let stale = add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2026-11-16", "odo": 8_600, "odoOffset": 0, "type": "repair" }),
    )
    .await;
    assert_eq!(stale["odo"], 71_600);

    // Old request shape (no odoOffset): an instrument reading is lifted ...
    let legacy = add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2026-11-17", "odo": 8_650, "type": "repair" }),
    )
    .await;
    assert_eq!(legacy["odo"], 71_650);
    // ... and a value that is already a total is kept.
    let legacy_total = add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2026-11-18", "odo": 71_700, "type": "repair" }),
    )
    .await;
    assert_eq!(legacy_total["odo"], 71_700);

    // Backfilled record from before the change: no offset applies.
    let earlier = add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2010-01-01", "odo": 60_000, "odoOffset": 0, "type": "repair" }),
    )
    .await;
    assert_eq!(earlier["odo"], 60_000);

    // Editing with the old shape and an unchanged odo keeps the total.
    let record_id = legacy["id"].as_i64().unwrap();
    let (status, body) = json_request(
        &app,
        &token,
        Method::PUT,
        format!("{BIKE}/{id}/maintenance/{record_id}"),
        json!({ "odo": 71_650, "description": "edited" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["maintenanceRecord"]["odo"], 71_650);
}

/// Previous contract: a bike without odometer changes behaves exactly as
/// before, with or without the new field.
#[tokio::test]
async fn bikes_without_changes_are_untouched() {
    let (app, token, id) = setup_test_app().await;
    let plain = add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2026-11-15", "odo": 8_560, "type": "repair" }),
    )
    .await;
    assert_eq!(plain["odo"], 8_560);

    let (status, issue) = json_request(
        &app,
        &token,
        Method::POST,
        format!("{BIKE}/{id}/issues"),
        json!({ "odo": 8_570, "title": "Leak", "date": "2026-11-15" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{issue}");
    assert_eq!(issue["issue"]["odo"], 8_570);

    let (_, list) = json_request(&app, &token, Method::GET, BIKE.to_string(), Value::Null).await;
    let bike = &list["motorcycles"][0];
    assert_eq!(bike["odometerChanges"], json!([]));
    assert_eq!(bike["odometerOffset"], 0);
    assert_eq!(bike["latestOdo"], 8_560);
}

#[tokio::test]
async fn issues_and_motorcycle_responses_follow_the_change() {
    let (app, token, id) = setup_test_app().await;
    json_request(
        &app,
        &token,
        Method::POST,
        format!("{BIKE}/{id}/issues"),
        json!({ "odo": 8_100, "title": "Leak", "date": "2020-05-01" }),
    )
    .await;
    add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2023-02-02", "odo": 8_402, "type": "repair" }),
    )
    .await;
    create_change(&app, &token, id, swap()).await;

    let (_, detail) = json_request(
        &app,
        &token,
        Method::GET,
        format!("{BIKE}/{id}"),
        Value::Null,
    )
    .await;
    assert_eq!(detail["issues"][0]["odo"], 71_100);
    assert_eq!(detail["motorcycle"]["odometerOffset"], 63_000);
    assert_eq!(
        detail["motorcycle"]["odometerChanges"][0]["readingAfter"],
        7_000
    );
    assert_eq!(detail["odometerChanges"][0]["offset"], 63_000);
    // Purchase predates the change (no purchaseDate set): initialOdo stays.
    assert_eq!(detail["motorcycle"]["initialOdo"], 1_000);

    let (_, list) = json_request(&app, &token, Method::GET, BIKE.to_string(), Value::Null).await;
    assert_eq!(list["motorcycles"][0]["latestOdo"], 71_402);
    assert_eq!(list["motorcycles"][0]["odometerOffset"], 63_000);

    // A new issue from an up-to-date client.
    let (_, issue) = json_request(
        &app,
        &token,
        Method::POST,
        format!("{BIKE}/{id}/issues"),
        json!({ "odo": 71_500, "odoOffset": 63_000, "title": "Rattle", "date": "2026-11-15" }),
    )
    .await;
    assert_eq!(issue["issue"]["odo"], 71_500);
}

#[tokio::test]
async fn fuel_consumption_spans_the_change() {
    let (app, token, id) = setup_test_app().await;
    add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2018-11-01", "odo": 69_800, "type": "fuel", "fuelAmount": 15.0 }),
    )
    .await;
    add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2018-12-01", "odo": 7_100, "type": "fuel", "fuelAmount": 21.0 }),
    )
    .await;
    create_change(&app, &token, id, swap()).await;

    let (_, body) = json_request(
        &app,
        &token,
        Method::GET,
        format!("{BIKE}/{id}/maintenance"),
        Value::Null,
    )
    .await;
    let later = body["maintenanceRecords"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["date"] == "2018-12-01")
        .unwrap();
    // 69'800 → 70'100 total: 300 km on 21 L.
    assert_eq!(later["odo"], 70_100);
    assert_eq!(later["tripDistance"], 300.0);
    assert!((later["fuelConsumption"].as_f64().unwrap() - 7.0).abs() < 1e-9);
}

#[tokio::test]
async fn invalid_changes_are_rejected_and_other_users_are_locked_out() {
    let (app, token, id) = setup_test_app().await;
    for body in [
        json!({ "date": "22.11.2018", "readingBefore": 70_000, "readingAfter": 7_000 }),
        json!({ "date": "2018-11-22", "readingBefore": -1, "readingAfter": 7_000 }),
        json!({ "date": "2018-11-22", "readingBefore": 70_000, "readingAfter": 7_000, "kind": "magic" }),
    ] {
        let (status, _) = create_change(&app, &token, id, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    // New unit showing far more than the old one would push a low record
    // below zero: rejected, nothing moved.
    add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2023-02-02", "odo": 500, "type": "repair" }),
    )
    .await;
    let (status, _) = create_change(
        &app,
        &token,
        id,
        json!({ "date": "2018-11-22", "readingBefore": 1_000, "readingAfter": 50_000 }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(record_odos(&app, &token, id).await[0].1, 500);
    let (_, list) = json_request(
        &app,
        &token,
        Method::GET,
        format!("{BIKE}/{id}/odometer-changes"),
        Value::Null,
    )
    .await;
    assert_eq!(list["odometerChanges"], json!([]));

    let (status, _) = create_change(&app, &token, id + 999, swap()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = json_request(
        &app,
        "not-a-token",
        Method::GET,
        format!("{BIKE}/{id}/odometer-changes"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// What an app release from before this feature does: it shows the stored
/// totals, sends `odo` on every create and update, and never sends
/// `odoOffset`.
#[tokio::test]
async fn old_app_versions_keep_working_after_a_change() {
    let (app, token, id) = setup_test_app().await;
    let synced = add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2023-02-01", "odo": 8_396, "type": "repair", "clientId": "c-1" }),
    )
    .await;
    let edited_later = add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2023-02-02", "odo": 8_402, "type": "repair", "clientId": "c-2" }),
    )
    .await;
    create_change(&app, &token, id, swap()).await;

    // The app has not pulled yet and pushes an edit carrying the odo it still
    // has locally (the pre-shift value): the stored total is kept.
    let record_id = edited_later["id"].as_i64().unwrap();
    let (status, body) = json_request(
        &app,
        &token,
        Method::PUT,
        format!("{BIKE}/{id}/maintenance/{record_id}"),
        json!({ "date": "2023-02-02", "odo": 8_402, "type": "repair", "description": "note" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["maintenanceRecord"]["odo"], 71_402);

    // A retried create is still answered from the stored record.
    let retried = add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2023-02-01", "odo": 8_396, "type": "repair", "clientId": "c-1" }),
    )
    .await;
    assert_eq!(retried["id"], synced["id"]);
    assert_eq!(retried["odo"], 71_396);

    // New entries: the user types what the instrument shows ...
    let reading = add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2026-11-15", "odo": 8_560, "type": "fuel", "fuelAmount": 11.2 }),
    )
    .await;
    assert_eq!(reading["odo"], 71_560);
    // ... or carries on from the total the app shows.
    let total = add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2026-11-20", "odo": 71_700, "type": "fuel", "fuelAmount": 9.0 }),
    )
    .await;
    assert_eq!(total["odo"], 71_700);

    // A total corrected slightly downwards stays a total.
    let total_id = total["id"].as_i64().unwrap();
    let (_, body) = json_request(
        &app,
        &token,
        Method::PUT,
        format!("{BIKE}/{id}/maintenance/{total_id}"),
        json!({ "odo": 71_690 }),
    )
    .await;
    assert_eq!(body["maintenanceRecord"]["odo"], 71_690);

    // Issues go through the same path.
    let (_, issue) = json_request(
        &app,
        &token,
        Method::POST,
        format!("{BIKE}/{id}/issues"),
        json!({ "odo": 8_800, "title": "Leak", "date": "2026-12-01" }),
    )
    .await;
    assert_eq!(issue["issue"]["odo"], 71_800);
    let issue_id = issue["issue"]["id"].as_i64().unwrap();
    let (_, issue) = json_request(
        &app,
        &token,
        Method::PUT,
        format!("{BIKE}/{id}/issues/{issue_id}"),
        json!({ "odo": 71_800, "status": "done" }),
    )
    .await;
    assert_eq!(issue["issue"]["odo"], 71_800);

    // The pull an old app decodes: same keys and types as before.
    let (_, pulled) = json_request(
        &app,
        &token,
        Method::GET,
        format!("{BIKE}/{id}/maintenance?since=2000-01-01T00:00:00.000Z"),
        Value::Null,
    )
    .await;
    for row in pulled["maintenanceRecords"].as_array().unwrap() {
        assert!(row["odo"].is_i64());
        assert!(row.get("odoOffset").is_none());
    }
}

/// Years later the new instrument has passed the old one's final reading
/// (70'000). A reading typed into an old app is still told apart from a total.
#[tokio::test]
async fn old_app_readings_resolve_after_the_new_instrument_passes_the_old_one() {
    let (app, token, id) = setup_test_app().await;
    create_change(&app, &token, id, swap()).await;
    add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2035-05-01", "odo": 137_900, "odoOffset": 63_000, "type": "repair" }),
    )
    .await;

    let reading = add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2035-06-01", "odo": 75_000, "type": "repair" }),
    )
    .await;
    assert_eq!(reading["odo"], 138_000);
    let total = add_maintenance(
        &app,
        &token,
        id,
        json!({ "date": "2035-07-01", "odo": 138_400, "type": "repair" }),
    )
    .await;
    assert_eq!(total["odo"], 138_400);
}
