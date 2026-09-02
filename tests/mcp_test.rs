//! MCP endpoint: token authentication, scope enforcement, user-level
//! isolation (incl. admins), input validation, and the audit trail.

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
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use tower::ServiceExt;

struct TestEnv {
    app: axum::Router,
    pool: SqlitePool,
    /// Session token of user "alice" (role user).
    alice_session: String,
    alice_id: i64,
    /// Session token of user "bob" (role user).
    bob_id: i64,
    /// Session token of user "root" (role admin).
    admin_session: String,
    admin_id: i64,
}

async fn create_user(pool: &SqlitePool, name: &str, role: &str) -> i64 {
    let password_hash = hash_password("password123").unwrap();
    sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(format!("{name}@example.com"))
    .bind(name)
    .bind(name)
    .bind(password_hash)
    .bind(role)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn create_motorcycle(pool: &SqlitePool, user_id: i64, make: &str, model: &str) -> i64 {
    sqlx::query("INSERT INTO motorcycles (make, model, userId, initialOdo) VALUES (?, ?, ?, 0)")
        .bind(make)
        .bind(model)
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid()
}

async fn setup() -> TestEnv {
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

    let alice_id = create_user(&pool, "alice", "user").await;
    let bob_id = create_user(&pool, "bob", "user").await;
    let admin_id = create_user(&pool, "root", "admin").await;
    let alice_session = create_session(&pool, alice_id).await.unwrap();
    let admin_session = create_session(&pool, admin_id).await.unwrap();

    TestEnv {
        app: build_app(state),
        pool,
        alice_session,
        alice_id,
        bob_id,
        admin_session,
        admin_id,
    }
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({ "_raw": String::from_utf8_lossy(&bytes) }))
}

/// Mint an API token through the real REST endpoint; returns the secret.
async fn mint_token(env: &TestEnv, session: &str, name: &str, scope: &str) -> String {
    let response = env
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/api-tokens")
                .header(header::AUTHORIZATION, format!("Bearer {session}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "name": name, "scope": scope }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = read_json(response).await;
    assert_eq!(body["apiToken"]["scope"], scope);
    assert!(
        body["apiToken"].get("tokenHash").is_none(),
        "hash must never be returned"
    );
    body["token"].as_str().unwrap().to_string()
}

async fn rpc(env: &TestEnv, bearer: &str, body: Value) -> (StatusCode, Value) {
    let response = env
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ACCEPT, "application/json, text/event-stream")
                .header("MCP-Protocol-Version", "2025-06-18")
                // Real HTTP/1.1 clients always send Host; `oneshot` does not.
                .header(header::HOST, "localhost")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (status, read_json(response).await)
}

async fn call_tool(env: &TestEnv, bearer: &str, name: &str, args: Value) -> Value {
    let (status, body) = rpc(
        env,
        bearer,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": name, "arguments": args }
        }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected status for {name}: {body}"
    );
    body
}

/// Text payload of a tool result, parsed as JSON when possible.
fn tool_text(body: &Value) -> String {
    body["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no text content in {body}"))
        .to_string()
}

fn tool_json(body: &Value) -> Value {
    assert_ne!(
        body["result"]["isError"],
        json!(true),
        "tool errored: {body}"
    );
    serde_json::from_str(&tool_text(body)).unwrap()
}

fn assert_tool_error(body: &Value, contains: &str) {
    assert_eq!(
        body["result"]["isError"],
        json!(true),
        "expected tool error: {body}"
    );
    let text = tool_text(body);
    assert!(text.contains(contains), "expected '{contains}' in '{text}'");
}

// MARK: - Authentication boundary

#[tokio::test]
async fn mcp_rejects_missing_session_and_garbage_credentials() {
    let env = setup().await;

    let (status, _) = rpc(
        &env,
        "not-a-token",
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // A browser/app session must not work on /mcp.
    let session = env.alice_session.clone();
    let (status, _) = rpc(
        &env,
        &session,
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // No Authorization header at all: 401 with a Bearer challenge.
    let response = env
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ACCEPT, "application/json, text/event-stream")
                .body(Body::from(
                    json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.headers().get(header::WWW_AUTHENTICATE).is_some());
}

#[tokio::test]
async fn api_tokens_are_not_accepted_on_the_rest_api() {
    let env = setup().await;
    let token = mint_token(&env, &env.alice_session, "cli", "write").await;

    let response = env
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/motorcycles")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Claude Code negotiates MCP 2026-07-28 through `server/discover` and then
/// sends header-addressed requests. That revision requires `ttlMs` and
/// `cacheScope` on every list result; a reply without them is rejected by
/// the client ("Invalid result for tools/list").
#[tokio::test]
async fn tools_list_carries_cache_hints_for_2026_07_28_clients() {
    let env = setup().await;
    let token = mint_token(&env, &env.alice_session, "claude-code", "read").await;

    let response = env
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ACCEPT, "application/json, text/event-stream")
                .header("MCP-Protocol-Version", "2026-07-28")
                .header("Mcp-Method", "tools/list")
                .header(header::HOST, "localhost")
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "tools/list",
                        "params": { "_meta": {
                            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                            "io.modelcontextprotocol/clientCapabilities": {},
                        } },
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = read_json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let result = &body["result"];
    assert_eq!(result["resultType"], "complete", "{body}");
    assert!(result["ttlMs"].is_u64(), "ttlMs must be a number: {body}");
    assert_eq!(result["cacheScope"], "private", "{body}");
    assert!(!result["tools"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn revoked_and_expired_tokens_stop_working() {
    let env = setup().await;
    let token = mint_token(&env, &env.alice_session, "laptop", "read").await;
    let (status, _) = rpc(
        &env,
        &token,
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Revoke via REST.
    let list = env
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/settings/api-tokens")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", env.alice_session),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let tokens = read_json(list).await;
    let id = tokens["apiTokens"][0]["id"].as_i64().unwrap();
    assert_eq!(tokens["apiTokens"][0]["tokenPrefix"], json!(&token[..11]));
    let response = env
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/settings/api-tokens/{id}"))
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", env.alice_session),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let (status, _) = rpc(
        &env,
        &token,
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Revoking again (or someone else's token) is a 404.
    let response = env
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/settings/api-tokens/{id}"))
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", env.alice_session),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Expired token: backdate expiresAt directly.
    let expiring = mint_token(&env, &env.alice_session, "short", "read").await;
    sqlx::query(
        "UPDATE apiTokens SET expiresAt = '2000-01-01T00:00:00+00:00' WHERE tokenPrefix = ?",
    )
    .bind(&expiring[..11])
    .execute(&env.pool)
    .await
    .unwrap();
    let (status, _) = rpc(
        &env,
        &expiring,
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn token_creation_validates_input_and_refuses_impersonation() {
    let env = setup().await;
    let post = |session: String, body: Value| {
        let app = env.app.clone();
        async move {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/settings/api-tokens")
                    .header(header::AUTHORIZATION, format!("Bearer {session}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
        }
    };

    assert_eq!(
        post(env.alice_session.clone(), json!({ "name": "  " }))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        post(
            env.alice_session.clone(),
            json!({ "name": "x", "scope": "admin" })
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        post(
            env.alice_session.clone(),
            json!({ "name": "x", "expiresInDays": 0 })
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        post(env.alice_session.clone(), json!({ "name": "x".repeat(65) }))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );

    let created = post(
        env.alice_session.clone(),
        json!({ "name": "phone", "expiresInDays": 30 }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let body = read_json(created).await;
    assert_eq!(body["apiToken"]["scope"], "read", "scope defaults to read");
    assert!(body["apiToken"]["expiresAt"].is_string());

    // An admin impersonating alice must not be able to mint a token for her.
    let impersonated = moto_manager_api::auth::session::create_impersonation_session(
        &env.pool,
        env.alice_id,
        env.admin_id,
    )
    .await
    .unwrap();
    assert_eq!(
        post(impersonated, json!({ "name": "sneaky" }))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}

// MARK: - Isolation

#[tokio::test]
async fn tools_only_see_the_token_owners_data_even_for_admins() {
    let env = setup().await;
    let alice_bike = create_motorcycle(&env.pool, env.alice_id, "BMW", "R1150GS").await;
    let bob_bike = create_motorcycle(&env.pool, env.bob_id, "Honda", "CB500").await;
    let admin_bike = create_motorcycle(&env.pool, env.admin_id, "Ducati", "Monster").await;

    let alice_token = mint_token(&env, &env.alice_session, "alice-cli", "write").await;
    let admin_token = mint_token(&env, &env.admin_session, "admin-cli", "write").await;

    let bikes = tool_json(&call_tool(&env, &alice_token, "list_motorcycles", json!({})).await);
    let ids: Vec<i64> = bikes["motorcycles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, vec![alice_bike]);

    // The admin's token is a plain user token: only the admin's own bike.
    let bikes = tool_json(&call_tool(&env, &admin_token, "list_motorcycles", json!({})).await);
    let ids: Vec<i64> = bikes["motorcycles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, vec![admin_bike]);

    // Writes against someone else's motorcycle are "not found", not applied.
    let body = call_tool(
        &env,
        &admin_token,
        "create_issue",
        json!({ "motorcycle_id": bob_bike, "title": "Hijack", "odo": 100 }),
    )
    .await;
    assert_tool_error(&body, "Not found");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM issues")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);

    // Expense attribution to a foreign motorcycle is refused as well.
    let body = call_tool(
        &env,
        &alice_token,
        "add_expense",
        json!({ "date": "2026-01-01", "amount": 10, "currency": "CHF", "category": "Steuern", "motorcycle_ids": [bob_bike] }),
    )
    .await;
    assert_tool_error(&body, "Not found");

    // No admin tool is exposed at all.
    let (_, list) = rpc(
        &env,
        &admin_token,
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
    )
    .await;
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"list_motorcycles") && names.contains(&"log_maintenance"),
        "{names:?}"
    );
    assert!(
        names
            .iter()
            .all(|n| !n.contains("admin") && !n.contains("user") && !n.contains("delete")),
        "{names:?}"
    );
    for tool in list["result"]["tools"].as_array().unwrap() {
        assert!(
            tool["annotations"]["readOnlyHint"].is_boolean(),
            "every tool declares readOnlyHint: {tool}"
        );
    }
}

// MARK: - Scope and audit

#[tokio::test]
async fn read_tokens_cannot_write_and_everything_is_audited() {
    let env = setup().await;
    let bike = create_motorcycle(&env.pool, env.alice_id, "BMW", "R1150GS").await;
    let read_token = mint_token(&env, &env.alice_session, "reader", "read").await;
    let write_token = mint_token(&env, &env.alice_session, "writer", "write").await;

    // Read works with a read token.
    let overview = tool_json(&call_tool(&env, &read_token, "get_fleet_overview", json!({})).await);
    assert_eq!(overview["motorcycles"][0]["id"], bike);

    // Write is denied with a read token and nothing is stored.
    let body = call_tool(
        &env,
        &read_token,
        "create_issue",
        json!({ "motorcycle_id": bike, "title": "Brake squeal", "odo": 12000 }),
    )
    .await;
    assert_tool_error(&body, "read-only");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM issues")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);

    // Same call with a write token succeeds.
    let created = tool_json(
        &call_tool(
            &env,
            &write_token,
            "create_issue",
            json!({ "motorcycle_id": bike, "title": "Brake squeal", "odo": 12000, "priority": "high" }),
        )
        .await,
    );
    let issue_id = created["issue"]["id"].as_i64().unwrap();
    assert_eq!(created["issue"]["priority"], "high");
    assert_eq!(created["issue"]["status"], "new");

    let done = tool_json(
        &call_tool(
            &env,
            &write_token,
            "update_issue_status",
            json!({ "motorcycle_id": bike, "issue_id": issue_id, "status": "done" }),
        )
        .await,
    );
    assert_eq!(done["issue"]["status"], "done");

    // Audit trail through the REST view: denied, ok, ok (+ the read).
    let response = env
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/settings/mcp-audit?limit=10")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", env.alice_session),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let audit = read_json(response).await;
    let entries = audit["entries"].as_array().unwrap();
    let summary: Vec<(String, String, String)> = entries
        .iter()
        .map(|e| {
            (
                e["tokenName"].as_str().unwrap().to_string(),
                e["tool"].as_str().unwrap().to_string(),
                e["outcome"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            ("writer".into(), "update_issue_status".into(), "ok".into()),
            ("writer".into(), "create_issue".into(), "ok".into()),
            ("reader".into(), "create_issue".into(), "denied".into()),
            ("reader".into(), "get_fleet_overview".into(), "ok".into()),
        ]
    );
    assert!(entries[1]["arguments"]
        .as_str()
        .unwrap()
        .contains("Brake squeal"));

    // Bob sees none of it.
    let bob_session = create_session(&env.pool, env.bob_id).await.unwrap();
    let response = env
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/settings/mcp-audit")
                .header(header::AUTHORIZATION, format!("Bearer {bob_session}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        read_json(response).await["entries"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

// MARK: - Validation and write semantics

#[tokio::test]
async fn write_tools_validate_input_before_touching_the_database() {
    let env = setup().await;
    let bike = create_motorcycle(&env.pool, env.alice_id, "BMW", "R1150GS").await;
    let token = mint_token(&env, &env.alice_session, "writer", "write").await;

    let base =
        json!({ "motorcycle_id": bike, "date": "2026-05-01", "odo": 50000, "type": "service" });
    let cases: Vec<(Value, &str)> = vec![
        (
            json!({ "motorcycle_id": bike, "date": "01.05.2026", "odo": 1, "type": "service" }),
            "YYYY-MM-DD",
        ),
        (
            json!({ "motorcycle_id": bike, "date": "2026-05-01", "odo": -5, "type": "service" }),
            "odo",
        ),
        (
            json!({ "motorcycle_id": bike, "date": "2026-05-01", "odo": 1, "type": "fuel" }),
            "type must be one of",
        ),
        (
            json!({ "motorcycle_id": bike, "date": "2026-05-01", "odo": 1, "type": "fluid" }),
            "fluid_type is required",
        ),
        (
            json!({ "motorcycle_id": bike, "date": "2026-05-01", "odo": 1, "type": "service", "cost": 120.0 }),
            "currency is required",
        ),
        (
            json!({ "motorcycle_id": bike, "date": "2026-05-01", "odo": 1, "type": "service", "cost": 120.0, "currency": "XXX" }),
            "not configured",
        ),
        (
            json!({ "motorcycle_id": bike, "date": "2026-05-01", "odo": 1, "type": "service", "cost": -1.0, "currency": "CHF" }),
            "cost must be",
        ),
        (
            json!({ "motorcycle_id": bike, "date": "2026-05-01", "odo": 1, "type": "service", "description": "x".repeat(4001) }),
            "at most 4000",
        ),
        (
            json!({ "motorcycle_id": bike, "date": "2026-05-01", "odo": 1, "type": "service", "idempotency_key": "abc" }),
            "idempotency_key",
        ),
        (
            json!({ "motorcycle_id": bike, "date": "2026-05-01", "odo": 1, "type": "service", "surprise": true }),
            "unknown field",
        ),
    ];
    for (args, expected) in cases {
        let body = call_tool(&env, &token, "log_maintenance", args.clone()).await;
        if body["result"].is_object() {
            assert_tool_error(&body, expected);
        } else {
            // Schema violations (unknown field) are protocol-level invalid params.
            let msg = body["error"]["message"].as_str().unwrap_or_default();
            assert!(
                msg.contains(expected),
                "args {args}: expected '{expected}' in '{msg}'"
            );
        }
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM maintenanceRecords")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "no invalid call may have created a record");

    // Valid call: record created, cost normalized through the currency table.
    let mut args = base.clone();
    args["cost"] = json!(180.5);
    args["currency"] = json!("chf");
    args["description"] = json!("  Annual service \u{0}  ");
    args["idempotency_key"] = json!("svc-2026-05-01-0001");
    let created = tool_json(&call_tool(&env, &token, "log_maintenance", args.clone()).await);
    let record = &created["maintenanceRecord"];
    assert_eq!(record["type"], "service");
    assert_eq!(record["cost"], 180.5);
    assert_eq!(record["currency"], "CHF");
    assert_eq!(record["normalizedCost"], 180.5);
    assert_eq!(record["description"], "Annual service");

    // Retry with the same idempotency key returns the same record.
    let again = tool_json(&call_tool(&env, &token, "log_maintenance", args).await);
    assert_eq!(again["maintenanceRecord"]["id"], record["id"]);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM maintenanceRecords")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // Fuel: derived price per litre and consumption from the previous stop.
    tool_json(
        &call_tool(
            &env,
            &token,
            "log_fuel",
            json!({ "motorcycle_id": bike, "date": "2026-05-02", "odo": 50100, "liters": 15.0 }),
        )
        .await,
    );
    let fuel = tool_json(&call_tool(&env, &token, "log_fuel", json!({ "motorcycle_id": bike, "date": "2026-05-10", "odo": 50400, "liters": 15.0, "total_cost": 27.0, "currency": "CHF" })).await);
    let rec = &fuel["maintenanceRecord"];
    assert_eq!(rec["type"], "fuel");
    assert_eq!(rec["fuelAmount"], 15.0);
    assert_eq!(rec["pricePerUnit"], 1.8);
    assert_eq!(rec["cost"], 27.0);
    assert_eq!(rec["tripDistance"], 300.0);
    assert_eq!(rec["fuelConsumption"], 5.0);

    let listed = tool_json(
        &call_tool(
            &env,
            &token,
            "list_maintenance",
            json!({ "motorcycle_id": bike, "types": ["fuel"], "limit": 1 }),
        )
        .await,
    );
    assert_eq!(listed["maintenanceRecords"].as_array().unwrap().len(), 1);
    assert_eq!(listed["maintenanceRecords"][0]["id"], rec["id"]);
    let body = call_tool(
        &env,
        &token,
        "list_maintenance",
        json!({ "motorcycle_id": bike, "types": ["nonsense"] }),
    )
    .await;
    assert_tool_error(&body, "types must be one of");
}

#[tokio::test]
async fn parts_tools_keep_the_catalogue_private_and_track_stock() {
    let env = setup().await;
    let token = mint_token(&env, &env.alice_session, "writer", "write").await;
    let bob_token = mint_token(
        &env,
        &create_session(&env.pool, env.bob_id).await.unwrap(),
        "bob",
        "write",
    )
    .await;

    let part = tool_json(
        &call_tool(&env, &token, "create_part", json!({ "part_number": "11 42 7 673 541", "name": "Oil filter", "manufacturer": "Mahle" })).await,
    );
    let part_id = part["part"]["id"].as_i64().unwrap();
    assert_eq!(part["part"]["isPublic"], false);

    tool_json(
        &call_tool(
            &env,
            &token,
            "add_part_stock",
            json!({ "part_id": part_id, "quantity": 3, "price": 12.5, "currency": "CHF" }),
        )
        .await,
    );
    let listed =
        tool_json(&call_tool(&env, &token, "list_parts", json!({ "search": "oil" })).await);
    assert_eq!(listed["parts"][0]["onHand"], 3);

    tool_json(
        &call_tool(
            &env,
            &token,
            "consume_part",
            json!({ "part_id": part_id, "quantity": 2, "date": "2026-05-01" }),
        )
        .await,
    );
    let listed = tool_json(&call_tool(&env, &token, "list_parts", json!({})).await);
    assert_eq!(listed["parts"][0]["onHand"], 1);
    let empty = tool_json(
        &call_tool(
            &env,
            &token,
            "list_parts",
            json!({ "out_of_stock_only": true }),
        )
        .await,
    );
    assert_eq!(empty["parts"].as_array().unwrap().len(), 0);

    // Bob cannot touch alice's part.
    let body = call_tool(
        &env,
        &bob_token,
        "add_part_stock",
        json!({ "part_id": part_id, "quantity": 1 }),
    )
    .await;
    assert_tool_error(&body, "Not found");
    let body = call_tool(
        &env,
        &token,
        "consume_part",
        json!({ "part_id": part_id, "quantity": 0 }),
    )
    .await;
    assert_tool_error(&body, "quantity must be");
}
