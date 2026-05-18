use crate::{Error, Result};

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
