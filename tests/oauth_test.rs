//! OAuth 2.1 for the MCP endpoint: discovery metadata, dynamic client
//! registration, the authorize → consent → code → token flow with PKCE,
//! refresh rotation with replay detection, revocation, and the guards that
//! keep the flow user-level (impersonation, redirect validation, resource
//! binding).

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use base64::Engine;
use moto_manager_api::{
    auth::{
        password::hash_password,
        session::{create_impersonation_session, create_session},
    },
    build_app, build_cors,
    config::Config,
    AppState,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use tower::ServiceExt;

const PUBLIC_URL: &str = "http://localhost:3001";
const WEB_ORIGIN: &str = "http://localhost:5173";
const REDIRECT_URI: &str = "https://claude.ai/api/mcp/auth_callback";
/// RFC 7636 appendix B test vector.
const CODE_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CODE_CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

struct TestEnv {
    app: axum::Router,
    pool: SqlitePool,
    alice_session: String,
    alice_id: i64,
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
        origin: WEB_ORIGIN.to_string(),
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
        public_url: PUBLIC_URL.to_string(),
    };
    let rp_origin = url::Url::parse(WEB_ORIGIN).unwrap();
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
    let admin_id = create_user(&pool, "root", "admin").await;
    let alice_session = create_session(&pool, alice_id).await.unwrap();

    TestEnv {
        // The CORS layer is part of the contract under test here.
        app: build_app(state).layer(build_cors(WEB_ORIGIN)),
        pool,
        alice_session,
        alice_id,
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

fn base_request(method: &str, uri: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(uri)
        // The rate limiter keys on the client IP; `oneshot` has no peer
        // address, so supply the forwarded header a reverse proxy would.
        .header("X-Forwarded-For", "203.0.113.7")
        .header(header::HOST, "localhost")
}

async fn get(env: &TestEnv, uri: &str) -> (StatusCode, axum::http::HeaderMap, Value) {
    let response = env
        .app
        .clone()
        .oneshot(base_request("GET", uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    (status, headers, read_json(response).await)
}

async fn post_json(
    env: &TestEnv,
    uri: &str,
    session: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut request = base_request("POST", uri).header(header::CONTENT_TYPE, "application/json");
    if let Some(session) = session {
        request = request.header(header::AUTHORIZATION, format!("Bearer {session}"));
    }
    let response = env
        .app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    (status, read_json(response).await)
}

async fn post_form(env: &TestEnv, uri: &str, fields: &[(&str, &str)]) -> (StatusCode, Value) {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields)
        .finish();
    let response = env
        .app
        .clone()
        .oneshot(
            base_request("POST", uri)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (status, read_json(response).await)
}

/// Registers a public client and returns its client_id.
async fn register_client(env: &TestEnv, name: &str) -> String {
    let (status, body) = post_json(
        env,
        "/oauth/register",
        None,
        json!({
            "client_name": name,
            "redirect_uris": [REDIRECT_URI],
            "token_endpoint_auth_method": "none",
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["client_name"], name);
    assert!(body.get("client_secret").is_none());
    body["client_id"].as_str().unwrap().to_string()
}

fn query_param(url: &str, key: &str) -> Option<String> {
    url::Url::parse(url)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}

/// Runs the browser part of the flow as the webapp would and returns the
/// authorization code.
async fn obtain_code(env: &TestEnv, client_id: &str, scope: &str) -> String {
    let (status, body) = post_json(
        env,
        "/api/oauth/consent",
        Some(&env.alice_session),
        json!({
            "clientId": client_id,
            "redirectUri": REDIRECT_URI,
            "scope": scope,
            "state": "xyz",
            "codeChallenge": CODE_CHALLENGE,
            "codeChallengeMethod": "S256",
            "resource": format!("{PUBLIC_URL}/mcp"),
            "decision": "allow",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let redirect_url = body["redirectUrl"].as_str().unwrap();
    assert!(redirect_url.starts_with(REDIRECT_URI), "{redirect_url}");
    assert_eq!(query_param(redirect_url, "state").as_deref(), Some("xyz"));
    assert_eq!(
        query_param(redirect_url, "iss").as_deref(),
        Some(PUBLIC_URL)
    );
    query_param(redirect_url, "code").expect("code in redirect")
}

async fn exchange_code(env: &TestEnv, client_id: &str, code: &str) -> (StatusCode, Value) {
    post_form(
        env,
        "/oauth/token",
        &[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", code),
            ("code_verifier", CODE_VERIFIER),
            ("redirect_uri", REDIRECT_URI),
            ("resource", &format!("{PUBLIC_URL}/mcp")),
        ],
    )
    .await
}

async fn mcp_tools_list(env: &TestEnv, bearer: &str) -> StatusCode {
    env.app
        .clone()
        .oneshot(
            base_request("POST", "/mcp")
                .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ACCEPT, "application/json, text/event-stream")
                .header("MCP-Protocol-Version", "2025-06-18")
                .body(Body::from(
                    json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn discovery_metadata_points_clients_at_the_flow() {
    let env = setup().await;

    // An unauthenticated MCP request advertises where to find the metadata.
    let response = env
        .app
        .clone()
        .oneshot(
            base_request("POST", "/mcp")
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
    let challenge = response
        .headers()
        .get(header::WWW_AUTHENTICATE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(challenge.starts_with("Bearer "), "{challenge}");
    assert!(
        challenge.contains(&format!(
            "resource_metadata=\"{PUBLIC_URL}/.well-known/oauth-protected-resource/mcp\""
        )),
        "{challenge}"
    );

    for path in [
        "/.well-known/oauth-protected-resource/mcp",
        "/.well-known/oauth-protected-resource",
    ] {
        let (status, _, body) = get(&env, path).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["resource"], format!("{PUBLIC_URL}/mcp"));
        assert_eq!(body["authorization_servers"], json!([PUBLIC_URL]));
        assert_eq!(body["scopes_supported"], json!(["read", "write"]));
    }

    let (status, _, body) = get(&env, "/.well-known/oauth-authorization-server").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["issuer"], PUBLIC_URL);
    assert_eq!(
        body["authorization_endpoint"],
        format!("{PUBLIC_URL}/oauth/authorize")
    );
    assert_eq!(body["token_endpoint"], format!("{PUBLIC_URL}/oauth/token"));
    assert_eq!(
        body["registration_endpoint"],
        format!("{PUBLIC_URL}/oauth/register")
    );
    assert_eq!(
        body["revocation_endpoint"],
        format!("{PUBLIC_URL}/oauth/revoke")
    );
    assert_eq!(body["code_challenge_methods_supported"], json!(["S256"]));
    assert_eq!(
        body["grant_types_supported"],
        json!(["authorization_code", "refresh_token"])
    );
    assert!(body["token_endpoint_auth_methods_supported"]
        .as_array()
        .unwrap()
        .contains(&json!("none")));
}

#[tokio::test]
async fn discovery_and_oauth_endpoints_allow_any_origin_but_the_api_does_not() {
    let env = setup().await;

    let cors = |uri: &str| {
        base_request("GET", uri)
            .header(header::ORIGIN, "https://claude.ai")
            .body(Body::empty())
            .unwrap()
    };
    let response = env
        .app
        .clone()
        .oneshot(cors("/.well-known/oauth-authorization-server"))
        .await
        .unwrap();
    assert_eq!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(|v| v.to_str().unwrap()),
        Some("https://claude.ai")
    );

    // Preflight for the token endpoint from a foreign origin succeeds too.
    let preflight = env
        .app
        .clone()
        .oneshot(
            base_request("OPTIONS", "/oauth/token")
                .header(header::ORIGIN, "https://inspector.example")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(preflight.status().is_success());
    assert_eq!(
        preflight
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(|v| v.to_str().unwrap()),
        Some("https://inspector.example")
    );

    // The regular API stays restricted to the webapp origin.
    let response = env.app.clone().oneshot(cors("/api/health")).await.unwrap();
    assert!(response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());
    let response = env
        .app
        .clone()
        .oneshot(
            base_request("GET", "/api/health")
                .header(header::ORIGIN, WEB_ORIGIN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(|v| v.to_str().unwrap()),
        Some(WEB_ORIGIN)
    );
}

#[tokio::test]
async fn registration_validates_client_metadata() {
    let env = setup().await;

    let (status, body) = post_json(&env, "/oauth/register", None, json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_redirect_uri");

    for bad in [
        "http://example.com/cb",
        "https://example.com/cb#fragment",
        "javascript:alert(1)",
        "not a url",
    ] {
        let (status, body) = post_json(
            &env,
            "/oauth/register",
            None,
            json!({ "redirect_uris": [bad] }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}: {body}");
        assert_eq!(body["error"], "invalid_redirect_uri", "{bad}");
    }

    let (status, body) = post_json(
        &env,
        "/oauth/register",
        None,
        json!({ "redirect_uris": [REDIRECT_URI], "token_endpoint_auth_method": "private_key_jwt" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_client_metadata");

    let (status, body) = post_json(
        &env,
        "/oauth/register",
        None,
        json!({ "redirect_uris": [REDIRECT_URI], "grant_types": ["implicit"] }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "invalid_client_metadata");

    // Minimal registration: loopback URI, no name → default name, public client.
    let (status, body) = post_json(
        &env,
        "/oauth/register",
        None,
        json!({ "redirect_uris": ["http://localhost:3334/callback"] }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["token_endpoint_auth_method"], "none");
    assert_eq!(body["client_name"], "Unbenannte Anwendung");
    assert!(body["client_id"].as_str().unwrap().starts_with("mmc_"));

    // Confidential client gets a secret back exactly once.
    let (status, body) = post_json(
        &env,
        "/oauth/register",
        None,
        json!({
            "client_name": "Server thing",
            "redirect_uris": [REDIRECT_URI],
            "token_endpoint_auth_method": "client_secret_post",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(body["client_secret"].as_str().unwrap().len() >= 32);
    assert_eq!(body["client_secret_expires_at"], 0);
}

#[tokio::test]
async fn authorize_redirects_to_the_consent_page_or_reports_errors_correctly() {
    let env = setup().await;
    let client_id = register_client(&env, "Claude").await;

    // Happy path: forwarded to the webapp with every parameter preserved.
    let query = format!(
        "response_type=code&client_id={client_id}&redirect_uri={}&scope=write&state=abc\
         &code_challenge={CODE_CHALLENGE}&code_challenge_method=S256&resource={}",
        urlencoding(REDIRECT_URI),
        urlencoding(&format!("{PUBLIC_URL}/mcp"))
    );
    let (status, location, _) = authorize(&env, query).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let location = location.unwrap();
    assert!(
        location.starts_with(&format!("{WEB_ORIGIN}/oauth/consent?")),
        "{location}"
    );
    assert_eq!(
        query_param(&location, "client_id").as_deref(),
        Some(client_id.as_str())
    );
    assert_eq!(
        query_param(&location, "redirect_uri").as_deref(),
        Some(REDIRECT_URI)
    );
    assert_eq!(query_param(&location, "scope").as_deref(), Some("write"));
    assert_eq!(query_param(&location, "state").as_deref(), Some("abc"));
    assert_eq!(
        query_param(&location, "code_challenge").as_deref(),
        Some(CODE_CHALLENGE)
    );
    assert_eq!(
        query_param(&location, "code_challenge_method").as_deref(),
        Some("S256")
    );

    // Unknown client or unregistered redirect: answered directly, never redirected.
    let (status, location, body) = authorize(
        &env,
        format!(
            "response_type=code&client_id=mmc_nope&redirect_uri={}",
            urlencoding(REDIRECT_URI)
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert!(location.is_none());
    assert_eq!(body["error"], "invalid_client");

    let (status, location, body) = authorize(
        &env,
        format!(
            "response_type=code&client_id={client_id}&redirect_uri={}",
            urlencoding("https://evil.example/cb")
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(location.is_none());

    // Missing PKCE: reported to the client through its redirect URI.
    let (status, location, _) = authorize(
        &env,
        format!(
            "response_type=code&client_id={client_id}&redirect_uri={}&state=s1",
            urlencoding(REDIRECT_URI)
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let location = location.unwrap();
    assert!(location.starts_with(REDIRECT_URI), "{location}");
    assert_eq!(
        query_param(&location, "error").as_deref(),
        Some("invalid_request")
    );
    assert_eq!(query_param(&location, "state").as_deref(), Some("s1"));

    // Wrong resource: invalid_target via redirect.
    let (_, location, _) = authorize(
        &env,
        format!(
        "response_type=code&client_id={client_id}&redirect_uri={}&code_challenge={CODE_CHALLENGE}\
         &code_challenge_method=S256&resource={}",
        urlencoding(REDIRECT_URI),
        urlencoding("https://other.example/mcp")
    ),
    )
    .await;
    assert_eq!(
        query_param(&location.unwrap(), "error").as_deref(),
        Some("invalid_target")
    );

    // Unknown scope: invalid_scope via redirect.
    let (_, location, _) = authorize(
        &env,
        format!(
        "response_type=code&client_id={client_id}&redirect_uri={}&code_challenge={CODE_CHALLENGE}\
         &code_challenge_method=S256&scope=admin",
        urlencoding(REDIRECT_URI)
    ),
    )
    .await;
    assert_eq!(
        query_param(&location.unwrap(), "error").as_deref(),
        Some("invalid_scope")
    );
}

async fn authorize(env: &TestEnv, query: String) -> (StatusCode, Option<String>, Value) {
    let response = env
        .app
        .clone()
        .oneshot(
            base_request("GET", &format!("/oauth/authorize?{query}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let location = response
        .headers()
        .get(header::LOCATION)
        .map(|v| v.to_str().unwrap().to_string());
    (status, location, read_json(response).await)
}

fn urlencoding(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

#[tokio::test]
async fn consent_info_requires_a_session_and_a_registered_pair() {
    let env = setup().await;
    let client_id = register_client(&env, "Claude").await;
    let uri = format!(
        "/api/oauth/consent?clientId={client_id}&redirectUri={}",
        urlencoding(REDIRECT_URI)
    );

    let (status, _, _) = get(&env, &uri).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let response = env
        .app
        .clone()
        .oneshot(
            base_request("GET", &uri)
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
    let body = read_json(response).await;
    assert_eq!(body["client"]["clientId"], client_id);
    assert_eq!(body["client"]["clientName"], "Claude");
    assert_eq!(body["client"]["redirectHost"], "claude.ai");
    assert_eq!(body["scopes"], json!(["read", "write"]));

    let response = env
        .app
        .clone()
        .oneshot(
            base_request(
                "GET",
                &format!(
                    "/api/oauth/consent?clientId={client_id}&redirectUri={}",
                    urlencoding("https://evil.example/cb")
                ),
            )
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
}

#[tokio::test]
async fn full_flow_issues_a_working_mcp_token_and_rotates_refresh_tokens() {
    let env = setup().await;
    let client_id = register_client(&env, "Claude").await;

    let code = obtain_code(&env, &client_id, "read").await;
    let (status, body) = exchange_code(&env, &client_id, &code).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["scope"], "read");
    assert_eq!(body["expires_in"], 3600);
    let access = body["access_token"].as_str().unwrap().to_string();
    let refresh = body["refresh_token"].as_str().unwrap().to_string();
    assert!(access.starts_with("mm_"));
    assert!(refresh.starts_with("mmr_"));

    // The access token is a real MCP credential; the refresh token is not.
    assert_eq!(mcp_tools_list(&env, &access).await, StatusCode::OK);
    assert_eq!(
        mcp_tools_list(&env, &refresh).await,
        StatusCode::UNAUTHORIZED
    );

    // It shows up in the user's token list as an OAuth grant named after the client.
    let response = env
        .app
        .clone()
        .oneshot(
            base_request("GET", "/api/settings/api-tokens")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", env.alice_session),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let list = read_json(response).await;
    let tokens = list["apiTokens"].as_array().unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0]["name"], "Claude");
    assert_eq!(tokens[0]["kind"], "oauth");
    assert_eq!(tokens[0]["scope"], "read");
    assert!(tokens[0]["expiresAt"].is_string());
    assert!(tokens[0].get("refreshTokenHash").is_none());
    let token_id = tokens[0]["id"].as_i64().unwrap();

    // A second exchange of the same code is a replay: refused, and the
    // token it produced is revoked.
    let (status, body) = exchange_code(&env, &client_id, &code).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_grant", "{body}");
    assert_eq!(
        mcp_tools_list(&env, &access).await,
        StatusCode::UNAUTHORIZED
    );

    // Fresh grant for the rest of the test.
    let code = obtain_code(&env, &client_id, "write").await;
    let (_, body) = exchange_code(&env, &client_id, &code).await;
    let access1 = body["access_token"].as_str().unwrap().to_string();
    let refresh1 = body["refresh_token"].as_str().unwrap().to_string();
    assert_eq!(body["scope"], "write");
    assert_eq!(mcp_tools_list(&env, &access1).await, StatusCode::OK);

    // Re-authorizing the same client replaced the earlier (revoked) grant:
    // still exactly one active token for alice.
    let active: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM apiTokens WHERE userId = ? AND revokedAt IS NULL")
            .bind(env.alice_id)
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert_eq!(active, 1);
    let new_id: i64 =
        sqlx::query_scalar("SELECT id FROM apiTokens WHERE userId = ? AND revokedAt IS NULL")
            .bind(env.alice_id)
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert_ne!(new_id, token_id);

    // Refresh: new access + refresh secrets, old access token stops working.
    let (status, body) = post_form(
        &env,
        "/oauth/token",
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &client_id),
            ("refresh_token", &refresh1),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let access2 = body["access_token"].as_str().unwrap().to_string();
    let refresh2 = body["refresh_token"].as_str().unwrap().to_string();
    assert_ne!(access2, access1);
    assert_ne!(refresh2, refresh1);
    assert_eq!(body["scope"], "write");
    assert_eq!(
        mcp_tools_list(&env, &access1).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(mcp_tools_list(&env, &access2).await, StatusCode::OK);

    // The grant is still the same row (audit history and the revoke entry stay put).
    let active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM apiTokens WHERE userId = ? AND revokedAt IS NULL AND id = ?",
    )
    .bind(env.alice_id)
    .bind(new_id)
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert_eq!(active, 1);

    // Replaying the rotated-away refresh token revokes the whole grant.
    let (status, body) = post_form(
        &env,
        "/oauth/token",
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &client_id),
            ("refresh_token", &refresh1),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_grant", "{body}");
    assert_eq!(
        mcp_tools_list(&env, &access2).await,
        StatusCode::UNAUTHORIZED
    );
    let (status, body) = post_form(
        &env,
        "/oauth/token",
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &client_id),
            ("refresh_token", &refresh2),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn token_endpoint_verifies_pkce_client_redirect_and_resource() {
    let env = setup().await;
    let client_id = register_client(&env, "Claude").await;
    let other_client = register_client(&env, "Other").await;

    let attempt = |fields: Vec<(&'static str, String)>| {
        let env = &env;
        async move {
            let owned: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (*k, v.as_str())).collect();
            post_form(env, "/oauth/token", &owned).await
        }
    };

    // Wrong verifier.
    let code = obtain_code(&env, &client_id, "read").await;
    let (status, body) = attempt(vec![
        ("grant_type", "authorization_code".into()),
        ("client_id", client_id.clone()),
        ("code", code.clone()),
        (
            "code_verifier",
            "wrong-verifier-wrong-verifier-wrong-verifier-wrong".into(),
        ),
        ("redirect_uri", REDIRECT_URI.into()),
    ])
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_grant", "{body}");

    // Wrong redirect_uri (the code is still unused after the failed attempt).
    let (_, body) = attempt(vec![
        ("grant_type", "authorization_code".into()),
        ("client_id", client_id.clone()),
        ("code", code.clone()),
        ("code_verifier", CODE_VERIFIER.into()),
        ("redirect_uri", "https://claude.ai/other".into()),
    ])
    .await;
    assert_eq!(body["error"], "invalid_grant", "{body}");

    // Another client cannot redeem it.
    let (_, body) = attempt(vec![
        ("grant_type", "authorization_code".into()),
        ("client_id", other_client.clone()),
        ("code", code.clone()),
        ("code_verifier", CODE_VERIFIER.into()),
        ("redirect_uri", REDIRECT_URI.into()),
    ])
    .await;
    assert_eq!(body["error"], "invalid_grant", "{body}");

    // Foreign resource.
    let (_, body) = attempt(vec![
        ("grant_type", "authorization_code".into()),
        ("client_id", client_id.clone()),
        ("code", code.clone()),
        ("code_verifier", CODE_VERIFIER.into()),
        ("redirect_uri", REDIRECT_URI.into()),
        ("resource", "https://other.example/mcp".into()),
    ])
    .await;
    assert_eq!(body["error"], "invalid_target", "{body}");

    // Unknown grant type / client / missing form body.
    let (status, body) = attempt(vec![
        ("grant_type", "password".into()),
        ("client_id", client_id.clone()),
    ])
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "unsupported_grant_type", "{body}");
    let (status, body) = attempt(vec![
        ("grant_type", "authorization_code".into()),
        ("client_id", "mmc_missing".into()),
    ])
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "invalid_client", "{body}");
    let (status, body) = post_json(&env, "/oauth/token", None, json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_request", "{body}");

    // The correct exchange still works after all the failed ones.
    let (status, body) = exchange_code(&env, &client_id, &code).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // Expired codes are rejected.
    let code = obtain_code(&env, &client_id, "read").await;
    sqlx::query("UPDATE oauthAuthorizationCodes SET expiresAt = '2000-01-01T00:00:00Z'")
        .execute(&env.pool)
        .await
        .unwrap();
    let (status, body) = exchange_code(&env, &client_id, &code).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_grant", "{body}");
}

#[tokio::test]
async fn confidential_clients_must_authenticate() {
    let env = setup().await;
    let (status, body) = post_json(
        &env,
        "/oauth/register",
        None,
        json!({
            "client_name": "Backend",
            "redirect_uris": [REDIRECT_URI],
            "token_endpoint_auth_method": "client_secret_basic",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let client_id = body["client_id"].as_str().unwrap().to_string();
    let secret = body["client_secret"].as_str().unwrap().to_string();

    let code = obtain_code(&env, &client_id, "read").await;

    // No secret → invalid_client with a Basic challenge.
    let (status, body) = exchange_code(&env, &client_id, &code).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["error"], "invalid_client");

    // Basic credentials work.
    let basic = base64::engine::general_purpose::STANDARD.encode(format!("{client_id}:{secret}"));
    let form = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("code_verifier", CODE_VERIFIER),
            ("redirect_uri", REDIRECT_URI),
        ])
        .finish();
    let response = env
        .app
        .clone()
        .oneshot(
            base_request("POST", "/oauth/token")
                .header(header::AUTHORIZATION, format!("Basic {basic}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert!(body["access_token"].is_string());

    // Wrong secret via the form is refused.
    let (status, _) = post_form(
        &env,
        "/oauth/token",
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &client_id),
            ("client_secret", "nope"),
            ("refresh_token", body["refresh_token"].as_str().unwrap()),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn consent_rejects_impersonation_bad_input_and_records_denials() {
    let env = setup().await;
    let client_id = register_client(&env, "Claude").await;

    // Deny: error redirect with state, no code, nothing stored.
    let (status, body) = post_json(
        &env,
        "/api/oauth/consent",
        Some(&env.alice_session),
        json!({
            "clientId": client_id,
            "redirectUri": REDIRECT_URI,
            "scope": "read",
            "state": "st",
            "codeChallenge": CODE_CHALLENGE,
            "codeChallengeMethod": "S256",
            "decision": "deny",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let url = body["redirectUrl"].as_str().unwrap();
    assert_eq!(query_param(url, "error").as_deref(), Some("access_denied"));
    assert_eq!(query_param(url, "state").as_deref(), Some("st"));
    assert!(query_param(url, "code").is_none());
    let codes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauthAuthorizationCodes")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(codes, 0);

    // Bad scope, bad PKCE, unknown decision, unregistered redirect.
    let base = json!({
        "clientId": client_id,
        "redirectUri": REDIRECT_URI,
        "scope": "read",
        "codeChallenge": CODE_CHALLENGE,
        "codeChallengeMethod": "S256",
        "decision": "allow",
    });
    let with = |patch: Value| {
        let mut v = base.clone();
        for (k, val) in patch.as_object().unwrap() {
            v[k] = val.clone();
        }
        v
    };
    for (patch, expected) in [
        (json!({ "scope": "admin" }), StatusCode::BAD_REQUEST),
        (
            json!({ "codeChallengeMethod": "plain" }),
            StatusCode::BAD_REQUEST,
        ),
        (json!({ "codeChallenge": "short" }), StatusCode::BAD_REQUEST),
        (json!({ "decision": "maybe" }), StatusCode::BAD_REQUEST),
        (
            json!({ "resource": "https://other.example/mcp" }),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({ "redirectUri": "https://evil.example/cb" }),
            StatusCode::NOT_FOUND,
        ),
        (json!({ "clientId": "mmc_missing" }), StatusCode::NOT_FOUND),
    ] {
        let (status, body) = post_json(
            &env,
            "/api/oauth/consent",
            Some(&env.alice_session),
            with(patch.clone()),
        )
        .await;
        assert_eq!(status, expected, "{patch}: {body}");
    }

    // A support admin impersonating alice cannot connect a client to her account.
    let impersonation = create_impersonation_session(&env.pool, env.alice_id, env.admin_id)
        .await
        .unwrap();
    let (status, _) = post_json(
        &env,
        "/api/oauth/consent",
        Some(&impersonation),
        base.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // No session at all.
    let (status, _) = post_json(&env, "/api/oauth/consent", None, base).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn grants_can_be_revoked_by_the_client_and_by_the_user() {
    let env = setup().await;
    let client_id = register_client(&env, "Claude").await;

    // Client-side revocation (RFC 7009) via the refresh token.
    let code = obtain_code(&env, &client_id, "read").await;
    let (_, body) = exchange_code(&env, &client_id, &code).await;
    let access = body["access_token"].as_str().unwrap().to_string();
    let refresh = body["refresh_token"].as_str().unwrap().to_string();
    assert_eq!(mcp_tools_list(&env, &access).await, StatusCode::OK);
    let (status, _) = post_form(
        &env,
        "/oauth/revoke",
        &[("client_id", &client_id), ("token", &refresh)],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        mcp_tools_list(&env, &access).await,
        StatusCode::UNAUTHORIZED
    );
    // Unknown tokens are answered identically.
    let (status, _) = post_form(
        &env,
        "/oauth/revoke",
        &[("client_id", &client_id), ("token", "mm_nothing")],
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // User-side revocation through the existing settings endpoint.
    let code = obtain_code(&env, &client_id, "read").await;
    let (_, body) = exchange_code(&env, &client_id, &code).await;
    let access = body["access_token"].as_str().unwrap().to_string();
    let refresh = body["refresh_token"].as_str().unwrap().to_string();
    let id: i64 = sqlx::query_scalar(
        "SELECT id FROM apiTokens WHERE userId = ? AND revokedAt IS NULL AND kind = 'oauth'",
    )
    .bind(env.alice_id)
    .fetch_one(&env.pool)
    .await
    .unwrap();
    let response = env
        .app
        .clone()
        .oneshot(
            base_request("DELETE", &format!("/api/settings/api-tokens/{id}"))
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
    assert_eq!(
        mcp_tools_list(&env, &access).await,
        StatusCode::UNAUTHORIZED
    );
    let (status, body) = post_form(
        &env,
        "/oauth/token",
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &client_id),
            ("refresh_token", &refresh),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_grant", "{body}");
}

#[tokio::test]
async fn personal_tokens_keep_their_old_contract() {
    let env = setup().await;
    // Old-shape request (no new fields) still creates a personal token and
    // the response gains only the additive `kind` key.
    let (status, body) = post_json(
        &env,
        "/api/settings/api-tokens",
        Some(&env.alice_session),
        json!({ "name": "laptop", "scope": "read" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["apiToken"]["kind"], "personal");
    assert_eq!(body["apiToken"]["name"], "laptop");
    assert!(body["apiToken"]["expiresAt"].is_null());
    let secret = body["token"].as_str().unwrap();
    assert_eq!(mcp_tools_list(&env, secret).await, StatusCode::OK);
    // PKCE test vector sanity: keep the constants honest.
    assert_eq!(
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(CODE_VERIFIER)),
        CODE_CHALLENGE
    );
}
