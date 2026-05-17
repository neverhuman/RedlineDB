//! WAL storage façade.
//!
//! Write-side mutation logic lives in `storage/write.rs`; read-side
//! recovery scanning lives in `storage/scan.rs`.

use super::*;

#[path = "storage/scan.rs"]
mod scan;
#[path = "storage/write.rs"]
mod write;
