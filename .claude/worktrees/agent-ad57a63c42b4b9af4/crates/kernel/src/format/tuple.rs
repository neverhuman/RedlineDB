use crate::format::bytes::{
    read_u16, read_u32, read_u64, write_bytes, write_u16, write_u32, write_u64,
};
use crate::format::ids::{Csn, RelId, RowId, TxId, UndoPtr};
use crate::txn::{Snapshot, TxStatusTable};
use crate::{Error, Result};

pub const TUPLE_MAGIC: u32 = 0x5244_5450; // "RDTP"
pub const TUPLE_FORMAT_VERSION: u16 = 2;
pub const TUPLE_FORMAT_VERSION_V1: u16 = 1;
pub const TUPLE_HEADER_LEN: usize = 72;

pub const TUPLE_FLAG_DELETED: u16 = 1 << 0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TupleVersion {
    pub row_id: RowId,
    pub rel_id: RelId,
    pub begin_tx: TxId,
    pub end_tx: TxId,
    pub begin_csn_hint: Csn,
    pub end_csn_hint: Csn,
    pub undo_head: UndoPtr,
    pub flags: u16,
    pub payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TupleVisibility {
    Visible,
    Deleted,
    Invisible,
}

impl TupleVersion {
    pub fn new(row_id: RowId, rel_id: RelId, begin_tx: TxId, payload: Vec<u8>) -> Self {
        Self {
            row_id,
            rel_id,
            begin_tx,
            end_tx: TxId::ZERO,
            begin_csn_hint: Csn::ZERO,
            end_csn_hint: Csn::ZERO,
            undo_head: UndoPtr::ZERO,
            flags: 0,
            payload,
        }
    }

    pub fn deleted(row_id: RowId, rel_id: RelId, begin_tx: TxId) -> Self {
        Self {
            row_id,
            rel_id,
            begin_tx,
            end_tx: TxId::ZERO,
            begin_csn_hint: Csn::ZERO,
            end_csn_hint: Csn::ZERO,
            undo_head: UndoPtr::ZERO,
            flags: TUPLE_FLAG_DELETED,
            payload: Vec::new(),
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.payload.len() > u32::MAX as usize {
            return Err(Error::CorruptPage("tuple payload too large"));
        }

        let mut out = vec![0; TUPLE_HEADER_LEN + self.payload.len()];
        write_u32(&mut out, 0, TUPLE_MAGIC)?;
        write_u16(&mut out, 4, TUPLE_FORMAT_VERSION)?;
        write_u16(&mut out, 6, self.flags)?;
        write_u64(&mut out, 8, self.row_id.0)?;
        write_u64(&mut out, 16, self.begin_tx.0)?;
        write_u64(&mut out, 24, self.end_tx.0)?;
        write_u64(&mut out, 32, self.begin_csn_hint.0)?;
        write_u64(&mut out, 40, self.end_csn_hint.0)?;
        write_u64(&mut out, 48, self.undo_head.0)?;
        write_u32(&mut out, 56, self.payload.len() as u32)?;
        write_u32(&mut out, 60, 0)?;
        write_u64(&mut out, 64, self.rel_id.0)?;
        write_bytes(&mut out, TUPLE_HEADER_LEN, &self.payload)?;
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < TUPLE_HEADER_LEN {
            return Err(Error::BufferTooSmall {
                needed: TUPLE_HEADER_LEN,
                actual: bytes.len(),
            });
        }

        let magic = read_u32(bytes, 0)?;
        if magic != TUPLE_MAGIC {
            return Err(Error::InvalidMagic {
                expected: TUPLE_MAGIC,
                actual: magic,
            });
        }
        let version = read_u16(bytes, 4)?;
        let payload_len = read_u32(bytes, 56)? as usize;
        let end = TUPLE_HEADER_LEN
            .checked_add(payload_len)
            .ok_or(Error::CorruptPage("tuple length overflow"))?;
        if bytes.len() < end {
            return Err(Error::BufferTooSmall {
                needed: end,
                actual: bytes.len(),
            });
        }

        Ok(Self {
            flags: read_u16(bytes, 6)?,
            row_id: RowId(read_u64(bytes, 8)?),
            rel_id: match version {
                TUPLE_FORMAT_VERSION => RelId(read_u64(bytes, 64)?),
                TUPLE_FORMAT_VERSION_V1 => RelId::ZERO,
                _ => return Err(Error::UnsupportedVersion(version)),
            },
            begin_tx: TxId(read_u64(bytes, 16)?),
            end_tx: TxId(read_u64(bytes, 24)?),
            begin_csn_hint: Csn(read_u64(bytes, 32)?),
            end_csn_hint: Csn(read_u64(bytes, 40)?),
            undo_head: UndoPtr(read_u64(bytes, 48)?),
            payload: bytes[TUPLE_HEADER_LEN..end].to_vec(),
        })
    }

    pub fn visibility(
        &self,
        tx_status: &TxStatusTable,
        snapshot: &Snapshot,
        owner: Option<TxId>,
    ) -> TupleVisibility {
        if !tx_status.is_tx_visible(self.begin_tx, snapshot, owner) {
            return TupleVisibility::Invisible;
        }

        if self.end_tx != TxId::ZERO && tx_status.is_tx_visible(self.end_tx, snapshot, owner) {
            return TupleVisibility::Invisible;
        }

        if self.flags & TUPLE_FLAG_DELETED != 0 {
            TupleVisibility::Deleted
        } else {
            TupleVisibility::Visible
        }
    }
}
