//! Model Context Protocol server, mounted at `/mcp` (Streamable HTTP).
//!
//! Security model, in one place:
//! - **Authentication** is a personal API token (`auth::api_token`), checked by
//!   [`auth::require_api_token`] before any request reaches the MCP layer.
//!   Sessions are not accepted here and API tokens are not accepted on `/api`.
//! - **Authorization** is user-level only. Tools receive an [`McpPrincipal`]
//!   and call the same handlers the apps use, with the token's user — so every
//!   ownership check (`verify_motorcycle_ownership`, `verify_part_ownership`,
//!   `userId = ?` filters) applies unchanged. No tool consults the user's
//!   role and no admin handler is reachable, so an administrator's token is
//!   exactly as powerful as anyone else's.
//! - **Scope**: read tokens may only call tools annotated `read_only_hint`;
//!   the check is central (`call_tool`) and defaults to "write" for any tool
//!   without an explicit read-only annotation.
//! - **Writes are additive**: create/log tools plus one status change; nothing
//!   deletes. Inputs are validated (`validate`) before a handler is called and
//!   unknown fields are rejected.
//! - **Audit**: every tool call is written to `mcpAuditLog` with its outcome.

pub mod auth;
mod server;
mod validate;

pub use server::McpServer;

use rmcp::transport::streamable_http_server::{
    session::never::NeverSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use std::sync::Arc;

use crate::AppState;

/// Stateless service: each POST is one JSON-RPC exchange, no server-side
/// session table to leak, and JSON responses unless a tool streams.
pub fn service(state: &AppState) -> StreamableHttpService<McpServer, NeverSessionManager> {
    let mut config = StreamableHttpServerConfig::default();
    config.legacy_session_mode = false;
    config.json_response = true;
    config.allowed_hosts = state.config.mcp_allowed_hosts.clone();

    let state = state.clone();
    StreamableHttpService::new(
        move || Ok(McpServer::new(state.pool.clone(), state.config.clone())),
        Arc::new(NeverSessionManager::default()),
        config,
    )
}
