use crate::format::bytes::{
    crc32_with_zeroed_field, read_u16, read_u32, read_u64, write_bytes, write_u16, write_u32,
    write_u64,
};
use crate::format::ids::{Lsn, PageGeneration, PageId, RelId};
use crate::{Error, Result};

#[path = "page/methods.rs"]
mod methods;

pub const DEFAULT_PAGE_SIZE: usize = 16 * 1024;
pub const PAGE_MAGIC: u32 = 0x5244_5047; // "RDPG"
pub const PAGE_FORMAT_VERSION: u16 = 1;
pub const PAGE_HEADER_LEN: usize = 64;
pub const SLOT_LEN: usize = 4;

const CHECKSUM_OFFSET: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PageState {
    NeverAllocated = 0,
    Reusable = 1,
    Active = 2,
    Retired = 3,
    Quarantined = 4,
    Invalid = 255,
}

impl PageState {
    fn from_u8(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::NeverAllocated),
            1 => Ok(Self::Reusable),
            2 => Ok(Self::Active),
            3 => Ok(Self::Retired),
            4 => Ok(Self::Quarantined),
            255 => Ok(Self::Invalid),
            _ => Err(Error::CorruptPage("unknown page state")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum PageKind {
    Meta = 1,
    Heap = 2,
    Undo = 3,
    TxnStatus = 4,
    FreeSpace = 5,
    Visibility = 6,
    BtreeInternal = 7,
    BtreeLeaf = 8,
    BtreeMeta = 9,
}

impl PageKind {
    fn from_u16(value: u16) -> Result<Self> {
        match value {
            1 => Ok(Self::Meta),
            2 => Ok(Self::Heap),
            3 => Ok(Self::Undo),
            4 => Ok(Self::TxnStatus),
            5 => Ok(Self::FreeSpace),
            6 => Ok(Self::Visibility),
            7 => Ok(Self::BtreeInternal),
            8 => Ok(Self::BtreeLeaf),
            9 => Ok(Self::BtreeMeta),
            _ => Err(Error::CorruptPage("unknown page kind")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageHeader {
    pub kind: PageKind,
    pub page_id: PageId,
    pub rel_id: RelId,
    pub page_lsn: Lsn,
    pub generation: PageGeneration,
    pub state: PageState,
    pub free_class_hint: u8,
    pub dead_bytes_hint: u16,
    pub horizon_csn_hint: u64,
    pub lower: u16,
    pub upper: u16,
    pub special: u16,
    pub flags: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    bytes: Vec<u8>,
}

/// Page CRC32. Public for `integrity::page_csum`, which recomputes the
/// checksum on raw bytes to report `expected vs got` when `Page::from_bytes`
/// reports `InvalidChecksum`. Body is a one-line call into the shared
/// `crc32_with_zeroed_field` helper so this stays in lockstep with the
/// control / tx-status checksums.
pub fn checksum_page_bytes(bytes: &[u8]) -> u32 {
    crc32_with_zeroed_field(bytes, CHECKSUM_OFFSET)
}
