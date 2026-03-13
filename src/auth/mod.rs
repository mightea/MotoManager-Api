pub mod password;
pub mod session;

use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts, Request},
    http::request::Parts,
    middleware::Next,
    response::Response,
};
use axum_extra::extract::CookieJar;
use sqlx::SqlitePool;

use crate::{error::AppError, models::User};

pub const SESSION_COOKIE_NAME: &str = "mb_session";
pub const SESSION_DURATION_DAYS: i64 = 14;

#[derive(Debug, Clone)]
pub struct AuthUser(pub User);

#[derive(Debug, Clone)]
pub struct AdminUser(pub User);

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    SqlitePool: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = SqlitePool::from_ref(state);
        let jar = CookieJar::from_headers(&parts.headers);

        let token = jar
            .get(SESSION_COOKIE_NAME)
            .map(|c| c.value().to_string())
            .ok_or(AppError::Unauthorized)?;

        let user = session::get_user_from_token(&pool, &token).await?;
        Ok(AuthUser(user))
    }
}

#[async_trait]
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

pub async fn session_refresh_middleware(
    pool: axum::extract::State<SqlitePool>,
    jar: CookieJar,
    request: Request,
    next: Next,
) -> Response {
    if let Some(token_cookie) = jar.get(SESSION_COOKIE_NAME) {
        let token = token_cookie.value().to_string();
        let _ = session::refresh_session(&pool, &token).await;
    }
    next.run(request).await
}
