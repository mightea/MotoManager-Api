use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::trace::TraceLayer;
use url::Url;
use webauthn_rs::WebauthnBuilder;

use moto_manager_api::{build_app, build_cors, config::Config, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "moto_manager_api=debug,tower_http=debug".into()),
        )
        .init();

    let config = Config::from_env()?;

    // reqwest is built with `rustls-no-provider`; register the ring provider
    // once so outbound TLS (part image fetches) has a crypto backend.
    moto_manager_api::install_crypto_provider();

    // Create data directories
    if !config.images_dir().exists() {
        tokio::fs::create_dir_all(config.images_dir()).await?;
    }
    if !config.documents_dir().exists() {
        tokio::fs::create_dir_all(config.documents_dir()).await?;
    }

    // Create cache directories
    if !config.previews_dir().exists() {
        tokio::fs::create_dir_all(config.previews_dir()).await?;
    }
    if !config.resized_images_dir().exists() {
        tokio::fs::create_dir_all(config.resized_images_dir()).await?;
    }

    // Connect to database. WAL is sticky: once enabled it persists in the DB file
    // and creates `-wal`/`-shm` sibling files next to it. A missing database
    // file is created (regardless of whether the URL carries `?mode=rwc`) so a
    // fresh deployment bootstraps itself: migrations run on the empty file and
    // the first registered account becomes the administrator.
    let connect_options = SqliteConnectOptions::from_str(&config.database_url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(connect_options)
        .await?;

    // Run migrations
    let migrator = sqlx::migrate!("./migrations");
    let latest_embedded = migrator
        .iter()
        .filter(|m| !m.migration_type.is_down_migration())
        .map(|m| m.version)
        .max()
        .ok_or_else(|| anyhow::anyhow!("no migrations bundled"))?;
    tracing::info!(
        "Applying database migrations (target version: {})",
        latest_embedded
    );
    migrator.run(&pool).await?;

    let latest_applied: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await?;
    if latest_applied != latest_embedded {
        anyhow::bail!(
            "migration version mismatch: embedded latest is {latest_embedded}, but database is at {latest_applied}"
        );
    }
    tracing::info!("Migrations up to date (version {})", latest_applied);

    // A backup row left `running` means a previous process died mid-backup;
    // clear it so the admin monitor doesn't show a phantom in-progress run.
    moto_manager_api::backup::reset_stale_running(&pool).await?;

    // Initialize WebAuthn
    let rp_id = &config.rp_id;
    let rp_origin = Url::parse(&config.origin)?;
    let builder = WebauthnBuilder::new(rp_id, &rp_origin)?;
    let webauthn = Arc::new(builder.build()?);

    let backup_lock: moto_manager_api::backup::BackupGuard = Arc::new(tokio::sync::Mutex::new(()));

    // Start the automatic backup scheduler (gated by BACKUP_ENABLED). Manual
    // "Back up now" from the admin UI works regardless.
    if config.backup_enabled {
        moto_manager_api::backup::spawn_scheduler(
            pool.clone(),
            config.clone(),
            backup_lock.clone(),
        );
    } else {
        tracing::info!("Automatic backups disabled (BACKUP_ENABLED=false)");
    }

    let state = AppState {
        pool: pool.clone(),
        config: config.clone(),
        webauthn,
        backup_lock,
    };

    // Build CORS layer
    let cors = build_cors(&config.origin);

    let app = build_app(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let addr = format!("0.0.0.0:{}", config.port);
    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    // `into_make_service_with_connect_info` exposes the peer socket address so the
    // rate limiter's IP key extractor has a fallback when no forwarded header is set.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
