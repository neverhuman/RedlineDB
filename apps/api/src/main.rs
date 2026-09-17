//! redline-web server entry point.
//!
//! Parses [`Config`] from CLI/env, initialises tracing, builds the app and
//! serves it with graceful Ctrl-C shutdown.

use anyhow::Context;
use clap::Parser;
use redline_web_server::build_app;
use redline_web_server::config::Config;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse();
    init_tracing();

    let bind = config.bind;
    let app = build_app(config)?;

    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .with_context(|| format!("binding {bind}"))?;
    tracing::info!(%bind, "redline-web listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;
    Ok(())
}

/// Initialise `tracing` with an env-driven filter (defaults to `info`).
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,redline_web_server=info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

/// Resolve when a Ctrl-C signal is received.
async fn shutdown_signal() {
    if tokio::signal::ctrl_c().await.is_err() {
        tracing::error!("failed to install Ctrl-C handler");
        return;
    }
    tracing::info!("shutdown signal received");
}
