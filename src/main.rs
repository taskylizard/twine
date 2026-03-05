mod config;

use confique::Config;
use eyre::Context;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};
use twine_git_server::router_with_git_config;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    dotenvy::dotenv().ok();

    let app_config = config::AppConfig::builder()
        .env()
        .file("twine.toml")
        .load()
        .context("failed to load application configuration")?;

    init_tracing(&app_config.log_filter);

    let listener = TcpListener::bind(&app_config.bind_addr)
        .await
        .with_context(|| format!("failed to bind tcp listener at {}", app_config.bind_addr))?;
    info!(bind_addr = %app_config.bind_addr, "listening for twine HTTP traffic");

    axum::serve(listener, router_with_git_config(app_config.git.into()))
        .await
        .context("server exited unexpectedly")?;

    Ok(())
}

fn init_tracing(default_filter: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}
