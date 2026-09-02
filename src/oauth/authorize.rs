//! Authorization endpoint and the consent exchange with the webapp.
//!
//! `GET /oauth/authorize` is where a client sends the user's browser. It
//! validates the request and forwards to the webapp's `/oauth/consent`
//! page — the API has no UI of its own, and the webapp already knows how to
//! get the user signed in and back. The page shows the verified client name
//! (from `GET /api/oauth/consent`) and posts the decision to
//! `POST /api/oauth/consent`, which mints the authorization code and returns
//! the redirect URL the browser must follow. Both API routes are session-
//! authenticated; the decision additionally refuses impersonation sessions
//! (a support admin must not be able to connect an AI client to a user's
//! account).

use std::collections::HashMap;

use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use chrono::{Duration, Utc};
use rand::Rng;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use url::Url;

use super::{
    check_resource, clients::load_client, parse_scope, OauthError, AUTH_CODE_TTL_MINUTES,
    MAX_STATE_LEN, SCOPES,
};
use crate::{
    auth::{api_token::hash_token, AuthUser, NotImpersonated},
    config::Config,
    error::{AppError, AppResult},
    models::OauthClient,
};

const CODE_RANDOM_BYTES: usize = 32;

/// `GET /oauth/authorize`
///
/// Errors in the client identity or redirect URI are answered directly (the
/// browser must not be sent to an unverified address); everything else is
/// reported to the client via its redirect URI as RFC 6749 §4.1.2.1 asks.
pub async fn authorize(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let get = |key: &str| {
        params
            .get(key)
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    };

    let client = match resolve_client(&pool, get("client_id"), get("redirect_uri")).await {
        Ok(client) => client,
        Err(e) => return e.into_response(),
    };
    let redirect_uri = get("redirect_uri").unwrap_or_default();
    let state = get("state");

    match validate_authorization_request(&config, &params) {
        Ok(()) => {}
        Err(e) => return error_redirect(redirect_uri, &e, state).into_response(),
    }

    let mut consent = match Url::parse(&format!("{}/oauth/consent", config.origin)) {
        Ok(url) => url,
        Err(e) => {
            return OauthError::server_error(format!("invalid ORIGIN configuration: {e}"))
                .into_response()
        }
    };
    {
        let mut query = consent.query_pairs_mut();
        query.append_pair("client_id", &client.client_id);
        query.append_pair("redirect_uri", redirect_uri);
        for key in [
            "scope",
            "state",
            "code_challenge",
            "code_challenge_method",
            "resource",
        ] {
            if let Some(value) = get(key) {
                query.append_pair(key, value);
            }
        }
    }
    Redirect::to(consent.as_str()).into_response()
}

async fn resolve_client(
    pool: &SqlitePool,
    client_id: Option<&str>,
    redirect_uri: Option<&str>,
) -> Result<OauthClient, OauthError> {
    let client_id =
        client_id.ok_or_else(|| OauthError::invalid_request("client_id is required"))?;
    let client = load_client(pool, client_id)
        .await?
        .ok_or_else(|| OauthError::invalid_client("unknown client_id"))?;
    let redirect_uri =
        redirect_uri.ok_or_else(|| OauthError::invalid_request("redirect_uri is required"))?;
    if !client.allows_redirect_uri(redirect_uri) {
        return Err(OauthError::invalid_request(
            "redirect_uri is not registered for this client",
        ));
    }
    Ok(client)
}

/// The checks that apply once the client and redirect URI are trusted.
fn validate_authorization_request(
    config: &Config,
    params: &HashMap<String, String>,
) -> Result<(), OauthError> {
    let get = |key: &str| {
        params
            .get(key)
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    };
    if get("response_type") != Some("code") {
        return Err(OauthError::invalid_request("response_type must be 'code'"));
    }
    validate_pkce_challenge(get("code_challenge"), get("code_challenge_method"))?;
    parse_scope(get("scope"))?;
    check_resource(config, get("resource"))?;
    if get("state").is_some_and(|s| s.len() > MAX_STATE_LEN) {
        return Err(OauthError::invalid_request("state is too long"));
    }
    Ok(())
}

/// OAuth 2.1 makes PKCE mandatory and only S256 is accepted.
pub fn validate_pkce_challenge(
    challenge: Option<&str>,
    method: Option<&str>,
) -> Result<(), OauthError> {
    let challenge = challenge
        .ok_or_else(|| OauthError::invalid_request("code_challenge is required (PKCE)"))?;
    if method != Some("S256") {
        return Err(OauthError::invalid_request(
            "code_challenge_method must be 'S256'",
        ));
    }
    // base64url(sha256) without padding is exactly 43 characters.
    let valid = challenge.len() == 43
        && challenge
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !valid {
        return Err(OauthError::invalid_request(
            "code_challenge must be a base64url-encoded SHA-256 digest",
        ));
    }
    Ok(())
}

/// Append query parameters to a redirect URI, keeping any query it already
/// has. Works for https, loopback and custom-scheme URIs alike.
fn append_query(redirect_uri: &str, pairs: &[(&str, &str)]) -> Result<Url, OauthError> {
    let mut url = Url::parse(redirect_uri)
        .map_err(|_| OauthError::invalid_request("redirect_uri is not a valid URL"))?;
    {
        let mut query = url.query_pairs_mut();
        for (key, value) in pairs {
            query.append_pair(key, value);
        }
    }
    Ok(url)
}

fn error_redirect(redirect_uri: &str, error: &OauthError, state: Option<&str>) -> Response {
    let mut pairs = vec![
        ("error", error.code()),
        ("error_description", error.description()),
    ];
    if let Some(state) = state {
        pairs.push(("state", state));
    }
    match append_query(redirect_uri, &pairs) {
        Ok(url) => Redirect::to(url.as_str()).into_response(),
        Err(e) => e.into_response(),
    }
}

// MARK: - Consent exchange with the webapp (session-authenticated)

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsentQuery {
    pub client_id: String,
    pub redirect_uri: String,
}

/// `GET /api/oauth/consent?clientId=&redirectUri=` — what the consent page
/// shows the user. Only a registered client/redirect pair is described, so
/// the page can never be talked into naming an unregistered target.
pub async fn consent_info(
    State(pool): State<SqlitePool>,
    AuthUser(_user): AuthUser,
    Query(query): Query<ConsentQuery>,
) -> AppResult<Json<Value>> {
    let client = load_client(&pool, &query.client_id)
        .await?
        .filter(|c| c.allows_redirect_uri(&query.redirect_uri))
        .ok_or_else(|| AppError::NotFound("Unknown client or redirect URI".to_string()))?;

    Ok(Json(json!({
        "client": {
            "clientId": client.client_id,
            "clientName": client.client_name,
            "clientUri": client.client_uri,
            "redirectHost": describe_redirect_target(&query.redirect_uri),
        },
        "scopes": SCOPES,
    })))
}

/// The part of the redirect URI worth showing a person: the host for web
/// targets, the scheme for native apps.
fn describe_redirect_target(redirect_uri: &str) -> String {
    match Url::parse(redirect_uri) {
        Ok(url) => match url.host_str() {
            Some(host) if !host.is_empty() => host.to_string(),
            _ => format!("{}://", url.scheme()),
        },
        Err(_) => redirect_uri.to_string(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsentDecision {
    pub client_id: String,
    pub redirect_uri: String,
    /// "read" or "write" — chosen by the user, independent of what the
    /// client asked for.
    pub scope: String,
    pub state: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: Option<String>,
    pub resource: Option<String>,
    /// "allow" or "deny".
    pub decision: String,
}

/// `POST /api/oauth/consent` — records the user's decision and returns the
/// URL the browser must be sent to. On approval that URL carries a single-
/// use authorization code bound to the client, redirect URI, PKCE challenge
/// and scope; on refusal it carries `error=access_denied`.
pub async fn consent_decide(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(user): AuthUser,
    _guard: NotImpersonated,
    Json(body): Json<ConsentDecision>,
) -> AppResult<Json<Value>> {
    let bad = |e: OauthError| AppError::BadRequest(format!("{}: {}", e.code(), e.description()));

    let client = load_client(&pool, &body.client_id)
        .await?
        .filter(|c| c.allows_redirect_uri(&body.redirect_uri))
        .ok_or_else(|| AppError::NotFound("Unknown client or redirect URI".to_string()))?;
    validate_pkce_challenge(
        Some(body.code_challenge.as_str()),
        body.code_challenge_method.as_deref(),
    )
    .map_err(bad)?;
    check_resource(&config, body.resource.as_deref()).map_err(bad)?;
    let state = body
        .state
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if state.is_some_and(|s| s.len() > MAX_STATE_LEN) {
        return Err(AppError::BadRequest("state is too long".to_string()));
    }

    match body.decision.as_str() {
        "deny" => {
            let mut pairs = vec![
                ("error", "access_denied"),
                ("error_description", "the user declined the request"),
            ];
            if let Some(state) = state {
                pairs.push(("state", state));
            }
            let url = append_query(&body.redirect_uri, &pairs).map_err(bad)?;
            tracing::info!(
                "User {} declined OAuth access for client {}",
                user.id,
                client.client_id
            );
            Ok(Json(json!({ "redirectUrl": url.as_str() })))
        }
        "allow" => {
            if !SCOPES.contains(&body.scope.as_str()) {
                return Err(AppError::BadRequest(
                    "scope must be 'read' or 'write'".to_string(),
                ));
            }
            let code = issue_code(&pool, &config, &client, user.id, &body).await?;
            let mut pairs = vec![("code", code.as_str()), ("iss", config.public_url.as_str())];
            if let Some(state) = state {
                pairs.push(("state", state));
            }
            let url = append_query(&body.redirect_uri, &pairs).map_err(bad)?;
            tracing::info!(
                "User {} granted '{}' OAuth access to client {} ({})",
                user.id,
                body.scope,
                client.client_id,
                client.client_name
            );
            Ok(Json(json!({ "redirectUrl": url.as_str() })))
        }
        _ => Err(AppError::BadRequest(
            "decision must be 'allow' or 'deny'".to_string(),
        )),
    }
}

async fn issue_code(
    pool: &SqlitePool,
    config: &Config,
    client: &OauthClient,
    user_id: i64,
    body: &ConsentDecision,
) -> AppResult<String> {
    let mut bytes = [0u8; CODE_RANDOM_BYTES];
    rand::rng().fill_bytes(&mut bytes);
    let code = hex::encode(bytes);
    let now = Utc::now();
    let expires_at = now + Duration::minutes(AUTH_CODE_TTL_MINUTES);
    // The code is bound to the canonical resource so the token endpoint can
    // compare without caring how the client spelled it.
    let resource = body
        .resource
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .map(|_| config.mcp_resource());

    sqlx::query("DELETE FROM oauthAuthorizationCodes WHERE expiresAt < ?")
        .bind(now.to_rfc3339())
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO oauthAuthorizationCodes (codeHash, clientId, userId, scope, redirectUri, \
         codeChallenge, resource, createdAt, expiresAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(hash_token(&code))
    .bind(client.id)
    .bind(user_id)
    .bind(&body.scope)
    .bind(&body.redirect_uri)
    .bind(&body.code_challenge)
    .bind(&resource)
    .bind(now.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_shape_is_enforced() {
        let ok = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(validate_pkce_challenge(Some(ok), Some("S256")).is_ok());
        assert!(validate_pkce_challenge(Some(ok), Some("plain")).is_err());
        assert!(validate_pkce_challenge(Some(ok), None).is_err());
        assert!(validate_pkce_challenge(None, Some("S256")).is_err());
        assert!(validate_pkce_challenge(Some("short"), Some("S256")).is_err());
        assert!(validate_pkce_challenge(Some(&format!("{ok}=")), Some("S256")).is_err());
    }

    #[test]
    fn query_is_appended_not_replaced() {
        let url = append_query(
            "https://app.example/cb?keep=1",
            &[("code", "abc"), ("state", "x y")],
        )
        .unwrap();
        assert_eq!(
            url.as_str(),
            "https://app.example/cb?keep=1&code=abc&state=x+y"
        );
        let native = append_query("claude://oauth/callback", &[("code", "abc")]).unwrap();
        assert_eq!(native.as_str(), "claude://oauth/callback?code=abc");
    }

    #[test]
    fn redirect_targets_are_described_for_humans() {
        assert_eq!(
            describe_redirect_target("https://claude.ai/api/cb"),
            "claude.ai"
        );
        assert_eq!(
            describe_redirect_target("http://localhost:3334/cb"),
            "localhost"
        );
        assert_eq!(describe_redirect_target("claude://oauth/callback"), "oauth");
    }
}
