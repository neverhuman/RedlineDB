use crc32fast::Hasher;

use crate::format::bytes::{
    read_u16, read_u32, read_u64, write_bytes, write_u16, write_u32, write_u64,
};
use crate::format::ids::{Lsn, PageGeneration, PageId, RelId};
use crate::{Error, Result};

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

impl Page {
    pub fn new(page_size: usize, kind: PageKind, page_id: PageId, rel_id: RelId) -> Result<Self> {
        Self::new_with_special(page_size, kind, page_id, rel_id, 0)
    }

    pub fn new_with_special(
        page_size: usize,
        kind: PageKind,
        page_id: PageId,
        rel_id: RelId,
        special_len: usize,
    ) -> Result<Self> {
        if page_size < PAGE_HEADER_LEN + SLOT_LEN {
            return Err(Error::BufferTooSmall {
                needed: PAGE_HEADER_LEN + SLOT_LEN,
                actual: page_size,
            });
        }
        if special_len > page_size.saturating_sub(PAGE_HEADER_LEN) {
            return Err(Error::BufferTooSmall {
                needed: PAGE_HEADER_LEN + special_len,
                actual: page_size,
            });
        }

        let mut page = Self {
            bytes: vec![0; page_size],
        };
        let special = page_size
            .checked_sub(special_len)
            .ok_or(Error::BufferTooSmall {
                needed: PAGE_HEADER_LEN + special_len,
                actual: page_size,
            })?;
        let header = PageHeader {
            kind,
            page_id,
            rel_id,
            page_lsn: Lsn::ZERO,
            generation: PageGeneration::ONE,
            state: PageState::Active,
            free_class_hint: 0,
            dead_bytes_hint: 0,
            horizon_csn_hint: 0,
            lower: PAGE_HEADER_LEN as u16,
            upper: special as u16,
            special: special as u16,
            flags: 0,
        };
        page.write_header(&header)?;
        page.refresh_checksum()?;
        Ok(page)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() < PAGE_HEADER_LEN {
            return Err(Error::BufferTooSmall {
                needed: PAGE_HEADER_LEN,
                actual: bytes.len(),
            });
        }

        let page = Self { bytes };
        page.validate()?;
        Ok(page)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn as_mut_bytes_for_io_test(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    pub fn header(&self) -> Result<PageHeader> {
        let magic = read_u32(&self.bytes, 0)?;
        if magic != PAGE_MAGIC {
            return Err(Error::InvalidMagic {
                expected: PAGE_MAGIC,
                actual: magic,
            });
        }

        let version = read_u16(&self.bytes, 4)?;
        if version != PAGE_FORMAT_VERSION {
            return Err(Error::UnsupportedVersion(version));
        }

        let kind = PageKind::from_u16(read_u16(&self.bytes, 6)?)?;
        Ok(PageHeader {
            kind,
            page_id: PageId(read_u64(&self.bytes, 16)?),
            rel_id: RelId(read_u64(&self.bytes, 24)?),
            page_lsn: Lsn(read_u64(&self.bytes, 32)?),
            generation: PageGeneration(read_u32(&self.bytes, 48)?),
            state: PageState::from_u8(self.bytes[52])?,
            free_class_hint: self.bytes[53],
            dead_bytes_hint: read_u16(&self.bytes, 54)?,
            horizon_csn_hint: read_u64(&self.bytes, 56)?,
            lower: read_u16(&self.bytes, 40)?,
            upper: read_u16(&self.bytes, 42)?,
            special: read_u16(&self.bytes, 44)?,
            flags: read_u16(&self.bytes, 46)?,
        })
    }

    pub fn set_page_lsn(&mut self, lsn: Lsn) -> Result<()> {
        write_u64(&mut self.bytes, 32, lsn.0)?;
        self.refresh_checksum()
    }

    pub fn set_generation(&mut self, generation: PageGeneration) -> Result<()> {
        write_u32(&mut self.bytes, 48, generation.0)?;
        self.refresh_checksum()
    }

    pub fn set_state(&mut self, state: PageState) -> Result<()> {
        self.bytes[52] = state as u8;
        self.refresh_checksum()
    }

    pub fn set_free_class_hint(&mut self, free_class_hint: u8) -> Result<()> {
        self.bytes[53] = free_class_hint;
        self.refresh_checksum()
    }

    pub fn set_dead_bytes_hint(&mut self, dead_bytes_hint: u16) -> Result<()> {
        write_u16(&mut self.bytes, 54, dead_bytes_hint)?;
        self.refresh_checksum()
    }

    pub fn set_horizon_csn_hint(&mut self, horizon_csn_hint: u64) -> Result<()> {
        write_u64(&mut self.bytes, 56, horizon_csn_hint)?;
        self.refresh_checksum()
    }

    pub fn reinitialize(
        &mut self,
        kind: PageKind,
        page_id: PageId,
        rel_id: RelId,
        generation: PageGeneration,
    ) -> Result<()> {
        self.reinitialize_with_special(kind, page_id, rel_id, generation, 0)
    }

    pub fn reinitialize_with_special(
        &mut self,
        kind: PageKind,
        page_id: PageId,
        rel_id: RelId,
        generation: PageGeneration,
        special_len: usize,
    ) -> Result<()> {
        let page_size = self.bytes.len();
        if special_len > page_size.saturating_sub(PAGE_HEADER_LEN) {
            return Err(Error::BufferTooSmall {
                needed: PAGE_HEADER_LEN + special_len,
                actual: page_size,
            });
        }
        self.bytes.fill(0);
        let special = page_size
            .checked_sub(special_len)
            .ok_or(Error::BufferTooSmall {
                needed: PAGE_HEADER_LEN + special_len,
                actual: page_size,
            })?;
        let header = PageHeader {
            kind,
            page_id,
            rel_id,
            page_lsn: Lsn::ZERO,
            generation,
            state: PageState::Active,
            free_class_hint: 0,
            dead_bytes_hint: 0,
            horizon_csn_hint: 0,
            lower: PAGE_HEADER_LEN as u16,
            upper: special as u16,
            special: special as u16,
            flags: 0,
        };
        self.write_header(&header)?;
        self.refresh_checksum()
    }

    pub fn slot_count(&self) -> Result<u16> {
        let header = self.header()?;
        Ok(((header.lower as usize - PAGE_HEADER_LEN) / SLOT_LEN) as u16)
    }

    pub fn insert_cell(&mut self, payload: &[u8]) -> Result<u16> {
        let slot_no = self.slot_count()?;
        self.insert_cell_at(slot_no, payload)
    }

    pub fn insert_cell_at(&mut self, slot: u16, payload: &[u8]) -> Result<u16> {
        let mut header = self.header()?;
        let slot_count = self.slot_count()?;
        if slot > slot_count {
            return Err(Error::CorruptPage("slot out of bounds"));
        }
        let new_lower = header.lower as usize + SLOT_LEN;
        let new_upper = (header.upper as usize)
            .checked_sub(payload.len())
            .ok_or(Error::PageFull)?;

        if new_lower > new_upper {
            return Err(Error::PageFull);
        }
        if payload.len() > u16::MAX as usize {
            return Err(Error::CorruptPage("cell too large"));
        }

        let slot_offset = PAGE_HEADER_LEN + slot as usize * SLOT_LEN;
        let old_lower = header.lower as usize;
        let directory_bytes = old_lower - PAGE_HEADER_LEN;
        let tail = directory_bytes.saturating_sub(slot as usize * SLOT_LEN);
        if tail > 0 {
            let src = slot_offset..old_lower;
            self.bytes.copy_within(src, slot_offset + SLOT_LEN);
        }

        write_bytes(&mut self.bytes, new_upper, payload)?;
        write_u16(&mut self.bytes, slot_offset, new_upper as u16)?;
        write_u16(&mut self.bytes, slot_offset + 2, payload.len() as u16)?;

        header.lower = new_lower as u16;
        header.upper = new_upper as u16;
        self.write_header(&header)?;
        self.refresh_checksum()?;
        Ok(slot)
    }

    pub fn delete_cell(&mut self, slot: u16) -> Result<()> {
        let mut header = self.header()?;
        let slot_count = self.slot_count()?;
        if slot >= slot_count {
            return Err(Error::CorruptPage("slot out of bounds"));
        }

        let slot_offset = PAGE_HEADER_LEN + slot as usize * SLOT_LEN;
        let old_lower = header.lower as usize;
        let next = slot_offset + SLOT_LEN;
        if next < old_lower {
            self.bytes.copy_within(next..old_lower, slot_offset);
        }
        header.lower = header.lower.saturating_sub(SLOT_LEN as u16);
        self.write_header(&header)?;
        self.refresh_checksum()
    }

    pub fn special_bytes(&self) -> Result<&[u8]> {
        let header = self.header()?;
        Ok(&self.bytes[header.special as usize..])
    }

    pub fn special_bytes_mut(&mut self) -> Result<&mut [u8]> {
        let header = self.header()?;
        let special = header.special as usize;
        Ok(&mut self.bytes[special..])
    }

    pub fn cell(&self, slot: u16) -> Result<&[u8]> {
        let slot_count = self.slot_count()?;
        if slot >= slot_count {
            return Err(Error::CorruptPage("slot out of bounds"));
        }
        let slot_offset = PAGE_HEADER_LEN + slot as usize * SLOT_LEN;
        let cell_offset = read_u16(&self.bytes, slot_offset)? as usize;
        let cell_len = read_u16(&self.bytes, slot_offset + 2)? as usize;
        let end = cell_offset
            .checked_add(cell_len)
            .ok_or(Error::CorruptPage("cell overflow"))?;
        if end > self.bytes.len() {
            return Err(Error::CorruptPage("cell extends past page"));
        }
        Ok(&self.bytes[cell_offset..end])
    }

    pub fn overwrite_cell(&mut self, slot: u16, payload: &[u8]) -> Result<()> {
        let slot_count = self.slot_count()?;
        if slot >= slot_count {
            return Err(Error::CorruptPage("slot out of bounds"));
        }

        let slot_offset = PAGE_HEADER_LEN + slot as usize * SLOT_LEN;
        let cell_offset = read_u16(&self.bytes, slot_offset)? as usize;
        let cell_len = read_u16(&self.bytes, slot_offset + 2)? as usize;
        if payload.len() != cell_len {
            return Err(Error::CorruptPage("overwrite cell length mismatch"));
        }

        let end = cell_offset
            .checked_add(cell_len)
            .ok_or(Error::CorruptPage("cell overflow"))?;
        if end > self.bytes.len() {
            return Err(Error::CorruptPage("cell extends past page"));
        }

        write_bytes(&mut self.bytes, cell_offset, payload)?;
        self.refresh_checksum()
    }

    pub fn validate(&self) -> Result<()> {
        let header = self.header()?;
        if header.lower as usize > header.upper as usize {
            return Err(Error::CorruptPage("lower exceeds upper"));
        }
        if header.upper as usize > self.bytes.len() || header.special as usize > self.bytes.len() {
            return Err(Error::CorruptPage("bounds exceed page length"));
        }
        if header.upper as usize > header.special as usize {
            return Err(Error::CorruptPage("upper exceeds special boundary"));
        }
        if header.lower as usize > header.special as usize {
            return Err(Error::CorruptPage(
                "slot directory exceeds special boundary",
            ));
        }

        let stored = read_u32(&self.bytes, CHECKSUM_OFFSET)?;
        let actual = checksum_page_bytes(&self.bytes);
        if stored != actual {
            return Err(Error::InvalidChecksum);
        }
        Ok(())
    }

    fn write_header(&mut self, header: &PageHeader) -> Result<()> {
        write_u32(&mut self.bytes, 0, PAGE_MAGIC)?;
        write_u16(&mut self.bytes, 4, PAGE_FORMAT_VERSION)?;
        write_u16(&mut self.bytes, 6, header.kind as u16)?;
        write_u32(&mut self.bytes, CHECKSUM_OFFSET, 0)?;
        write_u32(&mut self.bytes, 12, 0)?;
        write_u64(&mut self.bytes, 16, header.page_id.0)?;
        write_u64(&mut self.bytes, 24, header.rel_id.0)?;
        write_u64(&mut self.bytes, 32, header.page_lsn.0)?;
        write_u16(&mut self.bytes, 40, header.lower)?;
        write_u16(&mut self.bytes, 42, header.upper)?;
        write_u16(&mut self.bytes, 44, header.special)?;
        write_u16(&mut self.bytes, 46, header.flags)?;
        write_u32(&mut self.bytes, 48, header.generation.0)?;
        self.bytes[52] = header.state as u8;
        self.bytes[53] = header.free_class_hint;
        write_u16(&mut self.bytes, 54, header.dead_bytes_hint)?;
        write_u64(&mut self.bytes, 56, header.horizon_csn_hint)?;
        Ok(())
    }

    fn refresh_checksum(&mut self) -> Result<()> {
        write_u32(&mut self.bytes, CHECKSUM_OFFSET, 0)?;
        let checksum = checksum_page_bytes(&self.bytes);
        write_u32(&mut self.bytes, CHECKSUM_OFFSET, checksum)
    }
}

pub fn checksum_page_bytes(bytes: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(&bytes[..CHECKSUM_OFFSET]);
    hasher.update(&[0, 0, 0, 0]);
    hasher.update(&bytes[CHECKSUM_OFFSET + 4..]);
    hasher.finalize()
}
