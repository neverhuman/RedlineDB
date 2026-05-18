use super::*;

impl Page {
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

    pub(crate) fn write_header(&mut self, header: &PageHeader) -> Result<()> {
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

    pub(crate) fn refresh_checksum(&mut self) -> Result<()> {
        write_u32(&mut self.bytes, CHECKSUM_OFFSET, 0)?;
        let checksum = checksum_page_bytes(&self.bytes);
        write_u32(&mut self.bytes, CHECKSUM_OFFSET, checksum)
    }
}
