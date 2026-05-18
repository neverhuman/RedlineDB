//! RedlineDB JSONB wire format.
//!
//! # Layout
//!
//! Every encoded JSONB value begins with a 2-byte preamble followed by the
//! root node. The preamble lets readers quickly reject foreign formats and
//! makes future format revisions explicit.
//!
//! ```text
//! +---------+-----------+---------+
//! | magic   | version   | node ...|
//! | 1 byte  | 1 byte    |         |
//! +---------+-----------+---------+
//! ```
//!
//! - `magic` is [`MAGIC`] (`0xRD`); any other byte yields
//!   [`Error::InvalidJsonb`].
//! - `version` is [`FORMAT_VERSION`]. Older readers refuse newer versions.
//!
//! # Node tags
//!
//! Each node begins with a single tag byte. Only the low nibble is used for
//! the type discriminator today; the high nibble is reserved for future
//! length-class hinting and MUST be zero on encode. Decoders MUST mask it off
//! and ignore the high nibble for forward compatibility.
//!
//! | tag   | type    | body                                                 |
//! |-------|---------|------------------------------------------------------|
//! | 0x00  | NULL    | (none)                                               |
//! | 0x01  | TRUE    | (none)                                               |
//! | 0x02  | FALSE   | (none)                                               |
//! | 0x03  | INTEGER | LEB128 zig-zag encoded `i64`                         |
//! | 0x04  | REAL    | 8-byte IEEE-754 little-endian `f64`                  |
//! | 0x05  | TEXT    | LEB128 byte-length, then UTF-8 bytes                 |
//! | 0x06  | ARRAY   | LEB128 child count, LEB128 body length, N nodes      |
//! | 0x07  | OBJECT  | LEB128 child count, LEB128 body length, N (k,v) pairs|
//! | 0x08  | BLOB    | LEB128 byte-length, then raw bytes                   |
//!
//! Object keys are themselves TEXT nodes (with their own 0x05 tag) so they
//! can share decoding paths with regular strings. The "body length" inside
//! arrays and objects covers everything after the body-length varint itself
//! up through the last child, which lets readers skip an entire composite
//! node in O(1).

use crate::{Error, Result};

/// Magic byte identifying RedlineDB JSONB ("RD" → 0xRD nibble + 'd' is 0x64; we
/// pick 0xRD = 0x52 + 0x44 → 0x96 to be a single byte unique to RedlineDB).
pub const MAGIC: u8 = 0x96;

/// Format version. Bump on incompatible changes.
pub const FORMAT_VERSION: u8 = 1;

/// Length of the wire preamble (`magic` + `version`).
pub const PREAMBLE_LEN: usize = 2;

/// Type tag values.
pub mod tag {
    pub const NULL: u8 = 0x00;
    pub const TRUE: u8 = 0x01;
    pub const FALSE: u8 = 0x02;
    pub const INTEGER: u8 = 0x03;
    pub const REAL: u8 = 0x04;
    pub const TEXT: u8 = 0x05;
    pub const ARRAY: u8 = 0x06;
    pub const OBJECT: u8 = 0x07;
    pub const BLOB: u8 = 0x08;

    /// Mask isolating the type discriminator nibble.
    pub const TYPE_MASK: u8 = 0x0F;
}

/// Logical kind of a JSONB node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Null,
    Bool(bool),
    Integer,
    Real,
    Text,
    Array,
    Object,
    Blob,
}

/// Maximum encoded length of an unsigned LEB128 `u64`.
pub const MAX_VARINT_LEN: usize = 10;

#[inline]
pub fn write_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

#[inline]
pub fn read_varint(buf: &[u8], offset: usize) -> Result<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;
    let mut consumed: usize = 0;
    loop {
        if consumed >= MAX_VARINT_LEN {
            return Err(Error::InvalidJsonb("varint overflow"));
        }
        let idx = offset
            .checked_add(consumed)
            .ok_or(Error::InvalidJsonb("varint offset overflow"))?;
        let byte = *buf
            .get(idx)
            .ok_or(Error::InvalidJsonb("varint truncated"))?;
        consumed += 1;
        let chunk = (byte & 0x7F) as u64;
        // `shift` can reach 63 on the last legal byte; chunks beyond that must be 0.
        if shift >= 64 && chunk != 0 {
            return Err(Error::InvalidJsonb("varint overflow"));
        }
        if shift < 64 {
            result |= chunk << shift;
        }
        if byte & 0x80 == 0 {
            return Ok((result, consumed));
        }
        shift += 7;
    }
}

/// Zig-zag encode a signed `i64` into the unsigned LEB128 representation.
#[inline]
pub fn zigzag_encode(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

/// Zig-zag decode an unsigned LEB128 payload back into a signed `i64`.
#[inline]
pub fn zigzag_decode(value: u64) -> i64 {
    ((value >> 1) as i64) ^ -((value & 1) as i64)
}

/// Validate a JSONB preamble and return the offset of the first node byte.
pub fn parse_preamble(buf: &[u8]) -> Result<usize> {
    if buf.len() < PREAMBLE_LEN {
        return Err(Error::InvalidJsonb("preamble truncated"));
    }
    if buf[0] != MAGIC {
        return Err(Error::InvalidJsonb("bad magic"));
    }
    if buf[1] != FORMAT_VERSION {
        return Err(Error::InvalidJsonb("unsupported version"));
    }
    Ok(PREAMBLE_LEN)
}

/// Read a single tag byte at `offset`, masking off reserved bits.
#[inline]
pub fn read_tag(buf: &[u8], offset: usize) -> Result<u8> {
    let byte = *buf
        .get(offset)
        .ok_or(Error::InvalidJsonb("tag truncated"))?;
    Ok(byte & tag::TYPE_MASK)
}

/// Returns the byte span covering the node starting at `offset` (excluding
/// the tag) — that is, `(node_total_len_with_tag, node_kind)`.
pub fn node_span(buf: &[u8], offset: usize) -> Result<(usize, NodeKind)> {
    let t = read_tag(buf, offset)?;
    let body_start = offset + 1;
    match t {
        tag::NULL => Ok((1, NodeKind::Null)),
        tag::TRUE => Ok((1, NodeKind::Bool(true))),
        tag::FALSE => Ok((1, NodeKind::Bool(false))),
        tag::INTEGER => {
            let (_, n) = read_varint(buf, body_start)?;
            Ok((1 + n, NodeKind::Integer))
        }
        tag::REAL => {
            if buf.len() < body_start + 8 {
                return Err(Error::InvalidJsonb("real truncated"));
            }
            Ok((1 + 8, NodeKind::Real))
        }
        tag::TEXT | tag::BLOB => {
            let (len, n) = read_varint(buf, body_start)?;
            let len = usize::try_from(len)
                .map_err(|_| Error::InvalidJsonb("text/blob length overflow"))?;
            let total = 1 + n + len;
            if buf.len() < offset + total {
                return Err(Error::InvalidJsonb("text/blob truncated"));
            }
            let kind = if t == tag::TEXT {
                NodeKind::Text
            } else {
                NodeKind::Blob
            };
            Ok((total, kind))
        }
        tag::ARRAY | tag::OBJECT => {
            let (_, n_count) = read_varint(buf, body_start)?;
            let (body_len, n_body_len) = read_varint(buf, body_start + n_count)?;
            let body_len = usize::try_from(body_len)
                .map_err(|_| Error::InvalidJsonb("composite length overflow"))?;
            let total = 1 + n_count + n_body_len + body_len;
            if buf.len() < offset + total {
                return Err(Error::InvalidJsonb("composite truncated"));
            }
            let kind = if t == tag::ARRAY {
                NodeKind::Array
            } else {
                NodeKind::Object
            };
            Ok((total, kind))
        }
        _ => Err(Error::InvalidJsonb("unknown tag")),
    }
}

/// Read the LEB128 length and return the slice of bytes for a TEXT or BLOB
/// node beginning at `offset`. The caller must already know the tag is TEXT
/// or BLOB.
pub fn read_bytes_payload(buf: &[u8], offset: usize) -> Result<(&[u8], usize)> {
    let (len, n) = read_varint(buf, offset + 1)?;
    let len = usize::try_from(len).map_err(|_| Error::InvalidJsonb("text/blob length overflow"))?;
    let start = offset + 1 + n;
    let end = start
        .checked_add(len)
        .ok_or(Error::InvalidJsonb("text/blob length overflow"))?;
    if end > buf.len() {
        return Err(Error::InvalidJsonb("text/blob truncated"));
    }
    Ok((&buf[start..end], end - offset))
}

/// Borrowed view over an OBJECT node's body. Iterates `(key_bytes, value_offset)`
/// pairs lazily without allocating.
pub struct ObjectIter<'a> {
    buf: &'a [u8],
    cursor: usize,
    end: usize,
    remaining: u64,
}

impl<'a> ObjectIter<'a> {
    pub fn new(buf: &'a [u8], offset: usize) -> Result<Self> {
        let t = read_tag(buf, offset)?;
        if t != tag::OBJECT {
            return Err(Error::InvalidJsonb("expected object"));
        }
        let (count, n_count) = read_varint(buf, offset + 1)?;
        let (body_len, n_body) = read_varint(buf, offset + 1 + n_count)?;
        let body_start = offset + 1 + n_count + n_body;
        let body_len = usize::try_from(body_len)
            .map_err(|_| Error::InvalidJsonb("composite length overflow"))?;
        let end = body_start
            .checked_add(body_len)
            .ok_or(Error::InvalidJsonb("composite length overflow"))?;
        if end > buf.len() {
            return Err(Error::InvalidJsonb("composite truncated"));
        }
        Ok(Self {
            buf,
            cursor: body_start,
            end,
            remaining: count,
        })
    }

    pub fn child_count(&self) -> u64 {
        self.remaining
    }
}

impl<'a> Iterator for ObjectIter<'a> {
    /// `(key_bytes, value_offset, value_total_len)`
    type Item = Result<(&'a [u8], usize, usize)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        if self.cursor >= self.end {
            return Some(Err(Error::InvalidJsonb("object body underflow")));
        }
        // Key MUST be TEXT.
        let key_tag = match read_tag(self.buf, self.cursor) {
            Ok(t) => t,
            Err(e) => return Some(Err(e)),
        };
        if key_tag != tag::TEXT {
            return Some(Err(Error::InvalidJsonb("object key not text")));
        }
        let (key_bytes, key_total_len) = match read_bytes_payload(self.buf, self.cursor) {
            Ok(x) => x,
            Err(e) => return Some(Err(e)),
        };
        let value_off = self.cursor + key_total_len;
        let (value_total, _) = match node_span(self.buf, value_off) {
            Ok(x) => x,
            Err(e) => return Some(Err(e)),
        };
        self.cursor = value_off + value_total;
        self.remaining -= 1;
        Some(Ok((key_bytes, value_off, value_total)))
    }
}

/// Borrowed view over an ARRAY node's body. Iterates child offsets.
pub struct ArrayIter<'a> {
    buf: &'a [u8],
    cursor: usize,
    end: usize,
    remaining: u64,
}

impl<'a> ArrayIter<'a> {
    pub fn new(buf: &'a [u8], offset: usize) -> Result<Self> {
        let t = read_tag(buf, offset)?;
        if t != tag::ARRAY {
            return Err(Error::InvalidJsonb("expected array"));
        }
        let (count, n_count) = read_varint(buf, offset + 1)?;
        let (body_len, n_body) = read_varint(buf, offset + 1 + n_count)?;
        let body_start = offset + 1 + n_count + n_body;
        let body_len = usize::try_from(body_len)
            .map_err(|_| Error::InvalidJsonb("composite length overflow"))?;
        let end = body_start
            .checked_add(body_len)
            .ok_or(Error::InvalidJsonb("composite length overflow"))?;
        if end > buf.len() {
            return Err(Error::InvalidJsonb("composite truncated"));
        }
        Ok(Self {
            buf,
            cursor: body_start,
            end,
            remaining: count,
        })
    }

    pub fn child_count(&self) -> u64 {
        self.remaining
    }
}

impl<'a> Iterator for ArrayIter<'a> {
    /// `(value_offset, value_total_len)`
    type Item = Result<(usize, usize)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        if self.cursor >= self.end {
            return Some(Err(Error::InvalidJsonb("array body underflow")));
        }
        let off = self.cursor;
        let (total, _) = match node_span(self.buf, off) {
            Ok(x) => x,
            Err(e) => return Some(Err(e)),
        };
        self.cursor = off + total;
        self.remaining -= 1;
        Some(Ok((off, total)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_round_trip_small() {
        for v in [0_u64, 1, 127, 128, 255, 16_383, 16_384, u64::MAX] {
            let mut buf = Vec::new();
            write_varint(&mut buf, v);
            let (decoded, len) = read_varint(&buf, 0).unwrap();
            assert_eq!(decoded, v);
            assert_eq!(len, buf.len());
        }
    }

    #[test]
    fn varint_truncated_returns_err() {
        let buf = [0x80_u8]; // continuation but no follow-up
        assert!(read_varint(&buf, 0).is_err());
    }

    #[test]
    fn varint_overlong_returns_err() {
        let buf = [0x80_u8; 11];
        assert!(read_varint(&buf, 0).is_err());
    }

    #[test]
    fn zigzag_round_trips_extremes() {
        for v in [0_i64, 1, -1, i64::MAX, i64::MIN, 1234567, -7654321] {
            assert_eq!(zigzag_decode(zigzag_encode(v)), v);
        }
    }

    #[test]
    fn parse_preamble_rejects_garbage() {
        assert!(parse_preamble(&[]).is_err());
        assert!(parse_preamble(&[0x00, 0x01]).is_err());
        assert!(parse_preamble(&[MAGIC, 0xFF]).is_err());
        let off = parse_preamble(&[MAGIC, FORMAT_VERSION]).unwrap();
        assert_eq!(off, PREAMBLE_LEN);
    }
}
