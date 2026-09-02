//! Personal API tokens for the MCP endpoint (`/mcp`).
//!
//! Deliberately separate from browser/app sessions: a session token is never
//! accepted on `/mcp`, and an API token is never accepted on `/api/*`. That
//! keeps the AI surface opt-in per token and scoped — a token identifies one
//! user and one scope, and it never carries the user's admin role.

use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::{
    error::{AppError, AppResult},
    models::{ApiToken, User},
};

/// Secrets look like `mm_<48 hex chars>`; the prefix makes a leaked token
/// recognisable in logs and lets the middleware reject other credentials early.
pub const TOKEN_PREFIX: &str = "mm_";
const TOKEN_RANDOM_BYTES: usize = 24;
const TOKEN_LEN: usize = TOKEN_PREFIX.len() + TOKEN_RANDOM_BYTES * 2;
/// Characters of the secret shown in the UI so a user can tell tokens apart.
const DISPLAY_PREFIX_LEN: usize = TOKEN_PREFIX.len() + 8;

pub const MAX_ACTIVE_TOKENS_PER_USER: i64 = 20;
pub const MAX_TOKEN_NAME_LEN: usize = 64;
pub const MAX_TOKEN_LIFETIME_DAYS: i64 = 365;
/// `lastUsedAt` is only rewritten when the stored value is older than this,
/// so a chatty client does not turn every tool call into a write.
const LAST_USED_REFRESH_MINUTES: i64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenScope {
    /// Read tools only.
    Read,
    /// Read tools plus the additive write tools. Never deletes, never admin.
    Write,
}

impl TokenScope {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "read" => Some(Self::Read),
            "write" => Some(Self::Write),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }

    pub fn allows_write(self) -> bool {
        matches!(self, Self::Write)
    }
}

/// The identity an MCP request acts as. Built from a valid API token by the
/// `/mcp` middleware and handed to every tool. Note what is *not* here: no
/// admin flag, no session — tools only ever see a plain user.
#[derive(Debug, Clone)]
pub struct McpPrincipal {
    pub user: User,
    pub token_id: i64,
    pub token_name: String,
    pub scope: TokenScope,
}

pub fn generate_token() -> String {
    let mut bytes = [0u8; TOKEN_RANDOM_BYTES];
    rand::rng().fill_bytes(&mut bytes);
    format!("{TOKEN_PREFIX}{}", hex::encode(bytes))
}

pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub fn display_prefix(token: &str) -> String {
    token.chars().take(DISPLAY_PREFIX_LEN).collect()
}

/// Cheap shape check before touching the database: rejects session tokens and
/// garbage without a query.
pub fn looks_like_api_token(token: &str) -> bool {
    token.len() == TOKEN_LEN
        && token.starts_with(TOKEN_PREFIX)
        && token[TOKEN_PREFIX.len()..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
}

/// Resolve a bearer secret to its principal. Revoked, expired, and unknown
/// tokens all yield `Unauthorized` — the caller cannot distinguish them.
pub async fn authenticate_api_token(pool: &SqlitePool, token: &str) -> AppResult<McpPrincipal> {
    if !looks_like_api_token(token) {
        return Err(AppError::Unauthorized);
    }
    let hash = hash_token(token);

    let api_token = sqlx::query_as::<_, ApiToken>(
        "SELECT * FROM apiTokens WHERE tokenHash = ? AND revokedAt IS NULL",
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    let now = Utc::now();
    if let Some(expires_at) = api_token.expires_at.as_deref() {
        let expired = DateTime::parse_from_rfc3339(expires_at)
            .map(|t| t.with_timezone(&Utc) <= now)
            .unwrap_or(true);
        if expired {
            return Err(AppError::Unauthorized);
        }
    }

    let scope = TokenScope::parse(&api_token.scope).ok_or(AppError::Unauthorized)?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(api_token.user_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let stale = api_token
        .last_used_at
        .as_deref()
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|t| now - t.with_timezone(&Utc) > Duration::minutes(LAST_USED_REFRESH_MINUTES))
        .unwrap_or(true);
    if stale {
        let now_str = now.to_rfc3339();
        if let Err(e) = sqlx::query("UPDATE apiTokens SET lastUsedAt = ? WHERE id = ?")
            .bind(&now_str)
            .bind(api_token.id)
            .execute(pool)
            .await
        {
            tracing::warn!(
                "Failed to record API token use for token {}: {}",
                api_token.id,
                e
            );
        }
    }

    Ok(McpPrincipal {
        user,
        token_id: api_token.id,
        token_name: api_token.name,
        scope,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_have_the_expected_shape() {
        let token = generate_token();
        assert!(looks_like_api_token(&token));
        assert_eq!(display_prefix(&token).len(), DISPLAY_PREFIX_LEN);
        assert_ne!(generate_token(), token);
    }

    #[test]
    fn session_like_values_are_not_api_tokens() {
        assert!(!looks_like_api_token(&"a".repeat(80)));
        assert!(!looks_like_api_token("mm_short"));
        assert!(!looks_like_api_token(&format!("mm_{}", "z".repeat(48))));
    }

    #[test]
    fn hashing_is_stable_and_hides_the_secret() {
        let token = generate_token();
        assert_eq!(hash_token(&token), hash_token(&token));
        assert!(!hash_token(&token).contains(&token[3..]));
    }
}
