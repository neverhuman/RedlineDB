use std::cmp::Ordering;

use crate::format::bytes::{read_u16, read_u32, read_u64};
use crate::format::{PageGeneration, PageId, TuplePtr, TxId};
use crate::{Error, Result};

use super::{IndexRowRef, NON_TRANSACTIONAL_DELETE_TX};

#[path = "visibility.rs"]
mod visibility;
pub(crate) use visibility::{delete_marker_visible, entry_visible, leaf_entry_visible};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Entry {
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
        encode_leaf_cell(logical_key, row, physical, create_tx, delete_tx)
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<Entry> {
        let (logical_key, row, physical, create_tx, delete_tx) = decode_leaf_parts(bytes)?;
        Ok(Entry::Leaf {
            logical_key,
            row,
            physical,
            create_tx,
            delete_tx,
        })
    }

    pub(super) fn decode_leaf_entry(bytes: &[u8]) -> Result<LeafEntry> {
        let (logical_key, row, _physical, create_tx, delete_tx) = decode_leaf_parts(bytes)?;
        Ok(LeafEntry {
            logical_key,
            row,
            create_tx,
            delete_tx,
        })
    }

    pub(super) fn decode_ref<'a>(bytes: &'a [u8]) -> Result<LeafCellRef<'a>> {
        let header = read_leaf_header(bytes)?;
        let (logical_start, physical_start, physical_end) = leaf_payload_bounds(&header);
        if physical_end > bytes.len() {
            return Err(Error::CorruptPage("leaf cell overflow"));
        }
        Ok(LeafCellRef {
            logical_key: &bytes[logical_start..physical_start],
            row: header.row,
            create_tx: header.create_tx,
            delete_tx: header.delete_tx,
        })
    }
}

fn decode_leaf_parts(bytes: &[u8]) -> Result<(Vec<u8>, IndexRowRef, Vec<u8>, TxId, TxId)> {
    let header = read_leaf_header(bytes)?;
    let (logical_start, physical_start, physical_end) = leaf_payload_bounds(&header);
    if physical_end > bytes.len() {
        return Err(Error::CorruptPage("leaf cell overflow"));
    }
    Ok((
        bytes[logical_start..physical_start].to_vec(),
        header.row,
        bytes[physical_start..physical_end].to_vec(),
        header.create_tx,
        header.delete_tx,
    ))
}

impl InternalCell {
    pub(super) fn encode(separator: &[u8], child: PageId) -> Vec<u8> {
        encode_internal_cell(separator, child)
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

#[derive(Clone, Copy)]
struct LeafCellHeader {
    logical_len: usize,
    physical_len: usize,
    row: IndexRowRef,
    create_tx: TxId,
    delete_tx: TxId,
}

fn encode_leaf_cell(
    logical_key: &[u8],
    row: IndexRowRef,
    physical: &[u8],
    create_tx: TxId,
    delete_tx: TxId,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(42 + logical_key.len() + physical.len());
    push_u16(&mut out, logical_key.len() as u16);
    push_u16(&mut out, physical.len() as u16);
    push_u64(&mut out, row.row_id.0);
    push_u64(&mut out, row.tuple.page_id.0);
    push_u16(&mut out, row.tuple.slot);
    push_u32(&mut out, row.tuple.generation.0);
    push_u64(&mut out, create_tx.0);
    push_u64(&mut out, delete_tx.0);
    out.extend_from_slice(logical_key);
    out.extend_from_slice(physical);
    out
}

fn encode_internal_cell(separator: &[u8], child: PageId) -> Vec<u8> {
    let mut out = Vec::with_capacity(10 + separator.len());
    push_u16(&mut out, separator.len() as u16);
    push_u64(&mut out, child.0);
    out.extend_from_slice(separator);
    out
}

fn read_leaf_header(bytes: &[u8]) -> Result<LeafCellHeader> {
    if bytes.len() < 42 {
        return Err(Error::BufferTooSmall {
            needed: 42,
            actual: bytes.len(),
        });
    }
    Ok(LeafCellHeader {
        logical_len: read_u16(bytes, 0)? as usize,
        physical_len: read_u16(bytes, 2)? as usize,
        row: IndexRowRef {
            row_id: crate::format::RowId(read_u64(bytes, 4)?),
            tuple: TuplePtr::new_with_generation(
                PageId(read_u64(bytes, 12)?),
                read_u16(bytes, 20)?,
                PageGeneration(read_u32(bytes, 22)?),
            ),
        },
        create_tx: TxId(read_u64(bytes, 26)?),
        delete_tx: TxId(read_u64(bytes, 34)?),
    })
}

fn leaf_payload_bounds(header: &LeafCellHeader) -> (usize, usize, usize) {
    let logical_start = 42;
    let physical_start = logical_start + header.logical_len;
    let physical_end = physical_start + header.physical_len;
    (logical_start, physical_start, physical_end)
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
