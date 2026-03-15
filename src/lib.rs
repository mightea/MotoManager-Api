pub mod auth;
pub mod config;
pub mod error;
pub mod handlers;
pub mod models;

use axum::{
    http::{HeaderValue, Method},
    routing::{get, post, put},
    Json, Router,
};
use serde_json::json;
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;

use config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Config,
}

impl axum::extract::FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl axum::extract::FromRef<AppState> for Config {
    fn from_ref(state: &AppState) -> Self {
        state.config.clone()
    }
}

pub fn build_app(state: AppState) -> Router {
    let app_version = state.config.app_version.clone();

    Router::new()
        // Health check
        .route(
            "/api/health",
            get(move || async move {
                Json(json!({ "status": "ok", "version": app_version }))
            }),
        )
        // Auth routes
        .route("/api/auth/status", get(handlers::auth::status))
        .route("/api/auth/login", post(handlers::auth::login))
        .route("/api/auth/logout", post(handlers::auth::logout))
        .route("/api/auth/register", post(handlers::auth::register))
        .route("/api/auth/me", get(handlers::auth::me))
        // Motorcycle routes
        .route(
            "/api/motorcycles",
            get(handlers::motorcycles::list_motorcycles)
                .post(handlers::motorcycles::create_motorcycle),
        )
        .route(
            "/api/motorcycles/:id",
            get(handlers::motorcycles::get_motorcycle)
                .put(handlers::motorcycles::update_motorcycle)
                .delete(handlers::motorcycles::delete_motorcycle),
        )
        // Issues routes
        .route(
            "/api/motorcycles/:id/issues",
            get(handlers::issues::list_issues).post(handlers::issues::create_issue),
        )
        .route(
            "/api/motorcycles/:id/issues/:issue_id",
            put(handlers::issues::update_issue).delete(handlers::issues::delete_issue),
        )
        // Maintenance routes
        .route(
            "/api/motorcycles/:id/maintenance",
            get(handlers::maintenance::list_maintenance)
                .post(handlers::maintenance::create_maintenance),
        )
        .route(
            "/api/motorcycles/:id/maintenance/:mid",
            put(handlers::maintenance::update_maintenance)
                .delete(handlers::maintenance::delete_maintenance),
        )
        // Previous owners routes
        .route(
            "/api/motorcycles/:id/previous-owners",
            get(handlers::previous_owners::list_previous_owners)
                .post(handlers::previous_owners::create_previous_owner),
        )
        .route(
            "/api/motorcycles/:id/previous-owners/:oid",
            put(handlers::previous_owners::update_previous_owner)
                .delete(handlers::previous_owners::delete_previous_owner),
        )
        // Torque specs routes
        .route(
            "/api/motorcycles/:id/torque-specs",
            get(handlers::torque_specs::list_torque_specs)
                .post(handlers::torque_specs::create_torque_spec),
        )
        .route(
            "/api/motorcycles/:id/torque-specs/import",
            post(handlers::torque_specs::import_torque_specs),
        )
        .route(
            "/api/motorcycles/:id/torque-specs/:tid",
            put(handlers::torque_specs::update_torque_spec)
                .delete(handlers::torque_specs::delete_torque_spec),
        )
        // Documents routes
        .route(
            "/api/documents",
            get(handlers::documents::list_documents).post(handlers::documents::create_document),
        )
        .route(
            "/api/documents/:doc_id",
            put(handlers::documents::update_document).delete(handlers::documents::delete_document),
        )
        // Locations routes
        .route(
            "/api/locations",
            get(handlers::locations::list_locations).post(handlers::locations::create_location),
        )
        .route(
            "/api/locations/:lid",
            put(handlers::locations::update_location).delete(handlers::locations::delete_location),
        )
        // Settings routes
        .route(
            "/api/settings",
            get(handlers::settings::get_settings).put(handlers::settings::update_settings),
        )
        .route(
            "/api/settings/change-password",
            post(handlers::settings::change_password),
        )
        // Admin routes
        .route(
            "/api/admin/users",
            get(handlers::admin::list_users).post(handlers::admin::create_user),
        )
        .route(
            "/api/admin/users/:uid",
            put(handlers::admin::update_user).delete(handlers::admin::delete_user),
        )
        .route(
            "/api/admin/currencies",
            get(handlers::admin::list_currencies).post(handlers::admin::create_currency),
        )
        .route(
            "/api/admin/currencies/:cid",
            put(handlers::admin::update_currency).delete(handlers::admin::delete_currency),
        )
        // Public currencies
        .route("/api/currencies", get(handlers::admin::list_currencies_public))
        // Stats
        .route("/api/stats", get(handlers::stats::get_stats))
        // File serving
        .route("/images/:filename", get(handlers::files::serve_image))
        .route(
            "/documents/:filename",
            get(handlers::files::serve_document),
        )
        .route(
            "/previews/:filename",
            get(handlers::files::serve_preview),
        )
        .with_state(state)
}

pub fn build_cors(origin: &str) -> CorsLayer {
    tracing::info!("Building CORS layer with allowed origin: {}", origin);
    let allowed_origin = origin
        .parse::<HeaderValue>()
        .unwrap_or_else(|_| HeaderValue::from_static("*"));

    CorsLayer::new()
        .allow_origin(allowed_origin)
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
