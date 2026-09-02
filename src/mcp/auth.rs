//! Bearer-token gate for `/mcp`. Runs as an axum middleware in front of the
//! MCP service, so unauthenticated requests never reach the protocol layer.

use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use sqlx::SqlitePool;

use crate::auth::{api_token::authenticate_api_token, extract_bearer_token};

pub async fn require_api_token(
    State(pool): State<SqlitePool>,
    mut request: Request,
    next: Next,
) -> Response {
    let principal = match extract_bearer_token(request.headers()) {
        Some(token) => authenticate_api_token(&pool, &token).await.ok(),
        None => None,
    };

    let Some(principal) = principal else {
        let mut response = (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Unauthorized: a valid API token is required" })),
        )
            .into_response();
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer realm=\"MotoManager MCP\""),
        );
        return response;
    };

    request.extensions_mut().insert(principal);
    next.run(request).await
}
