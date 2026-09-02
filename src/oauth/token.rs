//! Token endpoint (`POST /oauth/token`) and revocation (`POST /oauth/revoke`).
//!
//! An OAuth grant is one `apiTokens` row with `kind = 'oauth'`: the access
//! token is the row's `mm_…` secret (so `/mcp` authenticates it exactly like
//! a personal token) and the refresh token is a second secret stored next to
//! it. Refreshing rotates both secrets in place, keeping the row — and with
//! it the audit trail and the revocation entry the user sees — stable for
//! the lifetime of the connection.

use std::collections::HashMap;

use axum::{
    extract::{rejection::FormRejection, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    Form, Json,
};
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use super::{
    check_resource, clients::load_client, no_store, parse_scope, OauthError,
    ACCESS_TOKEN_TTL_SECONDS, REFRESH_TOKEN_TTL_DAYS,
};
use crate::{
    auth::api_token::{display_prefix, generate_token, hash_token, MAX_ACTIVE_TOKENS_PER_USER},
    config::Config,
    models::{ApiToken, OauthAuthorizationCode, OauthClient},
};

const REFRESH_TOKEN_PREFIX: &str = "mmr_";

/// Credentials as presented to the token or revocation endpoint: from the
/// form body (`client_secret_post` / public clients) or a Basic header.
struct ClientCredentials {
    client_id: String,
    client_secret: Option<String>,
}

fn client_credentials(
    headers: &HeaderMap,
    form: &HashMap<String, String>,
) -> Result<ClientCredentials, OauthError> {
    if let Some(value) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
    {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(value.trim())
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .ok_or_else(|| OauthError::invalid_client("malformed Basic credentials"))?;
        let (id, secret) = decoded
            .split_once(':')
            .ok_or_else(|| OauthError::invalid_client("malformed Basic credentials"))?;
        return Ok(ClientCredentials {
            client_id: id.to_string(),
            client_secret: Some(secret.to_string()),
        });
    }
    let client_id = form
        .get("client_id")
        .map(String::as_str)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| OauthError::invalid_client("client_id is required"))?
        .to_string();
    Ok(ClientCredentials {
        client_id,
        client_secret: form.get("client_secret").cloned(),
    })
}

async fn authenticate_client(
    pool: &SqlitePool,
    headers: &HeaderMap,
    form: &HashMap<String, String>,
) -> Result<OauthClient, OauthError> {
    let credentials = client_credentials(headers, form)?;
    let client = load_client(pool, &credentials.client_id)
        .await?
        .ok_or_else(|| OauthError::invalid_client("unknown client_id"))?;
    client.verify_secret(credentials.client_secret.as_deref())?;
    Ok(client)
}

fn form_field<'a>(form: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    form.get(key).map(String::as_str).filter(|v| !v.is_empty())
}

fn require<'a>(form: &'a HashMap<String, String>, key: &str) -> Result<&'a str, OauthError> {
    form_field(form, key).ok_or_else(|| OauthError::invalid_request(format!("{key} is required")))
}

/// `POST /oauth/token`
pub async fn token(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    headers: HeaderMap,
    form: Result<Form<HashMap<String, String>>, FormRejection>,
) -> Result<Response, OauthError> {
    let Form(form) = form.map_err(|e| {
        OauthError::invalid_request(format!(
            "request body must be application/x-www-form-urlencoded: {e}"
        ))
    })?;
    let client = authenticate_client(&pool, &headers, &form).await?;

    let issued = match require(&form, "grant_type")? {
        "authorization_code" => exchange_code(&pool, &config, &client, &form).await?,
        "refresh_token" => refresh(&pool, &client, &form).await?,
        other => {
            return Err(OauthError::unsupported_grant_type(format!(
                "grant_type '{other}' is not supported"
            )))
        }
    };

    sqlx::query("UPDATE oauthClients SET lastUsedAt = ? WHERE id = ?")
        .bind(Utc::now().to_rfc3339())
        .bind(client.id)
        .execute(&pool)
        .await?;

    let mut response = Json(json!({
        "access_token": issued.access_token,
        "token_type": "Bearer",
        "expires_in": ACCESS_TOKEN_TTL_SECONDS,
        "refresh_token": issued.refresh_token,
        "scope": issued.scope,
    }))
    .into_response();
    no_store(response.headers_mut());
    Ok(response)
}

struct IssuedTokens {
    access_token: String,
    refresh_token: String,
    scope: String,
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

fn is_past(value: &str) -> bool {
    parse_timestamp(value).is_none_or(|t| t <= Utc::now())
}

fn generate_refresh_token() -> String {
    // Same entropy as an access token; the prefix keeps the two apart in
    // logs and lets the revocation endpoint pick the right lookup.
    format!(
        "{REFRESH_TOKEN_PREFIX}{}",
        generate_token().trim_start_matches(crate::auth::api_token::TOKEN_PREFIX)
    )
}

/// RFC 7636 §4.6: `BASE64URL(SHA256(code_verifier)) == code_challenge`.
pub fn verify_pkce(code_verifier: &str, code_challenge: &str) -> bool {
    let valid_verifier = (43..=128).contains(&code_verifier.len())
        && code_verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'));
    if !valid_verifier {
        return false;
    }
    let digest = Sha256::digest(code_verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest) == code_challenge
}

async fn exchange_code(
    pool: &SqlitePool,
    config: &Config,
    client: &OauthClient,
    form: &HashMap<String, String>,
) -> Result<IssuedTokens, OauthError> {
    let code = require(form, "code")?;
    let code_verifier = require(form, "code_verifier")?;
    let redirect_uri = require(form, "redirect_uri")?;

    let record = sqlx::query_as::<_, OauthAuthorizationCode>(
        "SELECT * FROM oauthAuthorizationCodes WHERE codeHash = ?",
    )
    .bind(hash_token(code))
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| OauthError::invalid_grant("unknown authorization code"))?;

    if record.client_id != client.id {
        return Err(OauthError::invalid_grant(
            "authorization code was issued to a different client",
        ));
    }
    if record.used_at.is_some() {
        // Replay: whoever holds the code now may also hold the tokens it
        // produced. Revoke them (RFC 6749 §4.1.2).
        if let Some(token_id) = record.issued_token_id {
            sqlx::query("UPDATE apiTokens SET revokedAt = ? WHERE id = ? AND revokedAt IS NULL")
                .bind(Utc::now().to_rfc3339())
                .bind(token_id)
                .execute(pool)
                .await?;
            tracing::warn!(
                "Authorization code replayed for client {}; token {} revoked",
                client.client_id,
                token_id
            );
        }
        return Err(OauthError::invalid_grant("authorization code already used"));
    }
    if is_past(&record.expires_at) {
        return Err(OauthError::invalid_grant("authorization code expired"));
    }
    if record.redirect_uri != redirect_uri {
        return Err(OauthError::invalid_grant("redirect_uri does not match"));
    }
    if !verify_pkce(code_verifier, &record.code_challenge) {
        return Err(OauthError::invalid_grant("PKCE verification failed"));
    }
    check_resource(config, form_field(form, "resource"))?;

    let now = Utc::now();
    // Mark the code used first so a concurrent exchange loses.
    let claimed = sqlx::query(
        "UPDATE oauthAuthorizationCodes SET usedAt = ? WHERE id = ? AND usedAt IS NULL",
    )
    .bind(now.to_rfc3339())
    .bind(record.id)
    .execute(pool)
    .await?;
    if claimed.rows_affected() == 0 {
        return Err(OauthError::invalid_grant("authorization code already used"));
    }

    // One live connection per client and user: a re-authorization replaces
    // the previous grant instead of piling up rows in the token list.
    sqlx::query(
        "UPDATE apiTokens SET revokedAt = ? WHERE userId = ? AND oauthClientId = ? \
         AND kind = 'oauth' AND revokedAt IS NULL",
    )
    .bind(now.to_rfc3339())
    .bind(record.user_id)
    .bind(client.id)
    .execute(pool)
    .await?;

    let active: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM apiTokens WHERE userId = ? AND revokedAt IS NULL")
            .bind(record.user_id)
            .fetch_one(pool)
            .await?;
    if active >= MAX_ACTIVE_TOKENS_PER_USER {
        return Err(OauthError::invalid_request(format!(
            "the user already has {MAX_ACTIVE_TOKENS_PER_USER} active API tokens; revoke one first"
        )));
    }

    let access_token = generate_token();
    let refresh_token = generate_refresh_token();
    let token_id = sqlx::query(
        "INSERT INTO apiTokens (userId, name, tokenHash, tokenPrefix, scope, createdAt, expiresAt, \
         kind, oauthClientId, refreshTokenHash, refreshExpiresAt) \
         VALUES (?, ?, ?, ?, ?, ?, ?, 'oauth', ?, ?, ?)",
    )
    .bind(record.user_id)
    .bind(&client.client_name)
    .bind(hash_token(&access_token))
    .bind(display_prefix(&access_token))
    .bind(&record.scope)
    .bind(now.to_rfc3339())
    .bind((now + Duration::seconds(ACCESS_TOKEN_TTL_SECONDS)).to_rfc3339())
    .bind(client.id)
    .bind(hash_token(&refresh_token))
    .bind((now + Duration::days(REFRESH_TOKEN_TTL_DAYS)).to_rfc3339())
    .execute(pool)
    .await?
    .last_insert_rowid();

    sqlx::query("UPDATE oauthAuthorizationCodes SET issuedTokenId = ? WHERE id = ?")
        .bind(token_id)
        .bind(record.id)
        .execute(pool)
        .await?;

    tracing::info!(
        "OAuth token {} issued to client {} ({}) for user {} with scope {}",
        token_id,
        client.client_id,
        client.client_name,
        record.user_id,
        record.scope
    );

    Ok(IssuedTokens {
        access_token,
        refresh_token,
        scope: record.scope,
    })
}

async fn refresh(
    pool: &SqlitePool,
    client: &OauthClient,
    form: &HashMap<String, String>,
) -> Result<IssuedTokens, OauthError> {
    let refresh_token = require(form, "refresh_token")?;
    let presented_hash = hash_token(refresh_token);

    let Some(token) = sqlx::query_as::<_, ApiToken>(
        "SELECT * FROM apiTokens WHERE refreshTokenHash = ? AND kind = 'oauth'",
    )
    .bind(&presented_hash)
    .fetch_optional(pool)
    .await?
    else {
        // A refresh token that was already rotated away is being replayed:
        // treat the grant as compromised (OAuth 2.1 §4.3.1).
        let replayed = sqlx::query(
            "UPDATE apiTokens SET revokedAt = ? WHERE previousRefreshTokenHash = ? \
             AND revokedAt IS NULL",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(&presented_hash)
        .execute(pool)
        .await?;
        if replayed.rows_affected() > 0 {
            tracing::warn!(
                "Rotated refresh token replayed for client {}; grant revoked",
                client.client_id
            );
        }
        return Err(OauthError::invalid_grant("unknown refresh token"));
    };

    if token.revoked_at.is_some() {
        return Err(OauthError::invalid_grant("grant has been revoked"));
    }
    if token.oauth_client_id != Some(client.id) {
        return Err(OauthError::invalid_grant(
            "refresh token was issued to a different client",
        ));
    }
    if token.refresh_expires_at.as_deref().is_none_or(is_past) {
        return Err(OauthError::invalid_grant("refresh token expired"));
    }
    if let Some(requested) = parse_scope(form_field(form, "scope"))? {
        if requested == "write" && token.scope != "write" {
            return Err(OauthError::invalid_scope(
                "cannot upgrade a read grant to write on refresh",
            ));
        }
    }

    let now = Utc::now();
    let access_token = generate_token();
    let new_refresh_token = generate_refresh_token();
    let rotated = sqlx::query(
        "UPDATE apiTokens SET tokenHash = ?, tokenPrefix = ?, expiresAt = ?, \
         refreshTokenHash = ?, refreshExpiresAt = ?, previousRefreshTokenHash = ?, \
         lastUsedAt = ? WHERE id = ? AND refreshTokenHash = ? AND revokedAt IS NULL",
    )
    .bind(hash_token(&access_token))
    .bind(display_prefix(&access_token))
    .bind((now + Duration::seconds(ACCESS_TOKEN_TTL_SECONDS)).to_rfc3339())
    .bind(hash_token(&new_refresh_token))
    .bind((now + Duration::days(REFRESH_TOKEN_TTL_DAYS)).to_rfc3339())
    .bind(&presented_hash)
    .bind(now.to_rfc3339())
    .bind(token.id)
    .bind(&presented_hash)
    .execute(pool)
    .await?;
    if rotated.rows_affected() == 0 {
        // Lost a race with a concurrent refresh of the same token.
        return Err(OauthError::invalid_grant("refresh token already used"));
    }

    Ok(IssuedTokens {
        access_token,
        refresh_token: new_refresh_token,
        scope: token.scope,
    })
}

/// `POST /oauth/revoke` (RFC 7009). Accepts an access or refresh token and
/// ends the whole grant. Unknown tokens still answer 200 — the endpoint
/// must not confirm whether a secret exists.
pub async fn revoke(
    State(pool): State<SqlitePool>,
    headers: HeaderMap,
    form: Result<Form<HashMap<String, String>>, FormRejection>,
) -> Result<Response, OauthError> {
    let Form(form) = form.map_err(|e| {
        OauthError::invalid_request(format!(
            "request body must be application/x-www-form-urlencoded: {e}"
        ))
    })?;
    let client = authenticate_client(&pool, &headers, &form).await?;
    let token = require(&form, "token")?;
    let hash = hash_token(token);

    let result = sqlx::query(
        "UPDATE apiTokens SET revokedAt = ? WHERE kind = 'oauth' AND oauthClientId = ? \
         AND revokedAt IS NULL AND (tokenHash = ? OR refreshTokenHash = ?)",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(client.id)
    .bind(&hash)
    .bind(&hash)
    .execute(&pool)
    .await?;
    if result.rows_affected() > 0 {
        tracing::info!("OAuth grant revoked by client {}", client.client_id);
    }

    let mut response = axum::http::StatusCode::OK.into_response();
    no_store(response.headers_mut());
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_verifier_matches_its_challenge() {
        // RFC 7636 appendix B test vector.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(verify_pkce(verifier, challenge));
        assert!(!verify_pkce(
            "wrong-verifier-wrong-verifier-wrong-verifier-1",
            challenge
        ));
        assert!(!verify_pkce("short", challenge));
    }

    #[test]
    fn basic_credentials_take_precedence_over_the_form() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode("mmc_abc:s3cret")
            )
            .parse()
            .unwrap(),
        );
        let form = HashMap::from([("client_id".to_string(), "other".to_string())]);
        let creds = client_credentials(&headers, &form).unwrap();
        assert_eq!(creds.client_id, "mmc_abc");
        assert_eq!(creds.client_secret.as_deref(), Some("s3cret"));

        let creds = client_credentials(&HeaderMap::new(), &form).unwrap();
        assert_eq!(creds.client_id, "other");
        assert!(creds.client_secret.is_none());
        assert!(client_credentials(&HeaderMap::new(), &HashMap::new()).is_err());
    }

    #[test]
    fn refresh_tokens_are_distinguishable_from_access_tokens() {
        let refresh = generate_refresh_token();
        assert!(refresh.starts_with(REFRESH_TOKEN_PREFIX));
        assert!(!crate::auth::api_token::looks_like_api_token(&refresh));
    }
}
