//! Metrics handlers: JSON snapshot, SSE stream, slow queries and Prometheus.

use std::convert::Infallible;
use std::time::Duration;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures::Stream;
use futures::StreamExt;
use serde::Deserialize;
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;

use super::AppState;
use crate::connector::Connector;
use crate::model::{MetricsSnapshot, SlowQueriesResponse};

/// Cadence of the metrics SSE stream.
const STREAM_PERIOD: Duration = Duration::from_secs(2);

/// Build a current snapshot, degrading gracefully when storage stats are
/// unavailable (e.g. the target-bin transport).
async fn snapshot(state: &AppState) -> MetricsSnapshot {
    let db = state.connector.db_stats().await.unwrap_or_default();
    let tables = state.connector.table_sizes().await.unwrap_or_default();
    state.metrics.snapshot(db, tables)
}

/// `GET /api/metrics` — point-in-time metrics snapshot.
pub async fn metrics(State(state): State<AppState>) -> Json<MetricsSnapshot> {
    Json(snapshot(&state).await)
}

/// `GET /api/metrics/stream` — Server-Sent Events of [`MetricsSnapshot`].
pub async fn metrics_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = IntervalStream::new(interval(STREAM_PERIOD)).then(move |_| {
        let state = state.clone();
        async move {
            let snap = snapshot(&state).await;
            let event = Event::default()
                .json_data(snap)
                .unwrap_or_else(|_| Event::default().comment("snapshot serialization failed"));
            Ok(event)
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Query parameters for the slow-query endpoint.
#[derive(Debug, Deserialize)]
pub struct SlowParams {
    pub limit: Option<usize>,
}

/// `GET /api/slow-queries?limit` — recent slow queries, newest first.
pub async fn slow_queries(
    State(state): State<AppState>,
    Query(params): Query<SlowParams>,
) -> Json<SlowQueriesResponse> {
    let limit = params.limit.unwrap_or(50);
    Json(SlowQueriesResponse {
        queries: state.metrics.slow_queries(limit),
    })
}

/// `GET /metrics` — Prometheus text exposition.
pub async fn prometheus(State(state): State<AppState>) -> Response {
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        state.metrics.prometheus_text(),
    )
        .into_response()
}
