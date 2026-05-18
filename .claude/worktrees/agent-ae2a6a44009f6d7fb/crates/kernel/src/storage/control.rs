use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crc32fast::Hasher;

use crate::format::Lsn;
use crate::format::bytes::{read_u32, read_u64, write_u32, write_u64};
use crate::{Error, Result};

pub const CONTROL_MAGIC: u32 = 0x5244_4354; // "RDCT"
pub const CONTROL_VERSION: u32 = 1;
pub const CONTROL_LEN: usize = 64;

const CONTROL_A: &str = "CONTROL_A";
const CONTROL_B: &str = "CONTROL_B";
const CHECKSUM_OFFSET: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlFile {
    pub generation: u64,
    pub checkpoint_lsn: Lsn,
    pub page_count: u64,
}

#[derive(Debug)]
pub struct ControlStore {
    dir: PathBuf,
}

impl ControlStore {
    pub fn new(dir: impl AsRef<Path>) -> Result<Self> {
        fs::create_dir_all(dir.as_ref())?;
        Ok(Self {
            dir: dir.as_ref().to_path_buf(),
        })
    }

    pub fn load_latest(&self) -> Result<Option<ControlFile>> {
        let a = self.read_named(CONTROL_A);
        let b = self.read_named(CONTROL_B);
        match (a, b) {
            (Ok(Some(a)), Ok(Some(b))) => {
                Ok(Some(if a.generation >= b.generation { a } else { b }))
            }
            (Ok(Some(a)), _) => Ok(Some(a)),
            (_, Ok(Some(b))) => Ok(Some(b)),
            (Ok(None), Ok(None)) => Ok(None),
            (Err(left), Err(_right)) => Err(left),
            (Err(_), Ok(None)) | (Ok(None), Err(_)) => Ok(None),
        }
    }

    pub fn write_next(
        &self,
        previous: Option<ControlFile>,
        checkpoint_lsn: Lsn,
        page_count: u64,
    ) -> Result<ControlFile> {
        let next = ControlFile {
            generation: previous.map(|control| control.generation + 1).unwrap_or(1),
            checkpoint_lsn,
            page_count,
        };
        let name = if next.generation.is_multiple_of(2) {
            CONTROL_B
        } else {
            CONTROL_A
        };
        self.write_named(name, &next)?;
        Ok(next)
    }

    fn read_named(&self, name: &str) -> Result<Option<ControlFile>> {
        let path = self.dir.join(name);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(path)?;
        Ok(Some(ControlFile::decode(&bytes)?))
    }

    fn write_named(&self, name: &str, control: &ControlFile) -> Result<()> {
        // Lane E failpoint: armed before the control-file generation lands on
        // disk. Crashing here forces the dual-control-file recovery path to
        // fall back to the previous generation's checksum-valid image.
        crate::fail_point!("storage::control::write");
        let path = self.dir.join(name);
        let bytes = control.encode()?;
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
}

impl ControlFile {
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = vec![0; CONTROL_LEN];
        write_u32(&mut out, 0, CONTROL_MAGIC)?;
        write_u32(&mut out, 4, CONTROL_VERSION)?;
        write_u32(&mut out, CHECKSUM_OFFSET, 0)?;
        write_u32(&mut out, 12, 0)?;
        write_u64(&mut out, 16, self.generation)?;
        write_u64(&mut out, 24, self.checkpoint_lsn.0)?;
        write_u64(&mut out, 32, self.page_count)?;
        let checksum = checksum_control_bytes(&out);
        write_u32(&mut out, CHECKSUM_OFFSET, checksum)?;
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CONTROL_LEN {
            return Err(Error::BufferTooSmall {
                needed: CONTROL_LEN,
                actual: bytes.len(),
            });
        }
        let magic = read_u32(bytes, 0)?;
        if magic != CONTROL_MAGIC {
            return Err(Error::InvalidMagic {
                expected: CONTROL_MAGIC,
                actual: magic,
            });
        }
        let version = read_u32(bytes, 4)?;
        if version != CONTROL_VERSION {
            return Err(Error::UnsupportedVersion(version as u16));
        }
        let stored = read_u32(bytes, CHECKSUM_OFFSET)?;
        let actual = checksum_control_bytes(bytes);
        if stored != actual {
            return Err(Error::InvalidChecksum);
        }
        Ok(Self {
            generation: read_u64(bytes, 16)?,
            checkpoint_lsn: Lsn(read_u64(bytes, 24)?),
            page_count: read_u64(bytes, 32)?,
        })
    }
}

fn checksum_control_bytes(bytes: &[u8]) -> u32 {
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
