pub mod password;
pub mod session;

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts, HeaderMap},
};
use sqlx::SqlitePool;

use crate::{error::AppError, models::User};

pub const SESSION_DURATION_DAYS: i64 = 14;

pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    value.strip_prefix("Bearer ").map(ToString::to_string)
}

#[derive(Debug, Clone)]
pub struct AuthUser(pub User);

/// Record the client app version reported via the `X-App-Version` /
/// `X-App-Build` headers. Piggybacks on the user row the extractor already
/// loaded: compares in memory and writes only when the value actually changed
/// (roughly once per app update per user). Clients that send no headers
/// (webapp, older app builds) never touch the stored values, and a failed
/// write must not fail the request.
async fn record_app_version(pool: &SqlitePool, user: &mut User, headers: &HeaderMap) {
    let Some(version) = headers
        .get("x-app-version")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty() && v.len() <= 32)
    else {
        return;
    };
    let Some(build) = headers
        .get("x-app-build")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|b| *b >= 0)
    else {
        return;
    };

    if user.app_version.as_deref() == Some(version) && user.app_build == Some(build) {
        return;
    }

    let result = sqlx::query("UPDATE users SET appVersion = ?, appBuild = ? WHERE id = ?")
        .bind(version)
        .bind(build)
        .bind(user.id)
        .execute(pool)
        .await;
    match result {
        Ok(_) => {
            user.app_version = Some(version.to_string());
            user.app_build = Some(build);
        }
        Err(e) => tracing::warn!("Failed to record app version for user {}: {}", user.id, e),
    }
}

#[derive(Debug, Clone)]
pub struct AdminUser(pub User);

/// Auth for the browser-navigated file routes (/documents, /previews) only:
/// `window.open` and `<img src>` cannot set an Authorization header, so these
/// routes also accept the session token as a `?token=` query parameter. Keep
/// every API route on `AuthUser` — query tokens end up in access logs and
/// browser history, which is tolerable only for these direct file links.
#[derive(Debug, Clone)]
pub struct FileAuthUser(pub User);

fn token_from_query(query: Option<&str>) -> Option<String> {
    // Session tokens are plain hex (see session::generate_session_token), so a
    // simple split is enough — no percent-decoding needed.
    query?
        .split('&')
        .find_map(|pair| pair.strip_prefix("token="))
        .filter(|t| !t.is_empty())
        .map(ToString::to_string)
}

impl<S> FromRequestParts<S> for FileAuthUser
where
    S: Send + Sync,
    SqlitePool: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = SqlitePool::from_ref(state);

        let token = extract_bearer_token(&parts.headers)
            .or_else(|| token_from_query(parts.uri.query()))
            .ok_or(AppError::Unauthorized)?;

        let user = session::get_user_from_token(&pool, &token).await?;
        Ok(FileAuthUser(user))
    }
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    SqlitePool: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = SqlitePool::from_ref(state);

        let token = extract_bearer_token(&parts.headers).ok_or(AppError::Unauthorized)?;

        let mut user = session::get_user_from_token(&pool, &token).await?;
        record_app_version(&pool, &mut user, &parts.headers).await;
        Ok(AuthUser(user))
    }
}

impl<S> FromRequestParts<S> for AdminUser
where
    S: Send + Sync,
    SqlitePool: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;
        if auth_user.0.role != "admin" {
            return Err(AppError::Forbidden);
        }
        Ok(AdminUser(auth_user.0))
    }
}
