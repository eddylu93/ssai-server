use anyhow::Context;
use sqlx::migrate;
use ssai_server::{
    auth::bootstrap::bootstrap_admin_user, config::Config, scheduler, state::AppState,
};
use tokio::{net::TcpListener, signal};
use tower_http::{
    compression::CompressionLayer,
    timeout::TimeoutLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::Level;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let config = Config::from_env()?;
    init_tracing(&config)?;

    let state = AppState::from_config(config.clone())
        .await
        .context("failed to initialize app state")?;

    migrate!("./migrations")
        .run(&state.pool)
        .await
        .context("failed to run migrations")?;

    bootstrap_admin_user(&state).await?;
    let mut scheduler = scheduler::spawn_all(&state).await?;

    let app = ssai_server::api::router(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::new(std::time::Duration::from_secs(30)));

    let listener = TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("failed to bind {}", config.bind_addr))?;
    let local_addr = listener.local_addr().context("failed to read local addr")?;

    tracing::info!(bind_addr = %local_addr, version = %config.version, "ssai-server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("axum server exited unexpectedly")?;

    scheduler
        .shutdown()
        .await
        .context("failed to shutdown scheduler")?;

    Ok(())
}

fn init_tracing(config: &Config) -> anyhow::Result<()> {
    let env_filter =
        tracing_subscriber::EnvFilter::try_new(&config.rust_log).context("invalid RUST_LOG")?;

    match config.log_format.as_str() {
        "json" => tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .json()
            .with_current_span(false)
            .with_span_list(false)
            .init(),
        "pretty" => tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .pretty()
            .init(),
        other => anyhow::bail!("unsupported LOG_FORMAT: {other}"),
    }

    Ok(())
}

async fn shutdown_signal() {
    let _ = signal::ctrl_c().await;
}
