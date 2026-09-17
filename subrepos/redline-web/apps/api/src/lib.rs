//! redline-web server library.
//!
//! Database observability + SQL console backend for a running `redline-core`
//! (RedlineDB) or any SQLite database. The HTTP surface is defined by
//! `CONTRACT.md`; [`build_app`] constructs the fully wired [`axum::Router`] so
//! both `main` and the integration tests share one code path.

pub mod api;
pub mod config;
pub mod connector;
pub mod metrics;
pub mod model;
pub mod repair;
pub mod static_assets;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use axum::Router;

use crate::api::AppState;
use crate::config::Config;
use crate::connector::{AnyConnector, SqliteConnector, TargetBinConnector};
use crate::metrics::MetricsRegistry;

/// Build the application router from `config`.
///
/// Selects the connector transport (`--target-bin` takes precedence over the
/// SQLite file), creates the metrics registry, and returns the wired router.
pub fn build_app(config: Config) -> anyhow::Result<Router> {
    let timeout = Duration::from_millis(config.query_timeout_ms);

    let connector = match &config.target_bin {
        Some(bin) => {
            AnyConnector::TargetBin(TargetBinConnector::new(bin, config.read_only, timeout))
        }
        None => AnyConnector::Sqlite(
            SqliteConnector::open(&config.db, config.read_only, timeout)
                .with_context(|| format!("opening sqlite database {:?}", config.db))?,
        ),
    };

    let metrics = MetricsRegistry::new(config.slow_ms);
    let state = AppState {
        connector: Arc::new(connector),
        metrics: Arc::new(metrics),
        config: Arc::new(config),
    };

    Ok(api::router(state))
}
