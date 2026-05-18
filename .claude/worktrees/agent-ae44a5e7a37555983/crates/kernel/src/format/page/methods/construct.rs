use super::*;

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
}
