use crate::{Error, Result};

use super::common::{node_span, read_bytes_payload, read_tag, read_varint, tag};

/// Borrowed view over an OBJECT node's body. Iterates `(key_bytes, value_offset)`
/// pairs lazily without allocating.
pub struct ObjectIter<'a> {
    buf: &'a [u8],
    cursor: usize,
    end: usize,
    remaining: u64,
}

impl<'a> ObjectIter<'a> {
    // dedup-allowed: same-name-different-type — `ObjectIter::new` and
    // `ArrayIter::new` share a header-decode skeleton but validate
    // different tags (OBJECT vs ARRAY) and yield different `Item`
    // shapes. Folding them into one generic over the tag would require
    // a phantom-tagged Iter that obscures the wire-format contract.
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
