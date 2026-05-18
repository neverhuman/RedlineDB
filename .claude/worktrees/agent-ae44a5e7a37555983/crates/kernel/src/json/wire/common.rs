use crate::{Error, Result};

/// Magic byte identifying RedlineDB JSONB.
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

#[inline]
pub fn zigzag_encode(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

#[inline]
pub fn zigzag_decode(value: u64) -> i64 {
    ((value >> 1) as i64) ^ -((value & 1) as i64)
}

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

#[inline]
pub fn read_tag(buf: &[u8], offset: usize) -> Result<u8> {
    let byte = *buf
        .get(offset)
        .ok_or(Error::InvalidJsonb("tag truncated"))?;
    Ok(byte & tag::TYPE_MASK)
}

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
