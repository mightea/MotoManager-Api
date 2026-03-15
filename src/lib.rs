pub mod auth;
pub mod config;
pub mod error;
pub mod handlers;
pub mod models;

use ax_auth_webauthn::{WebAuthnConfig, WebAuthnLayer};
use axum::{
    http::{HeaderValue, Method},
    routing::{get, post, put},
    Json, Router,
};
use serde_json::json;
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Config,
    pub webauthn: std::sync::Arc<ax_auth_webauthn::WebAuthn>,
}

pub fn build_app(state: AppState) -> Router {
    let webauthn_config = WebAuthnConfig {
        rp_name: "Moto Manager".to_string(),
        rp_id: state.config.rp_id.clone(),
        rp_origin: state.config.origin.clone(),
    };

    Router::new()
        .route("/api/health", get(health_check))
        .route("/api/stats", get(handlers::stats::get_stats))
        .route(
            "/api/motorcycles",
            get(handlers::motorcycles::list_motorcycles).post(handlers::motorcycles::create_motorcycle),
        )
        .route(
            "/api/motorcycles/:id",
            get(handlers::motorcycles::get_motorcycle)
                .put(handlers::motorcycles::update_motorcycle)
                .delete(handlers::motorcycles::delete_motorcycle),
        )
        .route(
            "/api/motorcycles/:id/issues",
            post(handlers::issues::create_issue),
        )
        .route(
            "/api/motorcycles/:id/issues/:issue_id",
            put(handlers::issues::update_issue).delete(handlers::issues::delete_issue),
        )
        .route(
            "/api/motorcycles/:id/maintenance",
            post(handlers::maintenance::create_maintenance),
        )
        .route(
            "/api/motorcycles/:id/maintenance/:m_id",
            put(handlers::maintenance::update_maintenance).delete(handlers::maintenance::delete_maintenance),
        )
        .route(
            "/api/motorcycles/:id/previous-owners",
            post(handlers::previous_owners::create_owner),
        )
        .route(
            "/api/motorcycles/:id/previous-owners/:o_id",
            put(handlers::previous_owners::update_owner).delete(handlers::previous_owners::delete_owner),
        )
        .route(
            "/api/motorcycles/:id/torque-specs",
            post(handlers::torque_specs::create_spec),
        )
        .route(
            "/api/motorcycles/:id/torque-specs/:s_id",
            put(handlers::torque_specs::update_spec).delete(handlers::torque_specs::delete_spec),
        )
        .route(
            "/api/locations",
            get(handlers::locations::list_locations).post(handlers::locations::create_location),
        )
        .route(
            "/api/locations/:lid",
            put(handlers::locations::update_location).delete(handlers::locations::delete_location),
        )
        .route(
            "/api/documents",
            get(handlers::documents::list_documents).post(handlers::documents::create_document),
        )
        .route(
            "/api/documents/:doc_id",
            put(handlers::documents::update_document).delete(handlers::documents::delete_document),
        )
        .route(
            "/api/settings",
            get(handlers::settings::get_settings).put(handlers::settings::update_settings),
        )
        .route(
            "/api/settings/authenticators",
            get(handlers::settings::get_authenticators),
        )
        .route(
            "/api/settings/authenticators/:id",
            get(handlers::settings::get_authenticators).delete(handlers::settings::delete_authenticator),
        )
        .route(
            "/api/settings/change-password",
            post(handlers::settings::change_password),
        )
        .route("/api/currencies", get(handlers::settings::get_currencies))
        .route(
            "/api/admin/currencies",
            post(handlers::settings::create_currency),
        )
        .route(
            "/api/admin/currencies/:cid",
            put(handlers::settings::update_currency).delete(handlers::settings::delete_currency),
        )
        .route(
            "/api/admin/regenerate-previews",
            post(handlers::admin::regenerate_previews),
        )
        .route("/api/auth/status", get(handlers::auth::status))
        .route("/api/auth/login", post(handlers::auth::login))
        .route("/api/auth/register", post(handlers::auth::register))
        .route("/api/auth/me", get(handlers::auth::me))
        .route("/api/auth/logout", post(handlers::auth::logout))
        .route("/api/admin/users", get(handlers::auth::list_users))
        .route(
            "/api/admin/users/:uid",
            put(handlers::auth::update_user).delete(handlers::auth::delete_user),
        )
        .route("/images/:filename", get(handlers::files::serve_image))
        .route("/documents/:filename", get(handlers::files::serve_document))
        .route("/previews/:filename", get(handlers::files::serve_preview))
        .layer(WebAuthnLayer::new(state.webauthn.clone(), webauthn_config))
        .with_state(state)
}

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

pub fn build_cors(origin: &str) -> CorsLayer {
    tracing::info!("Building CORS layer with allowed origin: {}", origin);
    CorsLayer::new()
        .allow_origin(origin.parse::<HeaderValue>().unwrap_or(HeaderValue::from_static("http://localhost:5173")))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ])
}
