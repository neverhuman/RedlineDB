use std::cmp::Ordering;

use crate::engine::ConcurrentTxStatus;
use crate::format::bytes::{read_u16, read_u32, read_u64};
use crate::format::{Csn, PageGeneration, PageId, TuplePtr, TxId};
use crate::txn::{Snapshot, TxState};
use crate::{Error, Result};

use super::{IndexRowRef, NON_TRANSACTIONAL_DELETE_TX};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Entry {
    Leaf {
        logical_key: Vec<u8>,
        row: IndexRowRef,
        physical: Vec<u8>,
        create_tx: TxId,
        delete_tx: TxId,
    },
    Internal {
        separator: Vec<u8>,
        child: PageId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LeafEntry {
    pub logical_key: Vec<u8>,
    pub row: IndexRowRef,
    pub create_tx: TxId,
    pub delete_tx: TxId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LeafCellRef<'a> {
    pub logical_key: &'a [u8],
    pub row: IndexRowRef,
    pub create_tx: TxId,
    pub delete_tx: TxId,
}

impl Entry {
    pub(super) fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (
                Entry::Leaf { physical: left, .. },
                Entry::Leaf {
                    physical: right, ..
                },
            ) => left.cmp(right),
            (
                Entry::Internal {
                    separator: left, ..
                },
                Entry::Internal {
                    separator: right, ..
                },
            ) => left.cmp(right),
            _ => Ordering::Equal,
        }
    }

    pub(super) fn logical_key(&self) -> Option<&[u8]> {
        match self {
            Entry::Leaf { logical_key, .. } => Some(logical_key),
            Entry::Internal { separator, .. } => Some(separator),
        }
    }

    pub(super) fn physical(&self) -> Option<&[u8]> {
        match self {
            Entry::Leaf { physical, .. } => Some(physical),
            Entry::Internal { .. } => None,
        }
    }

    pub(super) fn physically_live(&self) -> bool {
        matches!(self, Entry::Leaf { delete_tx, .. } if *delete_tx == TxId::ZERO)
    }

    pub(super) fn is_committed_deleted_before(
        &self,
        tx_status: &ConcurrentTxStatus,
        horizon: Csn,
    ) -> bool {
        match self {
            Entry::Leaf { delete_tx, .. } if *delete_tx != TxId::ZERO => {
                matches!(tx_status.state(*delete_tx), TxState::Committed(csn) if csn < horizon)
            }
            _ => false,
        }
    }
}

pub(super) struct LeafCell;
pub(super) struct InternalCell;

impl LeafCell {
    pub(super) fn encode(
        logical_key: &[u8],
        row: IndexRowRef,
        physical: &[u8],
        create_tx: TxId,
        delete_tx: TxId,
    ) -> Vec<u8> {
        let mut out = Vec::with_capacity(42 + logical_key.len() + physical.len());
        out.extend_from_slice(&(logical_key.len() as u16).to_le_bytes());
        out.extend_from_slice(&(physical.len() as u16).to_le_bytes());
        out.extend_from_slice(&row.row_id.0.to_le_bytes());
        out.extend_from_slice(&row.tuple.page_id.0.to_le_bytes());
        out.extend_from_slice(&row.tuple.slot.to_le_bytes());
        out.extend_from_slice(&row.tuple.generation.0.to_le_bytes());
        out.extend_from_slice(&create_tx.0.to_le_bytes());
        out.extend_from_slice(&delete_tx.0.to_le_bytes());
        out.extend_from_slice(logical_key);
        out.extend_from_slice(physical);
        out
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<Entry> {
        if bytes.len() < 42 {
            return Err(Error::BufferTooSmall {
                needed: 42,
                actual: bytes.len(),
            });
        }
        let logical_len = read_u16(bytes, 0)? as usize;
        let physical_len = read_u16(bytes, 2)? as usize;
        let row = IndexRowRef {
            row_id: crate::format::RowId(read_u64(bytes, 4)?),
            tuple: TuplePtr::new_with_generation(
                PageId(read_u64(bytes, 12)?),
                read_u16(bytes, 20)?,
                PageGeneration(read_u32(bytes, 22)?),
            ),
        };
        let create_tx = TxId(read_u64(bytes, 26)?);
        let delete_tx = TxId(read_u64(bytes, 34)?);
        let logical_start = 42;
        let physical_start = logical_start + logical_len;
        let physical_end = physical_start + physical_len;
        if physical_end > bytes.len() {
            return Err(Error::CorruptPage("leaf cell overflow"));
        }
        Ok(Entry::Leaf {
            logical_key: bytes[logical_start..physical_start].to_vec(),
            row,
            physical: bytes[physical_start..physical_end].to_vec(),
            create_tx,
            delete_tx,
        })
    }

    pub(super) fn decode_leaf_entry(bytes: &[u8]) -> Result<LeafEntry> {
        if bytes.len() < 42 {
            return Err(Error::BufferTooSmall {
                needed: 42,
                actual: bytes.len(),
            });
        }
        let logical_len = read_u16(bytes, 0)? as usize;
        let physical_len = read_u16(bytes, 2)? as usize;
        let row = IndexRowRef {
            row_id: crate::format::RowId(read_u64(bytes, 4)?),
            tuple: TuplePtr::new_with_generation(
                PageId(read_u64(bytes, 12)?),
                read_u16(bytes, 20)?,
                PageGeneration(read_u32(bytes, 22)?),
            ),
        };
        let create_tx = TxId(read_u64(bytes, 26)?);
        let delete_tx = TxId(read_u64(bytes, 34)?);
        let logical_start = 42;
        let physical_start = logical_start + logical_len;
        let physical_end = physical_start + physical_len;
        if physical_end > bytes.len() {
            return Err(Error::CorruptPage("leaf cell overflow"));
        }
        Ok(LeafEntry {
            logical_key: bytes[logical_start..physical_start].to_vec(),
            row,
            create_tx,
            delete_tx,
        })
    }

    pub(super) fn decode_ref<'a>(bytes: &'a [u8]) -> Result<LeafCellRef<'a>> {
        if bytes.len() < 42 {
            return Err(Error::BufferTooSmall {
                needed: 42,
                actual: bytes.len(),
            });
        }
        let logical_len = read_u16(bytes, 0)? as usize;
        let physical_len = read_u16(bytes, 2)? as usize;
        let row = IndexRowRef {
            row_id: crate::format::RowId(read_u64(bytes, 4)?),
            tuple: TuplePtr::new_with_generation(
                PageId(read_u64(bytes, 12)?),
                read_u16(bytes, 20)?,
                PageGeneration(read_u32(bytes, 22)?),
            ),
        };
        let create_tx = TxId(read_u64(bytes, 26)?);
        let delete_tx = TxId(read_u64(bytes, 34)?);
        let logical_start = 42;
        let physical_start = logical_start + logical_len;
        let physical_end = physical_start + physical_len;
        if physical_end > bytes.len() {
            return Err(Error::CorruptPage("leaf cell overflow"));
        }
        Ok(LeafCellRef {
            logical_key: &bytes[logical_start..physical_start],
            row,
            create_tx,
            delete_tx,
        })
    }
}

impl InternalCell {
    pub(super) fn encode(separator: &[u8], child: PageId) -> Vec<u8> {
        let mut out = Vec::with_capacity(10 + separator.len());
        out.extend_from_slice(&(separator.len() as u16).to_le_bytes());
        out.extend_from_slice(&child.0.to_le_bytes());
        out.extend_from_slice(separator);
        out
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<Entry> {
        if bytes.len() < 10 {
            return Err(Error::BufferTooSmall {
                needed: 10,
                actual: bytes.len(),
            });
        }
        let len = read_u16(bytes, 0)? as usize;
        let child = PageId(read_u64(bytes, 2)?);
        let end = 10 + len;
        if end > bytes.len() {
            return Err(Error::CorruptPage("internal cell overflow"));
        }
        Ok(Entry::Internal {
            separator: bytes[10..end].to_vec(),
            child,
        })
    }
}

pub(super) fn entry_visible(
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
    let create_check = *create_tx != TxId::ZERO
        && !tx_status.is_tx_visible(*create_tx, snapshot, owner);
    if create_check {
        eprintln!(
            "ENTRY_VISIBLE: reject create_tx={:?} owner={:?} snapshot is_tx_visible=false",
            create_tx, owner
        );
        return false;
    }
    if *delete_tx == NON_TRANSACTIONAL_DELETE_TX {
        eprintln!("ENTRY_VISIBLE: reject non-transactional delete");
        return false;
    }
    if *delete_tx != TxId::ZERO && tx_status.is_tx_visible(*delete_tx, snapshot, owner) {
        eprintln!("ENTRY_VISIBLE: reject delete_tx={:?} visible", delete_tx);
        return false;
    }
    eprintln!("ENTRY_VISIBLE: accept create_tx={:?} delete_tx={:?}", create_tx, delete_tx);
    true
}

pub(super) fn leaf_entry_visible(
    entry: &LeafEntry,
    tx_status: &ConcurrentTxStatus,
    snapshot: &Snapshot,
    owner: Option<TxId>,
) -> bool {
    if entry.create_tx != TxId::ZERO && !tx_status.is_tx_visible(entry.create_tx, snapshot, owner) {
        return false;
    }
    if entry.delete_tx == NON_TRANSACTIONAL_DELETE_TX {
        return false;
    }
    if entry.delete_tx != TxId::ZERO && tx_status.is_tx_visible(entry.delete_tx, snapshot, owner) {
        return false;
    }
    true
}

pub(super) fn delete_marker_visible(
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
