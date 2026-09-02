//! Dynamic client registration (RFC 7591) and the client store shared by the
//! authorize and token endpoints.
//!
//! Registration is open — that is what lets claude.ai or Claude Desktop
//! connect without an admin pre-registering them — but it grants nothing by
//! itself: a client only ever receives tokens after a signed-in user approves
//! it on the consent screen. Registrations are validated (exact redirect
//! URIs, https or loopback), rate-limited by the caller, and registrations
//! that never led to a grant are pruned after a day.

use axum::{
    extract::{rejection::JsonRejection, State},
    http::StatusCode,
    Json,
};
use base64::Engine;
use chrono::{Duration, Utc};
use rand::Rng;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use url::Url;

use super::OauthError;
use crate::{auth::api_token::hash_token, models::OauthClient};

const CLIENT_ID_PREFIX: &str = "mmc_";
const CLIENT_ID_RANDOM_BYTES: usize = 16;
const CLIENT_SECRET_RANDOM_BYTES: usize = 32;
pub const MAX_REDIRECT_URIS: usize = 10;
pub const MAX_URI_LEN: usize = 2000;
/// Matches the API token name cap: the client name becomes the token name.
pub const MAX_CLIENT_NAME_LEN: usize = 64;
pub const DEFAULT_CLIENT_NAME: &str = "Unbenannte Anwendung";
const UNUSED_CLIENT_RETENTION_HOURS: i64 = 24;

pub const AUTH_METHODS: [&str; 3] = ["none", "client_secret_post", "client_secret_basic"];
const GRANT_TYPES: [&str; 2] = ["authorization_code", "refresh_token"];

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub redirect_uris: Option<Vec<String>>,
    pub client_name: Option<String>,
    pub token_endpoint_auth_method: Option<String>,
    pub grant_types: Option<Vec<String>>,
    pub response_types: Option<Vec<String>>,
    pub client_uri: Option<String>,
}

/// `POST /oauth/register`
pub async fn register(
    State(pool): State<SqlitePool>,
    body: Result<Json<RegisterRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<Value>), OauthError> {
    let Json(body) = body.map_err(|e| {
        OauthError::invalid_client_metadata(format!("request body must be JSON: {e}"))
    })?;

    let redirect_uris = body.redirect_uris.unwrap_or_default();
    if redirect_uris.is_empty() {
        return Err(OauthError::invalid_redirect_uri(
            "redirect_uris must contain at least one URI",
        ));
    }
    if redirect_uris.len() > MAX_REDIRECT_URIS {
        return Err(OauthError::invalid_redirect_uri(format!(
            "at most {MAX_REDIRECT_URIS} redirect_uris are allowed"
        )));
    }
    for uri in &redirect_uris {
        validate_redirect_uri(uri)?;
    }

    let client_name = sanitize_client_name(body.client_name.as_deref());

    let auth_method = body
        .token_endpoint_auth_method
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .unwrap_or("none");
    if !AUTH_METHODS.contains(&auth_method) {
        return Err(OauthError::invalid_client_metadata(format!(
            "unsupported token_endpoint_auth_method '{auth_method}'"
        )));
    }

    if let Some(grant_types) = &body.grant_types {
        if let Some(unknown) = grant_types
            .iter()
            .find(|g| !GRANT_TYPES.contains(&g.as_str()))
        {
            return Err(OauthError::invalid_client_metadata(format!(
                "unsupported grant_type '{unknown}'"
            )));
        }
    }
    if let Some(response_types) = &body.response_types {
        if let Some(unknown) = response_types.iter().find(|r| r.as_str() != "code") {
            return Err(OauthError::invalid_client_metadata(format!(
                "unsupported response_type '{unknown}'"
            )));
        }
    }

    let client_uri = match body.client_uri.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(uri) => {
            let parsed = Url::parse(uri)
                .ok()
                .filter(|u| matches!(u.scheme(), "http" | "https") && uri.len() <= MAX_URI_LEN)
                .ok_or_else(|| {
                    OauthError::invalid_client_metadata("client_uri must be an http(s) URL")
                })?;
            Some(parsed.to_string())
        }
    };

    prune_unused_clients(&pool).await?;

    let client_id = generate_client_id();
    let secret = if auth_method == "none" {
        None
    } else {
        Some(generate_client_secret())
    };
    let secret_hash = secret.as_deref().map(hash_token);
    let now = Utc::now();
    let redirect_uris_json = serde_json::to_string(&redirect_uris)
        .map_err(|e| OauthError::server_error(e.to_string()))?;

    sqlx::query(
        "INSERT INTO oauthClients (clientId, clientName, redirectUris, tokenEndpointAuthMethod, \
         clientSecretHash, clientUri, createdAt) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&client_id)
    .bind(&client_name)
    .bind(&redirect_uris_json)
    .bind(auth_method)
    .bind(&secret_hash)
    .bind(&client_uri)
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await?;

    tracing::info!(
        "OAuth client {} registered as '{}' with {} redirect URI(s)",
        client_id,
        client_name,
        redirect_uris.len()
    );

    let mut response = json!({
        "client_id": client_id,
        "client_id_issued_at": now.timestamp(),
        "client_name": client_name,
        "redirect_uris": redirect_uris,
        "token_endpoint_auth_method": auth_method,
        "grant_types": GRANT_TYPES,
        "response_types": ["code"],
        "scope": super::SCOPES.join(" "),
    });
    if let Some(uri) = client_uri {
        response["client_uri"] = json!(uri);
    }
    if let Some(secret) = secret {
        response["client_secret"] = json!(secret);
        response["client_secret_expires_at"] = json!(0);
    }

    Ok((StatusCode::CREATED, Json(response)))
}

/// Exact-match redirect URIs only. `https` anywhere; plain `http` only on a
/// loopback host (native apps listening locally); custom schemes for native
/// apps; never anything that could execute in a browser context.
pub fn validate_redirect_uri(uri: &str) -> Result<(), OauthError> {
    if uri.len() > MAX_URI_LEN || uri.trim() != uri || uri.is_empty() {
        return Err(OauthError::invalid_redirect_uri(
            "redirect_uri must be a non-empty, trimmed URL",
        ));
    }
    let parsed = Url::parse(uri)
        .map_err(|_| OauthError::invalid_redirect_uri(format!("'{uri}' is not a valid URL")))?;
    if parsed.fragment().is_some() {
        return Err(OauthError::invalid_redirect_uri(
            "redirect_uri must not contain a fragment",
        ));
    }
    match parsed.scheme() {
        "https" => Ok(()),
        "http" => {
            let loopback = matches!(
                parsed.host_str(),
                Some("localhost") | Some("127.0.0.1") | Some("[::1]")
            );
            if loopback {
                Ok(())
            } else {
                Err(OauthError::invalid_redirect_uri(
                    "http redirect URIs are only allowed on localhost",
                ))
            }
        }
        "javascript" | "data" | "file" | "blob" | "vbscript" => Err(
            OauthError::invalid_redirect_uri("redirect_uri scheme is not allowed"),
        ),
        _ => Ok(()),
    }
}

fn sanitize_client_name(name: Option<&str>) -> String {
    let cleaned: String = name
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(MAX_CLIENT_NAME_LEN)
        .collect();
    if cleaned.is_empty() {
        DEFAULT_CLIENT_NAME.to_string()
    } else {
        cleaned
    }
}

fn generate_client_id() -> String {
    let mut bytes = [0u8; CLIENT_ID_RANDOM_BYTES];
    rand::rng().fill_bytes(&mut bytes);
    format!("{CLIENT_ID_PREFIX}{}", hex::encode(bytes))
}

fn generate_client_secret() -> String {
    let mut bytes = [0u8; CLIENT_SECRET_RANDOM_BYTES];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Open registration must not let anyone fill the table: registrations that
/// never produced a token (or a live code) are dropped after a day.
async fn prune_unused_clients(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    let cutoff = (now - Duration::hours(UNUSED_CLIENT_RETENTION_HOURS)).to_rfc3339();
    sqlx::query(
        "DELETE FROM oauthClients WHERE createdAt < ? \
         AND id NOT IN (SELECT oauthClientId FROM apiTokens WHERE oauthClientId IS NOT NULL) \
         AND id NOT IN (SELECT clientId FROM oauthAuthorizationCodes WHERE expiresAt > ?)",
    )
    .bind(&cutoff)
    .bind(now.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn load_client(
    pool: &SqlitePool,
    client_id: &str,
) -> Result<Option<OauthClient>, sqlx::Error> {
    sqlx::query_as::<_, OauthClient>("SELECT * FROM oauthClients WHERE clientId = ?")
        .bind(client_id)
        .fetch_optional(pool)
        .await
}

impl OauthClient {
    pub fn redirect_uri_list(&self) -> Vec<String> {
        serde_json::from_str(&self.redirect_uris).unwrap_or_default()
    }

    pub fn allows_redirect_uri(&self, uri: &str) -> bool {
        self.redirect_uri_list().iter().any(|u| u == uri)
    }

    pub fn is_public(&self) -> bool {
        self.token_endpoint_auth_method == "none"
    }

    /// Confidential clients must present their secret; public clients have
    /// none and any secret they send is ignored.
    pub fn verify_secret(&self, secret: Option<&str>) -> Result<(), OauthError> {
        if self.is_public() {
            return Ok(());
        }
        let Some(hash) = self.client_secret_hash.as_deref() else {
            return Err(OauthError::invalid_client("client has no secret on file"));
        };
        match secret {
            Some(s) if hash_token(s) == hash => Ok(()),
            _ => Err(OauthError::invalid_client("client authentication failed")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_uri_rules() {
        assert!(validate_redirect_uri("https://claude.ai/api/mcp/auth_callback").is_ok());
        assert!(validate_redirect_uri("http://localhost:3334/callback").is_ok());
        assert!(validate_redirect_uri("http://127.0.0.1:8080/cb").is_ok());
        assert!(validate_redirect_uri("claude://oauth/callback").is_ok());
        assert!(validate_redirect_uri("http://example.com/cb").is_err());
        assert!(validate_redirect_uri("https://example.com/cb#frag").is_err());
        assert!(validate_redirect_uri("javascript:alert(1)").is_err());
        assert!(validate_redirect_uri("not a url").is_err());
        assert!(validate_redirect_uri(" https://example.com/cb").is_err());
    }

    #[test]
    fn client_names_are_trimmed_capped_and_defaulted() {
        assert_eq!(sanitize_client_name(None), DEFAULT_CLIENT_NAME);
        assert_eq!(sanitize_client_name(Some("  \n ")), DEFAULT_CLIENT_NAME);
        assert_eq!(sanitize_client_name(Some(" Claude\u{0}\n ")), "Claude");
        assert_eq!(
            sanitize_client_name(Some(&"x".repeat(100))).chars().count(),
            MAX_CLIENT_NAME_LEN
        );
    }

    #[test]
    fn generated_identifiers_have_the_expected_shape() {
        let id = generate_client_id();
        assert!(id.starts_with(CLIENT_ID_PREFIX));
        assert_eq!(
            id.len(),
            CLIENT_ID_PREFIX.len() + CLIENT_ID_RANDOM_BYTES * 2
        );
        assert_ne!(generate_client_secret(), generate_client_secret());
    }
}
