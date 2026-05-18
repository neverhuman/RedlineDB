use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crc32fast::Hasher;

use crate::format::bytes::{read_u32, read_u64, write_u32, write_u64};
use crate::format::{Csn, TxId};
use crate::{Error, Result};

pub const TX_STATUS_MAGIC: u32 = 0x5244_5458; // "RDTX"
pub const TX_STATUS_VERSION: u32 = 2;
pub const TX_STATUS_HEADER_LEN: usize = 56;
pub const TX_STATUS_ENTRY_LEN: usize = 16;

const CHECKSUM_OFFSET: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TxStatusCheckpoint {
    pub generation: u64,
    pub next_tx: TxId,
    pub next_csn: Csn,
    pub published_csn: Csn,
    pub entries: Vec<(TxId, Csn)>,
}

#[derive(Debug)]
pub struct TxStatusStore {
    dir: PathBuf,
}

impl TxStatusStore {
    pub fn new(dir: impl AsRef<Path>) -> Result<Self> {
        fs::create_dir_all(dir.as_ref())?;
        Ok(Self {
            dir: dir.as_ref().to_path_buf(),
        })
    }

    pub fn load(&self, generation: u64) -> Result<TxStatusCheckpoint> {
        let bytes = fs::read(self.path_for_generation(generation))?;
        TxStatusCheckpoint::decode(&bytes)
    }

    pub fn write(&self, checkpoint: &TxStatusCheckpoint) -> Result<()> {
        let path = self.path_for_generation(checkpoint.generation);
        let bytes = checkpoint.encode()?;
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&path)?;
        file.write_all(&bytes)?;
        file.sync_data()?;
        sync_parent_dir(&self.dir)?;
        Ok(())
    }

    fn path_for_generation(&self, generation: u64) -> PathBuf {
        self.dir.join(format!("TX_STATUS_{generation:020}"))
    }
}

impl TxStatusCheckpoint {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.entries.len() > u32::MAX as usize {
            return Err(Error::CorruptPage("tx status checkpoint too large"));
        }
        let len = TX_STATUS_HEADER_LEN
            .checked_add(self.entries.len() * TX_STATUS_ENTRY_LEN)
            .ok_or(Error::CorruptPage("tx status checkpoint length overflow"))?;
        let mut out = vec![0; len];
        write_u32(&mut out, 0, TX_STATUS_MAGIC)?;
        write_u32(&mut out, 4, TX_STATUS_VERSION)?;
        write_u32(&mut out, CHECKSUM_OFFSET, 0)?;
        write_u32(&mut out, 12, self.entries.len() as u32)?;
        write_u64(&mut out, 16, self.generation)?;
        write_u64(&mut out, 24, self.next_tx.0)?;
        write_u64(&mut out, 32, self.next_csn.0)?;
        write_u64(&mut out, 40, self.published_csn.0)?;
        write_u64(&mut out, 48, 0)?;

        let mut entries = self.entries.clone();
        entries.sort_unstable_by_key(|(tx, _)| tx.0);
        for (idx, (tx, csn)) in entries.into_iter().enumerate() {
            let offset = TX_STATUS_HEADER_LEN + idx * TX_STATUS_ENTRY_LEN;
            write_u64(&mut out, offset, tx.0)?;
            write_u64(&mut out, offset + 8, csn.0)?;
        }

        let checksum = checksum_tx_status_bytes(&out);
        write_u32(&mut out, CHECKSUM_OFFSET, checksum)?;
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < TX_STATUS_HEADER_LEN {
            return Err(Error::BufferTooSmall {
                needed: TX_STATUS_HEADER_LEN,
                actual: bytes.len(),
            });
        }
        let magic = read_u32(bytes, 0)?;
        if magic != TX_STATUS_MAGIC {
            return Err(Error::InvalidMagic {
                expected: TX_STATUS_MAGIC,
                actual: magic,
            });
        }
        let version = read_u32(bytes, 4)?;
        if version != TX_STATUS_VERSION {
            return Err(Error::UnsupportedVersion(version as u16));
        }
        let entry_count = read_u32(bytes, 12)? as usize;
        let needed = TX_STATUS_HEADER_LEN
            .checked_add(entry_count * TX_STATUS_ENTRY_LEN)
            .ok_or(Error::CorruptPage("tx status checkpoint length overflow"))?;
        if bytes.len() != needed {
            return Err(Error::BufferTooSmall {
                needed,
                actual: bytes.len(),
            });
        }
        let stored = read_u32(bytes, CHECKSUM_OFFSET)?;
        let actual = checksum_tx_status_bytes(bytes);
        if stored != actual {
            return Err(Error::InvalidChecksum);
        }

        let mut entries = Vec::with_capacity(entry_count);
        for idx in 0..entry_count {
            let offset = TX_STATUS_HEADER_LEN + idx * TX_STATUS_ENTRY_LEN;
            entries.push((
                TxId(read_u64(bytes, offset)?),
                Csn(read_u64(bytes, offset + 8)?),
            ));
        }
        Ok(Self {
            generation: read_u64(bytes, 16)?,
            next_tx: TxId(read_u64(bytes, 24)?),
            next_csn: Csn(read_u64(bytes, 32)?),
            published_csn: Csn(read_u64(bytes, 40)?),
            entries,
        })
    }
}

fn checksum_tx_status_bytes(bytes: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(&bytes[..CHECKSUM_OFFSET]);
    hasher.update(&[0, 0, 0, 0]);
    hasher.update(&bytes[CHECKSUM_OFFSET + 4..]);
    hasher.finalize()
}

fn sync_parent_dir(path: &Path) -> Result<()> {
    let file = File::open(path)?;
    file.sync_data()?;
    Ok(())
}
