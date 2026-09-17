//! Health and connection-info handlers.

use axum::Json;
use axum::extract::State;

use super::{ApiResult, AppState, err_response};
use crate::connector::Connector;
use crate::model::{ConnectionInfo, Engine, Health};

/// `GET /api/health` — always reports `ok`; engine is detected best-effort.
pub async fn health(State(state): State<AppState>) -> Json<Health> {
    let engine = state
        .connector
        .connection_info()
        .await
        .map(|info| info.engine)
        .unwrap_or(Engine::Sqlite);
    Json(Health {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        engine,
    })
}

/// `GET /api/connection` — describe the live connection.
pub async fn connection(State(state): State<AppState>) -> ApiResult<ConnectionInfo> {
    let info = state
        .connector
        .connection_info()
        .await
        .map_err(err_response)?;
    Ok(Json(info))
}
