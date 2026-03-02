use eyre::Context;
use twine_git_server::router;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    init_tracing();

    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .context("failed to bind tcp listener")?;
    info!("listening on http://0.0.0.0:3000");

    axum::serve(listener, router())
        .await
        .context("server exited unexpectedly")?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}
