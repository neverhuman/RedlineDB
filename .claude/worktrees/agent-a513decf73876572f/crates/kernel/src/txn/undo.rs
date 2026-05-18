use crate::format::bytes::{
    read_u16, read_u32, read_u64, write_bytes, write_u16, write_u32, write_u64,
};
use crate::format::ids::{RowId, TxId, UndoPtr};
use crate::{Error, Result};

pub const UNDO_MAGIC: u32 = 0x5244_554e; // "RDUN"
pub const UNDO_FORMAT_VERSION: u16 = 1;
pub const UNDO_HEADER_LEN: usize = 48;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum UndoKind {
    InsertDelete = 1,
    UpdateBeforeImage = 2,
    DeleteBeforeImage = 3,
}

impl UndoKind {
    fn from_u16(value: u16) -> Result<Self> {
        match value {
            1 => Ok(Self::InsertDelete),
            2 => Ok(Self::UpdateBeforeImage),
            3 => Ok(Self::DeleteBeforeImage),
            _ => Err(Error::CorruptPage("unknown undo kind")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndoRecord {
    pub kind: UndoKind,
    pub tx_id: TxId,
    pub row_id: RowId,
    pub prev_undo: UndoPtr,
    pub before_image: Vec<u8>,
}

impl UndoRecord {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.before_image.len() > u32::MAX as usize {
            return Err(Error::CorruptPage("undo image too large"));
        }

        let mut out = vec![0; UNDO_HEADER_LEN + self.before_image.len()];
        write_u32(&mut out, 0, UNDO_MAGIC)?;
        write_u16(&mut out, 4, UNDO_FORMAT_VERSION)?;
        write_u16(&mut out, 6, self.kind as u16)?;
        write_u64(&mut out, 8, self.tx_id.0)?;
        write_u64(&mut out, 16, self.row_id.0)?;
        write_u64(&mut out, 24, self.prev_undo.0)?;
        write_u32(&mut out, 32, self.before_image.len() as u32)?;
        write_u32(&mut out, 36, 0)?;
        write_u64(&mut out, 40, 0)?;
        write_bytes(&mut out, UNDO_HEADER_LEN, &self.before_image)?;
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < UNDO_HEADER_LEN {
            return Err(Error::BufferTooSmall {
                needed: UNDO_HEADER_LEN,
                actual: bytes.len(),
            });
        }

        let magic = read_u32(bytes, 0)?;
        if magic != UNDO_MAGIC {
            return Err(Error::InvalidMagic {
                expected: UNDO_MAGIC,
                actual: magic,
            });
        }
        let version = read_u16(bytes, 4)?;
        if version != UNDO_FORMAT_VERSION {
            return Err(Error::UnsupportedVersion(version));
        }

        let image_len = read_u32(bytes, 32)? as usize;
        let end = UNDO_HEADER_LEN
            .checked_add(image_len)
            .ok_or(Error::CorruptPage("undo length overflow"))?;
        if bytes.len() < end {
            return Err(Error::BufferTooSmall {
                needed: end,
                actual: bytes.len(),
            });
        }

        Ok(Self {
            kind: UndoKind::from_u16(read_u16(bytes, 6)?)?,
            tx_id: TxId(read_u64(bytes, 8)?),
            row_id: RowId(read_u64(bytes, 16)?),
            prev_undo: UndoPtr(read_u64(bytes, 24)?),
            before_image: bytes[UNDO_HEADER_LEN..end].to_vec(),
        })
    }
}
