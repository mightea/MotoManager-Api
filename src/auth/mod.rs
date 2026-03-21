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

#[derive(Debug, Clone)]
pub struct AdminUser(pub User);

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    SqlitePool: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = SqlitePool::from_ref(state);

        let token = extract_bearer_token(&parts.headers).ok_or(AppError::Unauthorized)?;

        let user = session::get_user_from_token(&pool, &token).await?;
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
