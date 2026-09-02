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

use crate::{
    auth::{api_token::authenticate_api_token, extract_bearer_token},
    oauth::protected_resource_metadata_url,
    AppState,
};

pub async fn require_api_token(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let pool: &SqlitePool = &state.pool;
    let principal = match extract_bearer_token(request.headers()) {
        Some(token) => authenticate_api_token(pool, &token).await.ok(),
        None => None,
    };

    let Some(principal) = principal else {
        let mut response = (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Unauthorized: a valid API token is required" })),
        )
            .into_response();
        // RFC 9728 §5.1: point clients at the resource metadata so they can
        // discover the authorization server and start the OAuth flow.
        let challenge = format!(
            "Bearer realm=\"MotoManager MCP\", resource_metadata=\"{}\"",
            protected_resource_metadata_url(&state.config)
        );
        if let Ok(value) = HeaderValue::from_str(&challenge) {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, value);
        }
        return response;
    };

    request.extensions_mut().insert(principal);
    next.run(request).await
}
