use crc32fast::Hasher;

use crate::format::bytes::{
    read_u16, read_u32, read_u64, write_bytes, write_u16, write_u32, write_u64,
};
use crate::format::ids::{Lsn, TxId};
use crate::{Error, Result};

pub const WAL_MAGIC: u32 = 0x5244_574c; // "RDWL"
pub const WAL_FORMAT_VERSION: u16 = 1;
pub const WAL_HEADER_LEN: usize = 48;
const WAL_CRC_OFFSET: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum WalRecordKind {
    Begin = 1,
    PageImage = 2,
    PageDelta = 3,
    UndoAppend = 4,
    Commit = 5,
    Abort = 6,
    CheckpointBegin = 7,
    CheckpointEnd = 8,
    SegmentSeal = 9,
    BackupBegin = 10,
    BackupEnd = 11,
    TimelineFork = 12,
    Logical = 13,
}

impl WalRecordKind {
    fn from_u16(value: u16) -> Result<Self> {
        match value {
            1 => Ok(Self::Begin),
            2 => Ok(Self::PageImage),
            3 => Ok(Self::PageDelta),
            4 => Ok(Self::UndoAppend),
            5 => Ok(Self::Commit),
            6 => Ok(Self::Abort),
            7 => Ok(Self::CheckpointBegin),
            8 => Ok(Self::CheckpointEnd),
            9 => Ok(Self::SegmentSeal),
            10 => Ok(Self::BackupBegin),
            11 => Ok(Self::BackupEnd),
            12 => Ok(Self::TimelineFork),
            13 => Ok(Self::Logical),
            _ => Err(Error::CorruptWal("unknown record kind")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalRecord {
    pub lsn: Lsn,
    pub prev_lsn: Lsn,
    pub tx_id: TxId,
    pub kind: WalRecordKind,
    pub payload: Vec<u8>,
}

impl WalRecord {
    pub fn encoded_len(&self) -> usize {
        WAL_HEADER_LEN + self.payload.len()
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.payload.len() > u32::MAX as usize {
            return Err(Error::CorruptWal("payload too large"));
        }

        let mut out = vec![0; self.encoded_len()];
        write_u32(&mut out, 0, WAL_MAGIC)?;
        write_u16(&mut out, 4, WAL_FORMAT_VERSION)?;
        write_u16(&mut out, 6, self.kind as u16)?;
        write_u32(&mut out, WAL_CRC_OFFSET, 0)?;
        write_u32(&mut out, 12, self.payload.len() as u32)?;
        write_u64(&mut out, 16, self.lsn.0)?;
        write_u64(&mut out, 24, self.prev_lsn.0)?;
        write_u64(&mut out, 32, self.tx_id.0)?;
        write_u64(&mut out, 40, 0)?;
        write_bytes(&mut out, WAL_HEADER_LEN, &self.payload)?;
        let crc = checksum_wal_bytes(&out);
        write_u32(&mut out, WAL_CRC_OFFSET, crc)?;
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < WAL_HEADER_LEN {
            return Err(Error::BufferTooSmall {
                needed: WAL_HEADER_LEN,
                actual: bytes.len(),
            });
        }
        let magic = read_u32(bytes, 0)?;
        if magic != WAL_MAGIC {
            return Err(Error::InvalidMagic {
                expected: WAL_MAGIC,
                actual: magic,
            });
        }
        let version = read_u16(bytes, 4)?;
        if version != WAL_FORMAT_VERSION {
            return Err(Error::UnsupportedVersion(version));
        }

        let payload_len = read_u32(bytes, 12)? as usize;
        let record_len = WAL_HEADER_LEN
            .checked_add(payload_len)
            .ok_or(Error::CorruptWal("record length overflow"))?;
        if bytes.len() < record_len {
            return Err(Error::BufferTooSmall {
                needed: record_len,
                actual: bytes.len(),
            });
        }

        let stored = read_u32(bytes, WAL_CRC_OFFSET)?;
        let actual = checksum_wal_bytes(&bytes[..record_len]);
        if stored != actual {
            return Err(Error::InvalidChecksum);
        }

        Ok(Self {
            lsn: Lsn(read_u64(bytes, 16)?),
            prev_lsn: Lsn(read_u64(bytes, 24)?),
            tx_id: TxId(read_u64(bytes, 32)?),
            kind: WalRecordKind::from_u16(read_u16(bytes, 6)?)?,
            payload: bytes[WAL_HEADER_LEN..record_len].to_vec(),
        })
    }
}

pub fn checksum_wal_bytes(bytes: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(&bytes[..WAL_CRC_OFFSET]);
    hasher.update(&[0, 0, 0, 0]);
    hasher.update(&bytes[WAL_CRC_OFFSET + 4..]);
    hasher.finalize()
}
