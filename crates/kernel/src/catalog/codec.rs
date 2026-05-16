//! Shared LE-primitive writer/reader used by the catalog snapshot files
//! (`catalog::store::Writer`, `catalog::stats::Writer`).
//!
//! Before the dedup pass, both `store.rs` and `stats.rs` carried their own
//! local `Writer { bytes: Vec<u8> }` and `Reader { bytes: &[u8], cursor }`
//! types with identical bodies for `u8`/`u16`/`u32`/`u64`/`bool`/`bytes`.
//! The higher-level helpers — `value`, `opt_value`, `str`, `opt_str`,
//! `opt_box_str`, `opt_expr`, … — diverged because the two files encode
//! different on-disk schemas. Those diverging helpers stay in their
//! respective files; the primitives live here.
//!
//! The framing/checksum helpers also live here so both stores agree on
//! the 20-byte header layout (magic + version + reserved + payload_len +
//! crc32) and the trailer-free CRC computation.

use crc32fast::Hasher;

use crate::{Error, Result};

/// Append-only LE primitives writer over an owned `Vec<u8>`.
pub(super) struct BytesWriter {
    bytes: Vec<u8>,
}

impl BytesWriter {
    pub(super) fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
    }

    pub(super) fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(super) fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    pub(super) fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn f64(&mut self, value: f64) {
        self.u64(value.to_bits());
    }

    pub(super) fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }
}

/// Cursor-based LE primitives reader over a borrowed `&[u8]`.
pub(super) struct BytesReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> BytesReader<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    pub(super) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    /// Take the next `len` bytes as a borrowed slice. Returns
    /// `Error::CatalogCorrupt` on overflow / underflow so caller code
    /// uniformly surfaces a single error variant for catalog truncation.
    pub(super) fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(Error::CatalogCorrupt("catalog snapshot length overflow"))?;
        let bytes = self
            .bytes
            .get(self.pos..end)
            .ok_or(Error::CatalogCorrupt("catalog snapshot truncated"))?;
        self.pos = end;
        Ok(bytes)
    }

    pub(super) fn take_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = self.take(N)?;
        let mut out = [0_u8; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    pub(super) fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub(super) fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }

    pub(super) fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take_array()?))
    }

    pub(super) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take_array()?))
    }

    pub(super) fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take_array()?))
    }

    pub(super) fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.u64()?))
    }
}

/// Header byte layout for the framed snapshot files written by both
/// `CatalogStore` and `StatsStore`.
///
/// ```text
/// 0..4     magic u32 (LE)
/// 4..6     version u16 (LE)
/// 6..8     reserved u16
/// 8..16    payload_len u64 (LE)
/// 16..20   crc32 over payload u32 (LE)
/// 20..     payload bytes
/// ```
pub(super) const FILE_HEADER_LEN: usize = 20;

/// Build the framed `[header || payload]` bytes for a catalog snapshot
/// file. Both `CatalogStore::save_atomic` and `StatsStore::save` rely on
/// this exact framing so the two on-disk files share a single decode
/// path.
pub(super) fn frame_snapshot(magic: u32, version: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(FILE_HEADER_LEN + payload.len());
    out.extend_from_slice(&magic.to_le_bytes());
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    let crc = crc32_payload(payload);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// Decoded view over the framed header. Returned by [`parse_header`].
pub(super) struct FrameHeader<'a> {
    pub payload: &'a [u8],
}

/// Validate the 20-byte framed header and return the payload slice.
/// Caller supplies the `magic` it expects and an `invalid_version`
/// constructor for the version mismatch path so each store can keep
/// raising its own typed `Error` for version drift.
pub(super) fn parse_header<'a>(
    bytes: &'a [u8],
    magic: u32,
    file_too_small: Error,
    magic_mismatch: Error,
    length_overflow: Error,
    length_mismatch: Error,
    expected_version: u16,
    invalid_version: impl FnOnce(u16) -> Error,
) -> Result<FrameHeader<'a>> {
    if bytes.len() < FILE_HEADER_LEN {
        return Err(file_too_small);
    }
    let observed_magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    if observed_magic != magic {
        return Err(magic_mismatch);
    }
    let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
    if version != expected_version {
        return Err(invalid_version(version));
    }
    let payload_len = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
    let crc = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
    let expected = FILE_HEADER_LEN
        .checked_add(payload_len)
        .ok_or(length_overflow)?;
    if bytes.len() != expected {
        return Err(length_mismatch);
    }
    let payload = &bytes[FILE_HEADER_LEN..];
    if crc32_payload(payload) != crc {
        return Err(Error::InvalidChecksum);
    }
    Ok(FrameHeader { payload })
}

/// CRC32 over the snapshot payload (no zeroed-field convention here;
/// the checksum lives in the header, not inside the payload).
pub(super) fn crc32_payload(bytes: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    hasher.finalize()
}
