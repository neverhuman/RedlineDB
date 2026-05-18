use super::value::ValueRef;

const RECORD_V1: u8 = 0x01;
const ST_NULL: u64 = 0;
const ST_I8: u64 = 1;
const ST_I16: u64 = 2;
const ST_I32: u64 = 3;
const ST_I64: u64 = 4;
const ST_F64: u64 = 5;
const ST_VAR_BASE: u64 = 16;

#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    #[error("record is empty")]
    Empty,
    #[error("unknown record format {0}")]
    UnknownFormat(u8),
    #[error("truncated record")]
    Truncated,
    #[error("invalid utf8")]
    InvalidUtf8,
    #[error("invalid serial type {0}")]
    InvalidSerial(u64),
    #[error("column out of bounds")]
    ColumnOutOfBounds,
}

#[derive(Debug, Default)]
pub struct RecordScratch {
    serials: Vec<u64>,
    offsets: Vec<usize>,
}

#[derive(Debug, Copy, Clone)]
pub struct RecordRef<'a> {
    bytes: &'a [u8],
}

impl<'a> RecordRef<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, RecordError> {
        if bytes.is_empty() {
            return Err(RecordError::Empty);
        }
        if bytes[0] != RECORD_V1 {
            return Err(RecordError::UnknownFormat(bytes[0]));
        }
        Ok(Self { bytes })
    }

    pub fn column_count(&self) -> Result<usize, RecordError> {
        let mut pos = 1;
        Ok(read_varint(self.bytes, &mut pos)? as usize)
    }

    pub fn decode_into(&self, scratch: &mut RecordScratch) -> Result<(), RecordError> {
        scratch.serials.clear();
        scratch.offsets.clear();
        let mut pos = 1;
        let ncols = read_varint(self.bytes, &mut pos)? as usize;
        scratch.serials.reserve(ncols);
        scratch.offsets.reserve(ncols);
        for _ in 0..ncols {
            scratch.serials.push(read_varint(self.bytes, &mut pos)?);
        }
        let mut payload = pos;
        for &st in &scratch.serials {
            scratch.offsets.push(payload);
            payload = payload
                .checked_add(payload_len(st)?)
                .ok_or(RecordError::Truncated)?;
            if payload > self.bytes.len() {
                return Err(RecordError::Truncated);
            }
        }
        if payload != self.bytes.len() {
            return Err(RecordError::Truncated);
        }
        Ok(())
    }

    pub fn value_at(
        &self,
        scratch: &RecordScratch,
        ordinal: usize,
    ) -> Result<ValueRef<'a>, RecordError> {
        let st = *scratch
            .serials
            .get(ordinal)
            .ok_or(RecordError::ColumnOutOfBounds)?;
        let off = *scratch
            .offsets
            .get(ordinal)
            .ok_or(RecordError::ColumnOutOfBounds)?;
        Ok(match st {
            ST_NULL => ValueRef::Null,
            ST_I8 => ValueRef::Integer(read_exact(self.bytes, off, 1)?[0] as i8 as i64),
            ST_I16 => {
                let mut buf = [0u8; 2];
                buf.copy_from_slice(read_exact(self.bytes, off, 2)?);
                ValueRef::Integer(i16::from_le_bytes(buf) as i64)
            }
            ST_I32 => {
                let mut buf = [0u8; 4];
                buf.copy_from_slice(read_exact(self.bytes, off, 4)?);
                ValueRef::Integer(i32::from_le_bytes(buf) as i64)
            }
            ST_I64 => {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(read_exact(self.bytes, off, 8)?);
                ValueRef::Integer(i64::from_le_bytes(buf))
            }
            ST_F64 => {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(read_exact(self.bytes, off, 8)?);
                ValueRef::Real(f64::from_le_bytes(buf))
            }
            s if s >= ST_VAR_BASE => {
                let len = payload_len(s)?;
                let payload = read_exact(self.bytes, off, len)?;
                if is_text_serial(s) {
                    let text =
                        std::str::from_utf8(payload).map_err(|_| RecordError::InvalidUtf8)?;
                    ValueRef::Text(text)
                } else {
                    ValueRef::Blob(payload)
                }
            }
            other => return Err(RecordError::InvalidSerial(other)),
        })
    }
}

pub fn encode_record(values: &[ValueRef<'_>], out: &mut Vec<u8>) -> Result<(), RecordError> {
    out.clear();
    out.push(RECORD_V1);
    put_varint(values.len() as u64, out);
    for &value in values {
        put_varint(serial_type(value), out);
    }
    for &value in values {
        match value {
            ValueRef::Null => {}
            ValueRef::Integer(v) => encode_integer(v, out),
            ValueRef::Real(v) => out.extend_from_slice(&canonical_f64(v).to_le_bytes()),
            ValueRef::Text(v) => out.extend_from_slice(v.as_bytes()),
            ValueRef::Blob(v) => out.extend_from_slice(v),
        }
    }
    Ok(())
}

fn serial_type(value: ValueRef<'_>) -> u64 {
    match value {
        ValueRef::Null => ST_NULL,
        ValueRef::Integer(v) if i8::try_from(v).is_ok() => ST_I8,
        ValueRef::Integer(v) if i16::try_from(v).is_ok() => ST_I16,
        ValueRef::Integer(v) if i32::try_from(v).is_ok() => ST_I32,
        ValueRef::Integer(_) => ST_I64,
        ValueRef::Real(_) => ST_F64,
        ValueRef::Blob(v) => ST_VAR_BASE + (v.len() as u64) * 2,
        ValueRef::Text(v) => ST_VAR_BASE + (v.len() as u64) * 2 + 1,
    }
}

fn encode_integer(v: i64, out: &mut Vec<u8>) {
    if let Ok(v) = i8::try_from(v) {
        out.push(v as u8);
    } else if let Ok(v) = i16::try_from(v) {
        out.extend_from_slice(&v.to_le_bytes());
    } else if let Ok(v) = i32::try_from(v) {
        out.extend_from_slice(&v.to_le_bytes());
    } else {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

fn canonical_f64(v: f64) -> f64 {
    if v == 0.0 {
        0.0
    } else if v.is_nan() {
        f64::NAN
    } else {
        v
    }
}

fn payload_len(serial: u64) -> Result<usize, RecordError> {
    match serial {
        ST_NULL => Ok(0),
        ST_I8 => Ok(1),
        ST_I16 => Ok(2),
        ST_I32 => Ok(4),
        ST_I64 | ST_F64 => Ok(8),
        s if s >= ST_VAR_BASE => Ok(((s - ST_VAR_BASE) / 2) as usize),
        other => Err(RecordError::InvalidSerial(other)),
    }
}

fn is_text_serial(serial: u64) -> bool {
    serial >= ST_VAR_BASE && ((serial - ST_VAR_BASE) & 1) == 1
}

fn read_exact(bytes: &[u8], off: usize, len: usize) -> Result<&[u8], RecordError> {
    bytes
        .get(off..off.saturating_add(len))
        .ok_or(RecordError::Truncated)
}

fn put_varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn read_varint(bytes: &[u8], pos: &mut usize) -> Result<u64, RecordError> {
    let mut out = 0u64;
    let mut shift = 0u32;
    loop {
        let byte = *bytes.get(*pos).ok_or(RecordError::Truncated)?;
        *pos += 1;
        out |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(out);
        }
        shift += 7;
        if shift >= 64 {
            return Err(RecordError::Truncated);
        }
    }
}
