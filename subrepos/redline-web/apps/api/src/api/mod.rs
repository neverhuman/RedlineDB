//! HTTP API: router, shared state and error plumbing.
//!
//! [`router`] wires every endpoint from `CONTRACT.md` onto an [`axum::Router`],
//! mounts the embedded SPA on a catch-all route, and applies permissive CORS,
//! request tracing and gzip compression. All handlers share [`AppState`] (the
//! active connector, the metrics registry and the runtime config) via axum
//! state.

pub mod health;
pub mod metrics;
pub mod query;
pub mod schema;

use std::sync::Arc;

use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::connector::{AnyConnector, ConnectorError};
use crate::metrics::MetricsRegistry;
use crate::model::ApiError;
use crate::static_assets;

/// Shared, cheaply cloneable application state.
#[derive(Clone)]
pub struct AppState {
    /// The active database connector.
    pub connector: Arc<AnyConnector>,
    /// Process-wide metrics registry.
    pub metrics: Arc<MetricsRegistry>,
    /// Runtime configuration.
    pub config: Arc<Config>,
}

/// An error response: an HTTP status paired with the JSON [`ApiError`] envelope.
pub type ApiErr = (StatusCode, Json<ApiError>);

/// Result alias for JSON handlers.
pub type ApiResult<T> = Result<Json<T>, ApiErr>;

/// Map a [`ConnectorError`] to an HTTP error response.
///
/// The typed [`crate::repair::RepairHint`] is logged so a failure is locally
/// debuggable (purpose, reason, common fixes, docs_url, repair_hint) instead of
/// opaque; the client still receives the compact [`ApiError`] envelope.
pub fn err_response(err: ConnectorError) -> ApiErr {
    let status =
        StatusCode::from_u16(err.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let hint = err.repair();
    tracing::warn!(
        purpose = hint.purpose,
        reason = %hint.reason,
        docs_url = hint.docs_url,
        repair_hint = hint.repair_hint,
        "request failed at a database boundary"
    );
    (status, Json(ApiError::new(err.to_string())))
}

/// Build the full application router for `state`.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health::health))
        .route("/api/connection", get(health::connection))
        .route("/api/schema", get(schema::schema))
        .route("/api/tables/{name}/schema", get(schema::table_schema))
        .route("/api/tables/{name}", get(schema::table_page))
        .route("/api/query", post(query::query))
        .route("/api/metrics", get(metrics::metrics))
        .route("/api/metrics/stream", get(metrics::metrics_stream))
        .route("/api/slow-queries", get(metrics::slow_queries))
        .route("/metrics", get(metrics::prometheus))
        // Serve the embedded SPA on the root and every unmatched GET path so
        // client-side routing resolves to `index.html`.
        .route("/", get(static_assets::static_handler))
        .route("/{*spa_path}", get(static_assets::static_handler))
        .with_state(state)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}
