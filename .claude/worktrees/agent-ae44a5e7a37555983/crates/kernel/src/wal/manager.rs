//! WAL manager façade.
//!
//! Configuration, counters, and shared types live in `manager/config.rs`,
//! `manager/counters.rs`, and `manager/types.rs`. The operational logic stays
//! in `manager/coordinator.rs` and `manager/storage.rs`.

#[path = "manager/config.rs"]
mod config;
mod coordinator;
#[path = "manager/counters.rs"]
mod counters;
mod storage;
#[path = "manager/types.rs"]
mod types;

pub use config::*;
pub use counters::*;
pub use types::*;
