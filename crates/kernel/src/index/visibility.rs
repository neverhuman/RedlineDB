use crate::engine::ConcurrentTxStatus;
use crate::format::TxId;
use crate::txn::Snapshot;

use super::{Entry, LeafEntry, NON_TRANSACTIONAL_DELETE_TX};

pub(crate) fn entry_visible(
    entry: &Entry,
    tx_status: &ConcurrentTxStatus,
    snapshot: &Snapshot,
    owner: Option<TxId>,
) -> bool {
    let Entry::Leaf {
        create_tx,
        delete_tx,
        ..
    } = entry
    else {
        return false;
    };
    visible_leaf_state(*create_tx, *delete_tx, tx_status, snapshot, owner)
}

pub(crate) fn leaf_entry_visible(
    entry: &LeafEntry,
    tx_status: &ConcurrentTxStatus,
    snapshot: &Snapshot,
    owner: Option<TxId>,
) -> bool {
    visible_leaf_state(entry.create_tx, entry.delete_tx, tx_status, snapshot, owner)
}

fn visible_leaf_state(
    create_tx: TxId,
    delete_tx: TxId,
    tx_status: &ConcurrentTxStatus,
    snapshot: &Snapshot,
    owner: Option<TxId>,
) -> bool {
    if create_tx != TxId::ZERO && !tx_status.is_tx_visible(create_tx, snapshot, owner) {
        return false;
    }
    if delete_tx == NON_TRANSACTIONAL_DELETE_TX {
        return false;
    }
    if delete_tx != TxId::ZERO && tx_status.is_tx_visible(delete_tx, snapshot, owner) {
        return false;
    }
    true
}

pub(crate) fn delete_marker_visible(
    delete_tx: TxId,
    visibility: Option<(&ConcurrentTxStatus, &Snapshot, Option<TxId>)>,
) -> bool {
    if delete_tx == TxId::ZERO {
        return false;
    }
    if delete_tx == NON_TRANSACTIONAL_DELETE_TX {
        return true;
    }
    let Some((tx_status, snapshot, owner)) = visibility else {
        return true;
    };
    tx_status.is_tx_visible(delete_tx, snapshot, owner)
}
