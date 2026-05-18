//! Per-index full leaf walk used by [`super::IntegrityReport`].
//!
//! Wraps [`crate::index::BtreeIndex::iter_all_entries`] and
//! [`crate::index::BtreeIndex::validate`] so the integrity checker can
//! reuse a single dump for both equivalence and structural checks.

use std::sync::Arc;

use crate::Result;
use crate::index::{BtreeIndex, IndexEntry, IndexValidationReport};

#[derive(Debug)]
pub(crate) struct IndexDump {
    pub entries: Vec<IndexEntry>,
    pub validation: IndexValidationReport,
}

pub(crate) fn dump_index(btree: &Arc<BtreeIndex>) -> Result<IndexDump> {
    let validation = btree.validate()?;
    let entries = btree.iter_all_entries()?;
    Ok(IndexDump {
        entries,
        validation,
    })
}
