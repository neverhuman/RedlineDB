//! Ad-hoc SQL query handler.
//!
//! Every execution is timed and recorded into the metrics registry (count,
//! latency, and the slow-query ring when over threshold), per the contract.

use std::time::Instant;

use axum::Json;
use axum::extract::State;

use super::{ApiResult, AppState, err_response};
use crate::connector::Connector;
use crate::model::{QueryRequest, QueryResult};

/// `POST /api/query` — run arbitrary SQL, capped at `maxRows`.
pub async fn query(
    State(state): State<AppState>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<QueryResult> {
    let max_rows = req.max_rows.unwrap_or(state.config.max_rows);

    let start = Instant::now();
    let result = state.connector.query(&req.sql, max_rows).await;
    let elapsed = start.elapsed();

    let ok = result.is_ok();
    let row_count = result
        .as_ref()
        .ok()
        .map(|r| r.rows_affected.unwrap_or(r.row_count));
    state.metrics.record(&req.sql, &elapsed, ok, row_count);

    result.map(Json).map_err(err_response)
}
