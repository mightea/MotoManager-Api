//! OAuth 2.1 authorization server for the MCP endpoint (phase 2 of the MCP
//! integration). Connector-style clients — claude.ai, Claude Desktop, the
//! Claude mobile apps, MCP Inspector — cannot be handed a pasted secret; they
//! discover this server through the MCP authorization flow and obtain a
//! token via a browser consent screen instead:
//!
//! 1. `GET /mcp` without a token answers 401 with
//!    `WWW-Authenticate: Bearer resource_metadata="…"`, pointing at the
//!    protected-resource metadata (RFC 9728), which names this server as
//!    the authorization server (RFC 8414 metadata under `/.well-known`).
//! 2. The client registers itself (`POST /oauth/register`, RFC 7591) and
//!    sends the user's browser to `GET /oauth/authorize` with a PKCE
//!    challenge. That endpoint validates the request and forwards to the
//!    webapp's consent page, where the signed-in user picks a scope.
//! 3. The webapp posts the decision to `/api/oauth/consent` (session-
//!    authenticated), gets back the redirect URL carrying a single-use
//!    authorization code, and sends the browser to the client.
//! 4. The client trades the code for tokens at `POST /oauth/token` (PKCE
//!    verified) and refreshes them there later; `POST /oauth/revoke` ends a
//!    grant early.
//!
//! Tokens issued this way are ordinary rows in `apiTokens` (`kind = 'oauth'`),
//! so the `/mcp` middleware, the read/write scope gate, the audit log and the
//! revocation UI in both apps apply unchanged. Access tokens are short-lived
//! and refreshed with rotating refresh tokens; a replayed refresh token or
//! authorization code revokes the grant it belongs to.

pub mod authorize;
pub mod clients;
pub mod token;

use axum::{
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;

use crate::{config::Config, AppState};

/// Scopes in ascending order of power; `write` implies `read`.
pub const SCOPES: [&str; 2] = ["read", "write"];
pub const AUTH_CODE_TTL_MINUTES: i64 = 10;
pub const ACCESS_TOKEN_TTL_SECONDS: i64 = 60 * 60;
pub const REFRESH_TOKEN_TTL_DAYS: i64 = 90;
pub const MAX_STATE_LEN: usize = 1024;

/// Public, unauthenticated OAuth endpoints. The mutable ones (register,
/// authorize, token, revoke) are rate-limited by the caller; the metadata
/// documents are static and go on the plain router.
pub fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/oauth/register", post(clients::register))
        .route("/oauth/authorize", get(authorize::authorize))
        .route("/oauth/token", post(token::token))
        .route("/oauth/revoke", post(token::revoke))
}

pub fn metadata_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(authorization_server_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            get(protected_resource_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(protected_resource_metadata),
        )
}

/// Where the protected-resource metadata lives; advertised on every 401 from
/// `/mcp` so clients can start the flow without prior configuration.
pub fn protected_resource_metadata_url(config: &Config) -> String {
    format!(
        "{}/.well-known/oauth-protected-resource/mcp",
        config.public_url
    )
}

/// RFC 8414 authorization server metadata.
async fn authorization_server_metadata(State(config): State<Config>) -> Response {
    let base = &config.public_url;
    let auth_methods = json!(["none", "client_secret_post", "client_secret_basic"]);
    cacheable_json(json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/oauth/authorize"),
        "token_endpoint": format!("{base}/oauth/token"),
        "registration_endpoint": format!("{base}/oauth/register"),
        "revocation_endpoint": format!("{base}/oauth/revoke"),
        "response_types_supported": ["code"],
        "response_modes_supported": ["query"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": auth_methods,
        "revocation_endpoint_auth_methods_supported": auth_methods,
        "scopes_supported": SCOPES,
        "authorization_response_iss_parameter_supported": true,
    }))
}

/// RFC 9728 protected resource metadata for `/mcp`.
async fn protected_resource_metadata(State(config): State<Config>) -> Response {
    cacheable_json(json!({
        "resource": config.mcp_resource(),
        "resource_name": "MotoManager MCP",
        "authorization_servers": [config.public_url],
        "scopes_supported": SCOPES,
        "bearer_methods_supported": ["header"],
    }))
}

fn cacheable_json(value: serde_json::Value) -> Response {
    let mut response = Json(value).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=3600"),
    );
    response
}

/// RFC 6749 §5.2 error response: `{"error", "error_description"}` with the
/// status the spec prescribes and caching disabled.
#[derive(Debug)]
pub struct OauthError {
    status: StatusCode,
    error: &'static str,
    description: String,
}

impl OauthError {
    fn new(status: StatusCode, error: &'static str, description: impl Into<String>) -> Self {
        Self {
            status,
            error,
            description: description.into(),
        }
    }

    pub fn invalid_request(description: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", description)
    }

    pub fn invalid_client(description: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "invalid_client", description)
    }

    pub fn invalid_grant(description: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_grant", description)
    }

    pub fn invalid_scope(description: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_scope", description)
    }

    pub fn invalid_target(description: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_target", description)
    }

    pub fn invalid_client_metadata(description: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_client_metadata",
            description,
        )
    }

    pub fn invalid_redirect_uri(description: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_redirect_uri", description)
    }

    pub fn unsupported_grant_type(description: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
            description,
        )
    }

    pub fn server_error(description: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server_error",
            description,
        )
    }

    pub fn code(&self) -> &'static str {
        self.error
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

impl From<sqlx::Error> for OauthError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!("Database error in OAuth handler: {}", e);
        Self::server_error("database error")
    }
}

impl IntoResponse for OauthError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(json!({
                "error": self.error,
                "error_description": self.description,
            })),
        )
            .into_response();
        no_store(response.headers_mut());
        if self.status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_static("Basic realm=\"MotoManager OAuth\""),
            );
        }
        response
    }
}

/// Token responses must never be cached (RFC 6749 §5.1).
pub fn no_store(headers: &mut header::HeaderMap) {
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
}

/// Reduce a space-separated scope string to the single scope MotoManager
/// tokens carry: `write` if requested, otherwise `read`; `None` when the
/// client asked for nothing (the user then picks on the consent screen).
pub fn parse_scope(value: Option<&str>) -> Result<Option<&'static str>, OauthError> {
    let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let mut wants_write = false;
    for item in value.split_ascii_whitespace() {
        match item {
            "read" => {}
            "write" => wants_write = true,
            other => {
                return Err(OauthError::invalid_scope(format!(
                    "unknown scope '{other}'; supported: read, write"
                )))
            }
        }
    }
    Ok(Some(if wants_write { "write" } else { "read" }))
}

/// RFC 8707: a client may bind the request to a resource. Only this server's
/// MCP endpoint exists, so anything else is a misdirected request.
pub fn check_resource(config: &Config, resource: Option<&str>) -> Result<(), OauthError> {
    match resource.map(str::trim).filter(|r| !r.is_empty()) {
        None => Ok(()),
        Some(r) if r.trim_end_matches('/') == config.mcp_resource() => Ok(()),
        Some(r) => Err(OauthError::invalid_target(format!(
            "unknown resource '{r}'; this server only issues tokens for {}",
            config.mcp_resource()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_reduces_to_the_strongest_known_scope() {
        assert_eq!(parse_scope(None).unwrap(), None);
        assert_eq!(parse_scope(Some("  ")).unwrap(), None);
        assert_eq!(parse_scope(Some("read")).unwrap(), Some("read"));
        assert_eq!(parse_scope(Some("read write")).unwrap(), Some("write"));
        assert_eq!(parse_scope(Some("write")).unwrap(), Some("write"));
        assert_eq!(
            parse_scope(Some("admin")).unwrap_err().code(),
            "invalid_scope"
        );
    }
}
