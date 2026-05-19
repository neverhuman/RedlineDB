//! Tokio async adapter for RedlineDB.
//!
//! RedlineDB's core API is synchronous and thread-bound. This crate wraps it
//! in a thin async layer built on `tokio::task::spawn_blocking`.

#![warn(missing_docs)]

use std::time::Duration;

mod async_row;
mod builder;
mod pool;

pub use async_row::AsyncRow;
pub use builder::PoolBuilder;
pub use pool::Pool;

pub use redlinedb::{
    BeginMode, Connection, Database, Error, ErrorCode, ExecuteSummary, OpenOptions, Result, Row,
    Statement, Step, Value, ValueRef, params,
};

pub(crate) const DEFAULT_MAX_CONNECTIONS: usize = 10;
pub(crate) const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
