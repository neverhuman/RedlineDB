use crc32fast::Hasher;

use crate::{Error, Result};

/// CRC32 over `bytes` with the four-byte slot starting at
/// `checksum_offset` treated as zero. Mirrors the on-disk convention used
/// by the page header, the dual control file, and the tx-status
/// checkpoint: reserve the slot, hash the buffer, write the digest back
/// into the slot. Centralised here so all three callers stay in lockstep.
#[inline]
pub fn crc32_with_zeroed_field(bytes: &[u8], checksum_offset: usize) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(&bytes[..checksum_offset]);
    hasher.update(&[0, 0, 0, 0]);
    hasher.update(&bytes[checksum_offset + 4..]);
    hasher.finalize()
}

#[inline]
pub fn read_u16(buf: &[u8], offset: usize) -> Result<u16> {
    let bytes = read_array::<2>(buf, offset)?;
    Ok(u16::from_le_bytes(bytes))
}

#[inline]
pub fn read_u32(buf: &[u8], offset: usize) -> Result<u32> {
    let bytes = read_array::<4>(buf, offset)?;
    Ok(u32::from_le_bytes(bytes))
}

#[inline]
pub fn read_u64(buf: &[u8], offset: usize) -> Result<u64> {
    let bytes = read_array::<8>(buf, offset)?;
    Ok(u64::from_le_bytes(bytes))
}

// dedup-allowed: type-specialization — the write_uN trio look identical in
// body shape but each one binds to a different `to_le_bytes` impl on its
// respective primitive type. Folding them into a generic via a trait like
// `ToLeBytes` would force an extra trait bound at every call site and is
// known to suppress the small-buffer inlining that `to_le_bytes` enjoys
// today. Leave as-is.
#[inline]
pub fn write_u16(buf: &mut [u8], offset: usize, value: u16) -> Result<()> {
    write_bytes(buf, offset, &value.to_le_bytes())
}

#[inline]
pub fn write_u32(buf: &mut [u8], offset: usize, value: u32) -> Result<()> {
    write_bytes(buf, offset, &value.to_le_bytes())
}

#[inline]
pub fn write_u64(buf: &mut [u8], offset: usize, value: u64) -> Result<()> {
    write_bytes(buf, offset, &value.to_le_bytes())
}

#[inline]
pub fn write_bytes(buf: &mut [u8], offset: usize, bytes: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(bytes.len())
        .ok_or(Error::BufferTooSmall {
            needed: usize::MAX,
            actual: buf.len(),
        })?;
    if end > buf.len() {
        return Err(Error::BufferTooSmall {
            needed: end,
            actual: buf.len(),
        });
    }
    buf[offset..end].copy_from_slice(bytes);
    Ok(())
}

#[inline]
fn read_array<const N: usize>(buf: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::BufferTooSmall {
        needed: usize::MAX,
        actual: buf.len(),
    })?;
    if end > buf.len() {
        return Err(Error::BufferTooSmall {
            needed: end,
            actual: buf.len(),
        });
    }
    let mut out = [0_u8; N];
    out.copy_from_slice(&buf[offset..end]);
    Ok(out)
}
