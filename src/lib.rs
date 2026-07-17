pub mod auth;
pub mod backup;
pub mod config;
pub mod error;
pub mod handlers;
pub mod models;
pub mod pdfium_lib;

use axum::{
    extract::DefaultBodyLimit,
    http::{HeaderValue, Method},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde_json::json;
use sqlx::SqlitePool;
use std::sync::Arc;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer,
};
use tower_http::cors::CorsLayer;
use webauthn_rs::Webauthn;

use config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Config,
    pub webauthn: Arc<Webauthn>,
    /// Guards backup runs so scheduled and manual backups never overlap.
    pub backup_lock: backup::BackupGuard,
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

impl axum::extract::FromRef<AppState> for Arc<Webauthn> {
    fn from_ref(state: &AppState) -> Self {
        state.webauthn.clone()
    }
}

impl axum::extract::FromRef<AppState> for backup::BackupGuard {
    fn from_ref(state: &AppState) -> Self {
        state.backup_lock.clone()
    }
}

/// Body-size cap for the multipart upload routes (motorcycle images, documents).
/// Axum's default is 2 MB, which silently 413s ordinary phone photos; everything
/// else keeps the small default so JSON endpoints can't be used to buffer megabytes.
const UPLOAD_BODY_LIMIT: usize = 30 * 1024 * 1024;

pub fn build_app(state: AppState) -> Router {
    let app_version = state.config.app_version.clone();

    let router = Router::new()
        .route(
            "/api/health",
            get(move || async move { Json(json!({ "status": "ok", "version": app_version })) }),
        )
        .route("/api/auth/status", get(handlers::auth::status))
        .route("/api/auth/logout", post(handlers::auth::logout))
        .route("/api/auth/me", get(handlers::auth::me))
        .route(
            "/api/auth/passkey/register-options",
            get(handlers::passkey::register_options),
        )
        .route(
            "/api/auth/passkey/login-options",
            get(handlers::passkey::login_options),
        )
        .route(
            "/api/motorcycles",
            get(handlers::motorcycles::list_motorcycles)
                .post(handlers::motorcycles::create_motorcycle)
                .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)),
        )
        .route(
            "/api/motorcycles/{id}",
            get(handlers::motorcycles::get_motorcycle)
                .put(handlers::motorcycles::update_motorcycle)
                .delete(handlers::motorcycles::delete_motorcycle)
                .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)),
        )
        .route(
            "/api/motorcycles/{id}/issues",
            get(handlers::issues::list_issues).post(handlers::issues::create_issue),
        )
        .route(
            "/api/motorcycles/{id}/issues/{issue_id}",
            put(handlers::issues::update_issue).delete(handlers::issues::delete_issue),
        )
        .route(
            "/api/motorcycles/{id}/maintenance",
            get(handlers::maintenance::list_maintenance)
                .post(handlers::maintenance::create_maintenance),
        )
        .route(
            "/api/motorcycles/{id}/maintenance/{mid}",
            put(handlers::maintenance::update_maintenance)
                .delete(handlers::maintenance::delete_maintenance),
        )
        .route(
            "/api/motorcycles/{id}/previous-owners",
            get(handlers::previous_owners::list_previous_owners)
                .post(handlers::previous_owners::create_previous_owner),
        )
        .route(
            "/api/motorcycles/{id}/previous-owners/{oid}",
            put(handlers::previous_owners::update_previous_owner)
                .delete(handlers::previous_owners::delete_previous_owner),
        )
        .route(
            "/api/motorcycles/{id}/torque-specs",
            get(handlers::torque_specs::list_torque_specs)
                .post(handlers::torque_specs::create_torque_spec),
        )
        .route(
            "/api/motorcycles/{id}/torque-specs/import",
            post(handlers::torque_specs::import_torque_specs),
        )
        .route(
            "/api/motorcycles/{id}/torque-specs/{tid}",
            put(handlers::torque_specs::update_torque_spec)
                .delete(handlers::torque_specs::delete_torque_spec),
        )
        .route(
            "/api/motorcycles/{id}/details",
            get(handlers::motorcycle_details::list_details)
                .post(handlers::motorcycle_details::create_detail),
        )
        .route(
            "/api/motorcycles/{id}/details/{did}",
            put(handlers::motorcycle_details::update_detail)
                .delete(handlers::motorcycle_details::delete_detail),
        )
        .route(
            "/api/motorcycles/{id}/tire-pressure",
            get(handlers::tire_pressure::get_tire_pressure)
                .put(handlers::tire_pressure::upsert_tire_pressure)
                .delete(handlers::tire_pressure::delete_tire_pressure),
        )
        .route(
            "/api/documents",
            get(handlers::documents::list_documents)
                .post(handlers::documents::create_document)
                .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)),
        )
        .route(
            "/api/documents/{doc_id}",
            put(handlers::documents::update_document)
                .delete(handlers::documents::delete_document)
                .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)),
        )
        .route(
            "/api/locations",
            get(handlers::locations::list_locations).post(handlers::locations::create_location),
        )
        .route(
            "/api/locations/merge",
            post(handlers::locations::merge_locations),
        )
        .route(
            "/api/locations/{lid}",
            put(handlers::locations::update_location).delete(handlers::locations::delete_location),
        )
        .route(
            "/api/expenses",
            get(handlers::expenses::list_expenses).post(handlers::expenses::create_expense),
        )
        .route(
            "/api/expenses/{id}",
            put(handlers::expenses::update_expense).delete(handlers::expenses::delete_expense),
        )
        .route(
            "/api/model-series",
            get(handlers::model_series::list_model_series)
                .post(handlers::model_series::create_model_series),
        )
        .route(
            "/api/model-series/{sid}",
            put(handlers::model_series::update_model_series)
                .delete(handlers::model_series::delete_model_series),
        )
        .route("/api/vin/decode", get(handlers::model_series::decode_vin))
        .route(
            "/api/storage-locations",
            get(handlers::storage_locations::list_storage_locations)
                .post(handlers::storage_locations::create_storage_location),
        )
        .route(
            "/api/storage-locations/{id}",
            put(handlers::storage_locations::update_storage_location)
                .delete(handlers::storage_locations::delete_storage_location),
        )
        .route(
            "/api/parts",
            get(handlers::parts::list_parts).post(handlers::parts::create_part),
        )
        .route("/api/parts/public", get(handlers::parts::list_public_parts))
        .route(
            "/api/parts/{id}",
            put(handlers::parts::update_part).delete(handlers::parts::delete_part),
        )
        .route(
            "/api/parts/{id}/image",
            post(handlers::parts::upload_part_image)
                .delete(handlers::parts::delete_part_image)
                .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)),
        )
        .route(
            "/api/parts/{id}/image-from-url",
            post(handlers::parts::import_part_image_from_url),
        )
        .route(
            "/api/part-imports/parse",
            post(handlers::part_import::parse_invoice)
                .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)),
        )
        .route(
            "/api/part-stocks",
            get(handlers::parts::list_part_stocks).post(handlers::parts::create_part_stock),
        )
        .route(
            "/api/part-stocks/{id}",
            put(handlers::parts::update_part_stock).delete(handlers::parts::delete_part_stock),
        )
        .route(
            "/api/part-consumptions",
            get(handlers::parts::list_part_consumptions)
                .post(handlers::parts::create_part_consumption),
        )
        .route(
            "/api/part-consumptions/{id}",
            put(handlers::parts::update_part_consumption)
                .delete(handlers::parts::delete_part_consumption),
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
            "/api/settings/authenticators/{id}",
            delete(handlers::settings::delete_authenticator),
        )
        .route(
            "/api/settings/change-password",
            post(handlers::settings::change_password),
        )
        .route(
            "/api/admin/users",
            get(handlers::admin::list_users).post(handlers::admin::create_user),
        )
        .route(
            "/api/admin/users/{uid}",
            put(handlers::admin::update_user).delete(handlers::admin::delete_user),
        )
        .route(
            "/api/admin/currencies",
            get(handlers::admin::list_currencies).post(handlers::admin::create_currency),
        )
        .route(
            "/api/admin/currencies/{cid}",
            put(handlers::admin::update_currency).delete(handlers::admin::delete_currency),
        )
        .route(
            "/api/admin/regenerate-previews",
            post(handlers::admin::regenerate_previews),
        )
        .route(
            "/api/admin/backups",
            get(handlers::backups::list_backups).post(handlers::backups::create_backup),
        )
        .route(
            "/api/admin/backups/{id}",
            delete(handlers::backups::delete_backup),
        )
        .route(
            "/api/admin/backups/{id}/download",
            get(handlers::backups::download_backup),
        )
        .route(
            "/api/currencies",
            get(handlers::admin::list_currencies_public),
        )
        .route("/api/stats", get(handlers::stats::get_stats))
        .route("/api/home", get(handlers::home::get_home_data))
        .route("/images/{filename}", get(handlers::files::serve_image))
        .route(
            "/documents/{filename}",
            get(handlers::files::serve_document),
        )
        .route("/previews/{filename}", get(handlers::files::serve_preview));

    // Rate-limit the credential-guessing endpoints (login, register, passkey
    // verification). Argon2id already slows each guess; this caps flood attempts
    // per client IP. `SmartIpKeyExtractor` honours `X-Forwarded-For`/`Forwarded`
    // for the reverse-proxy deployment, falling back to the peer IP (which
    // requires `ConnectInfo`, wired up in `main`).
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(2)
            .burst_size(15)
            .key_extractor(SmartIpKeyExtractor)
            .finish()
            .expect("valid governor config"),
    );
    let auth_routes = Router::new()
        .route("/api/auth/login", post(handlers::auth::login))
        .route("/api/auth/register", post(handlers::auth::register))
        .route(
            "/api/auth/passkey/register-verify",
            post(handlers::passkey::register_verify),
        )
        .route(
            "/api/auth/passkey/login-verify",
            post(handlers::passkey::login_verify),
        )
        .layer(GovernorLayer {
            config: governor_conf,
        });

    router.merge(auth_routes).with_state(state)
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
