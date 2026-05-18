//! WAL coordinator façade.
//!
//! The implementation lives in `coordinator/methods.rs`,
//! `coordinator/helpers.rs`, and `coordinator/writer.rs`.

use super::*;

#[path = "coordinator/control.rs"]
mod control;
#[path = "coordinator/helpers.rs"]
mod helpers;
#[path = "coordinator/methods.rs"]
mod methods;
#[path = "coordinator/writer.rs"]
mod writer;
