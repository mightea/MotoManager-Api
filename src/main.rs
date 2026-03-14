use sqlx::sqlite::SqlitePoolOptions;
use tower_http::trace::TraceLayer;

use moto_manager_api::{build_app, build_cors, AppState, config::Config};

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

    // Create data directories
    tokio::fs::create_dir_all(config.images_dir()).await?;
    tokio::fs::create_dir_all(config.documents_dir()).await?;
    tokio::fs::create_dir_all(config.previews_dir()).await?;

    // Connect to database
    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    let state = AppState {
        pool: pool.clone(),
        config: config.clone(),
    };

    // Build CORS layer
    let cors = build_cors(&config.origin);

    let app = build_app(state).layer(cors).layer(TraceLayer::new_for_http());

    let addr = format!("0.0.0.0:{}", config.port);
    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
