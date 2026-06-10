//! Schema, table-schema and table-paging handlers.
//!
//! Table paging is recorded into the metrics registry (per the contract:
//! "Every query through `POST /api/query` **and table paging** is recorded").

use std::time::Instant;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use super::{ApiResult, AppState, err_response};
use crate::connector::{Connector, PageOptions};
use crate::model::{SchemaResponse, TablePage, TableSchema};

/// `GET /api/schema` — list every schema object.
pub async fn schema(State(state): State<AppState>) -> ApiResult<SchemaResponse> {
    let resp = state.connector.schema().await.map_err(err_response)?;
    Ok(Json(resp))
}

/// `GET /api/tables/{name}/schema` — full schema of one table/view.
pub async fn table_schema(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResult<TableSchema> {
    let resp = state
        .connector
        .table_schema(&name)
        .await
        .map_err(err_response)?;
    Ok(Json(resp))
}

/// Query parameters for table paging.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub order_by: Option<String>,
    pub dir: Option<String>,
}

/// `GET /api/tables/{name}?limit&offset&orderBy&dir` — a page of rows.
pub async fn table_page(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(params): Query<PageParams>,
) -> ApiResult<TablePage> {
    let max_rows = state.config.max_rows;
    let limit = params.limit.unwrap_or(50).clamp(0, max_rows.max(0));
    let offset = params.offset.unwrap_or(0).max(0);
    let opts = PageOptions {
        limit,
        offset,
        order_by: params.order_by,
        dir: params.dir,
    };

    let start = Instant::now();
    let result = state.connector.table_page(&name, &opts).await;
    let elapsed = start.elapsed();

    let ok = result.is_ok();
    let row_count = result.as_ref().ok().map(|p| p.rows.len() as i64);
    state
        .metrics
        .record(&format!("[page] {name}"), &elapsed, ok, row_count);

    result.map(Json).map_err(err_response)
}
