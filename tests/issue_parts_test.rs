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

async fn send(app: &axum::Router, method: Method, uri: &str, token: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(auth(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
            token,
        ))
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

async fn insert_issue(pool: &sqlx::SqlitePool, moto_id: i64, title: &str, status: &str) -> i64 {
    sqlx::query(
        "INSERT INTO issues (motorcycleId, odo, title, priority, status, date, updatedAt) \
         VALUES (?, 1200, ?, 'high', ?, '2026-10-01', '2026-10-01T00:00:00.000Z')",
    )
    .bind(moto_id)
    .bind(title)
    .bind(status)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn insert_part(pool: &sqlx::SqlitePool, user_id: i64, name: &str, stock: i64) -> i64 {
    let part_id =
        sqlx::query("INSERT INTO parts (userId, partNumber, name) VALUES (?, '11 12 345', ?)")
            .bind(user_id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap()
            .last_insert_rowid();
    if stock > 0 {
        sqlx::query("INSERT INTO partStocks (partId, quantity) VALUES (?, ?)")
            .bind(part_id)
            .bind(stock)
            .execute(pool)
            .await
            .unwrap();
    }
    part_id
}

async fn second_user(pool: &sqlx::SqlitePool) -> (i64, String, i64) {
    let user_id = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("other@example.com")
    .bind("other")
    .bind("Other")
    .bind(hash_password("password123").unwrap())
    .bind("user")
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let token = create_session(pool, user_id).await.unwrap();
    let moto_id = sqlx::query(
        "INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES ('Yamaha', 'XT', ?, 0)",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid();
    (user_id, token, moto_id)
}

async fn owner_id(pool: &sqlx::SqlitePool, moto_id: i64) -> i64 {
    sqlx::query_scalar("SELECT userId FROM motorcycles WHERE id = ?")
        .bind(moto_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn test_link_list_and_unlink() {
    let (app, pool, token, moto_id) = setup_test_app().await;
    let user_id = owner_id(&pool, moto_id).await;
    let issue_id = insert_issue(&pool, moto_id, "Bremse quietscht", "new").await;
    let part_id = insert_part(&pool, user_id, "Bremsbeläge", 2).await;

    let link_uri = format!("/api/issues/{issue_id}/parts/{part_id}");
    let (status, _) = send(&app, Method::PUT, &link_uri, &token).await;
    assert_eq!(status, StatusCode::OK);
    // Idempotent: linking twice is fine and does not duplicate.
    let (status, _) = send(&app, Method::PUT, &link_uri, &token).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &app,
        Method::GET,
        &format!("/api/parts/{part_id}/issues"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let issues = body["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0]["id"], issue_id);
    assert_eq!(issues[0]["title"], "Bremse quietscht");
    assert_eq!(issues[0]["motorcycleMake"], "BMW");
    assert_eq!(issues[0]["motorcycleModel"], "R1250GS");
    assert_eq!(issues[0]["priority"], "high");
    assert_eq!(issues[0]["isDone"], false);

    // The bike's issue list carries the additive linkedParts field.
    let (_, body) = send(
        &app,
        Method::GET,
        &format!("/api/motorcycles/{moto_id}/issues"),
        &token,
    )
    .await;
    let linked = &body["issues"][0]["linkedParts"];
    assert_eq!(linked.as_array().unwrap().len(), 1);
    assert_eq!(linked[0]["id"], part_id);
    assert_eq!(linked[0]["name"], "Bremsbeläge");
    assert_eq!(linked[0]["partNumber"], "11 12 345");
    assert_eq!(linked[0]["onHand"], 2);
    // ...and so does the motorcycle detail payload the webapp reads.
    let (_, body) = send(
        &app,
        Method::GET,
        &format!("/api/motorcycles/{moto_id}"),
        &token,
    )
    .await;
    assert_eq!(body["issues"][0]["linkedParts"][0]["id"], part_id);

    // Linking does not bump the issue's sync cursor.
    let updated_at: String = sqlx::query_scalar("SELECT updatedAt FROM issues WHERE id = ?")
        .bind(issue_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(updated_at, "2026-10-01T00:00:00.000Z");

    let (status, _) = send(&app, Method::DELETE, &link_uri, &token).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(&app, Method::DELETE, &link_uri, &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, body) = send(
        &app,
        Method::GET,
        &format!("/api/parts/{part_id}/issues"),
        &token,
    )
    .await;
    assert!(body["issues"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_issue_list_without_links_keeps_old_shape() {
    let (app, pool, token, moto_id) = setup_test_app().await;
    insert_issue(&pool, moto_id, "Kette längen", "new").await;

    let (status, body) = send(
        &app,
        Method::GET,
        &format!("/api/motorcycles/{moto_id}/issues"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let issue = &body["issues"][0];
    // Every pre-existing key is still present.
    for key in [
        "id",
        "motorcycleId",
        "odo",
        "title",
        "description",
        "descriptionMarkup",
        "priority",
        "status",
        "date",
        "clientId",
        "updatedAt",
        "deletedAt",
    ] {
        assert!(issue.get(key).is_some(), "missing key {key}");
    }
    assert_eq!(issue["linkedParts"], json!([]));
}

#[tokio::test]
async fn test_deleted_part_and_issue_are_hidden() {
    let (app, pool, token, moto_id) = setup_test_app().await;
    let user_id = owner_id(&pool, moto_id).await;
    let issue_id = insert_issue(&pool, moto_id, "Ölverlust", "new").await;
    let part_id = insert_part(&pool, user_id, "Dichtung", 0).await;
    send(
        &app,
        Method::PUT,
        &format!("/api/issues/{issue_id}/parts/{part_id}"),
        &token,
    )
    .await;

    let (_, body) = send(
        &app,
        Method::GET,
        &format!("/api/motorcycles/{moto_id}/issues"),
        &token,
    )
    .await;
    assert_eq!(body["issues"][0]["linkedParts"][0]["onHand"], 0);

    // Tombstoned part: gone from the issue's linkedParts.
    send(
        &app,
        Method::DELETE,
        &format!("/api/parts/{part_id}"),
        &token,
    )
    .await;
    let (_, body) = send(
        &app,
        Method::GET,
        &format!("/api/motorcycles/{moto_id}/issues"),
        &token,
    )
    .await;
    assert_eq!(body["issues"][0]["linkedParts"], json!([]));

    // Tombstoned issue: gone from the part's issue list.
    let part_id = insert_part(&pool, user_id, "Dichtung neu", 1).await;
    send(
        &app,
        Method::PUT,
        &format!("/api/issues/{issue_id}/parts/{part_id}"),
        &token,
    )
    .await;
    send(
        &app,
        Method::DELETE,
        &format!("/api/motorcycles/{moto_id}/issues/{issue_id}"),
        &token,
    )
    .await;
    let (_, body) = send(
        &app,
        Method::GET,
        &format!("/api/parts/{part_id}/issues"),
        &token,
    )
    .await;
    assert!(body["issues"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_foreign_issue_or_part_is_not_found() {
    let (app, pool, token, moto_id) = setup_test_app().await;
    let user_id = owner_id(&pool, moto_id).await;
    let my_issue = insert_issue(&pool, moto_id, "Mein Mangel", "new").await;
    let my_part = insert_part(&pool, user_id, "Mein Teil", 1).await;

    let (other_id, other_token, other_moto) = second_user(&pool).await;
    let other_issue = insert_issue(&pool, other_moto, "Fremder Mangel", "new").await;
    let other_part = insert_part(&pool, other_id, "Fremdes Teil", 1).await;

    // Foreign part on my issue, my part on a foreign issue: both 404.
    let (status, _) = send(
        &app,
        Method::PUT,
        &format!("/api/issues/{my_issue}/parts/{other_part}"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(
        &app,
        Method::PUT,
        &format!("/api/issues/{other_issue}/parts/{my_part}"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A real link of mine is invisible and unremovable for the other user.
    send(
        &app,
        Method::PUT,
        &format!("/api/issues/{my_issue}/parts/{my_part}"),
        &token,
    )
    .await;
    let (status, _) = send(
        &app,
        Method::GET,
        &format!("/api/parts/{my_part}/issues"),
        &other_token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(
        &app,
        Method::DELETE,
        &format!("/api/issues/{my_issue}/parts/{my_part}"),
        &other_token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM issueParts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_deleting_motorcycle_cascades_links() {
    let (app, pool, token, moto_id) = setup_test_app().await;
    let user_id = owner_id(&pool, moto_id).await;
    let issue_id = insert_issue(&pool, moto_id, "Lager defekt", "new").await;
    let part_id = insert_part(&pool, user_id, "Lager", 1).await;
    send(
        &app,
        Method::PUT,
        &format!("/api/issues/{issue_id}/parts/{part_id}"),
        &token,
    )
    .await;

    let (status, _) = send(
        &app,
        Method::DELETE,
        &format!("/api/motorcycles/{moto_id}"),
        &token,
    )
    .await;
    assert!(status.is_success(), "motorcycle delete failed: {status}");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM issueParts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
