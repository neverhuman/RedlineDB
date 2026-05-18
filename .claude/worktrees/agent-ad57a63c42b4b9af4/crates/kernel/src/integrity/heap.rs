//! Per-relation heap walk used by [`super::IntegrityReport`].
//!
//! Returns the visible-at-snapshot `(RowId, payload)` pairs for one
//! relation. Visibility uses the engine's current published-CSN snapshot —
//! the integrity check is intended to be run between transactions, not
//! against an in-flight one, so any uncommitted writes are correctly
//! invisible to it.

use crate::Result;
use crate::format::{RelId, RowId};

use super::CheckContext;

pub(crate) fn collect_visible_rows(
    ctx: &CheckContext<'_>,
    rel_id: RelId,
) -> Result<Vec<(RowId, Vec<u8>)>> {
    let snapshot = ctx.engine.txs_snapshot_for_integrity();
    ctx.engine
        .heap_for_integrity()
        .iter_visible_rows_for_relation(ctx.engine.txs_for_integrity(), &snapshot, rel_id)
}
