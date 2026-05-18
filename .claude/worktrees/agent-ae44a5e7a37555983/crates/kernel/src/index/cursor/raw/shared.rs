use std::sync::atomic::Ordering as AtomicOrdering;

use crate::Result;
use crate::format::TxId;
use crate::telemetry::Phase11Counters;

use super::super::super::NON_TRANSACTIONAL_DELETE_TX;
use super::super::super::cells::LeafCellRef;
use super::super::{CursorYield, SnapshotView, SnapshotViewKind, cached_tx_visible};

#[derive(Clone, Copy)]
pub(crate) enum BatchKind {
    Point,
    Range,
}

pub(crate) fn finish_batch(
    counters: Option<&Phase11Counters>,
    kind: BatchKind,
    pushed: usize,
) -> Result<CursorYield> {
    if pushed == 0 {
        return Ok(CursorYield::End);
    }
    bump_batch_counters(counters, kind);
    Ok(CursorYield::Batch(pushed))
}

pub(crate) fn bump_batch_counters(counters: Option<&Phase11Counters>, kind: BatchKind) {
    if let Some(c) = counters {
        c.cursor_batches_emitted
            .fetch_add(1, AtomicOrdering::Relaxed);
        match kind {
            BatchKind::Point => {
                c.index_raw_point_batches
                    .fetch_add(1, AtomicOrdering::Relaxed);
            }
            BatchKind::Range => {
                c.index_raw_range_batches
                    .fetch_add(1, AtomicOrdering::Relaxed);
            }
        }
    }
}

pub(crate) fn matches_ref_cached(
    view: SnapshotView<'_>,
    entry: &LeafCellRef<'_>,
    cache: &mut Vec<(TxId, bool)>,
) -> bool {
    match view.inner {
        SnapshotViewKind::All => entry.delete_tx == TxId::ZERO,
        SnapshotViewKind::Visible {
            tx_status,
            snapshot,
            owner,
        } => {
            if entry.create_tx != TxId::ZERO
                && !cached_tx_visible(tx_status, snapshot, owner, entry.create_tx, cache)
            {
                return false;
            }
            if entry.delete_tx == NON_TRANSACTIONAL_DELETE_TX {
                return false;
            }
            if entry.delete_tx != TxId::ZERO
                && cached_tx_visible(tx_status, snapshot, owner, entry.delete_tx, cache)
            {
                return false;
            }
            true
        }
    }
}

pub(crate) fn close_cursor<T>(cursor: T) {
    drop(cursor);
}
