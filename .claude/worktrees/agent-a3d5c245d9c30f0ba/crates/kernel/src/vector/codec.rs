//! On-disk wire format for vector values.
//!
//! Layout:
//!
//! ```text
//!   +-------------------+--------------+----------------------+
//!   | varint dim (≤ 4B) | element kind | dim * sizeof(elem)   |
//!   +-------------------+--------------+----------------------+
//! ```
//!
//! * `dim` is encoded as an unsigned LEB128 varint (1 byte for ≤ 127 lanes,
//!   ≤ 4 bytes for the entire `u32` range).
//! * `element kind` is a single discriminant byte. Today only `F32` is
//!   implemented; `F16`/`I8` are reserved so future quantisation does not
//!   require a wire-format break.
//! * `payload` is `dim * element_size` raw bytes in **little endian**.
//!   Component endianness is fixed to LE for cross-platform byte-equality of
//!   stored vectors regardless of the host architecture.
//!
//! Lane V2 (HNSW) and Lane V3 (DiskANN) build on this codec — keep it stable.

use super::{VectorError, VectorResult};

/// Element kind discriminant. Only `F32` is implemented today.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum ElementKind {
    F32 = 0x01,
    /// Reserved — half-precision floats. Reading bytes tagged `F16` returns
    /// `Err` until Lane V1.5 lands the converters.
    F16 = 0x02,
    /// Reserved — int8 quantised vectors with per-vector scale.
    I8 = 0x03,
}

impl ElementKind {
    fn from_byte(b: u8) -> VectorResult<Self> {
        match b {
            0x01 => Ok(Self::F32),
            0x02 => Ok(Self::F16),
            0x03 => Ok(Self::I8),
            other => Err(VectorError::Decode(format!(
                "unknown vector element kind 0x{other:02x}"
            ))),
        }
    }
}

/// Encode an `f32` slice using the wire format above.
pub fn encode_vector(values: &[f32]) -> Vec<u8> {
    let dim = values.len() as u32;
    let mut out = Vec::with_capacity(8 + values.len() * 4);
    write_varint(&mut out, dim);
    out.push(ElementKind::F32 as u8);
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Decode the wire format back into an owned `Vec<f32>`. Returns `Err` on any
/// malformation (truncation, unknown element kind, dimension/length mismatch).
pub fn decode_vector(bytes: &[u8]) -> VectorResult<Vec<f32>> {
    let (dim, rest) = read_varint(bytes)?;
    if rest.is_empty() {
        return Err(VectorError::Decode(
            "truncated: missing element kind byte".to_owned(),
        ));
    }
    let kind = ElementKind::from_byte(rest[0])?;
    let payload = &rest[1..];
    match kind {
        ElementKind::F32 => {
            let expected = (dim as usize) * 4;
            if payload.len() != expected {
                return Err(VectorError::Decode(format!(
                    "f32 payload length {} does not match dim {} (expected {} bytes)",
                    payload.len(),
                    dim,
                    expected
                )));
            }
            let mut out = Vec::with_capacity(dim as usize);
            for chunk in payload.chunks_exact(4) {
                let arr = [chunk[0], chunk[1], chunk[2], chunk[3]];
                out.push(f32::from_le_bytes(arr));
            }
            Ok(out)
        }
        ElementKind::F16 | ElementKind::I8 => Err(VectorError::Decode(format!(
            "element kind {:?} not implemented (reserved)",
            kind
        ))),
    }
}

fn write_varint(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn read_varint(bytes: &[u8]) -> VectorResult<(u32, &[u8])> {
    let mut value: u32 = 0;
    let mut shift: u32 = 0;
    let mut consumed = 0;
    for (i, &b) in bytes.iter().enumerate() {
        if i >= 5 {
            return Err(VectorError::Decode("varint too long".to_owned()));
        }
        value |= ((b & 0x7F) as u32) << shift;
        consumed = i + 1;
        if b & 0x80 == 0 {
            return Ok((value, &bytes[consumed..]));
        }
        shift += 7;
    }
    let _ = consumed;
    Err(VectorError::Decode("truncated varint".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_small() {
        let v = vec![1.0_f32, -2.5, 3.25, 4.0];
        let bytes = encode_vector(&v);
        let decoded = decode_vector(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn round_trip_large_dim() {
        // Exceed 127 to exercise the multi-byte varint path.
        let v: Vec<f32> = (0..513).map(|i| i as f32 * 0.5).collect();
        let bytes = encode_vector(&v);
        // 513 needs two varint bytes (0x81, 0x04).
        assert_eq!(bytes[0], 0x81);
        assert_eq!(bytes[1], 0x04);
        let decoded = decode_vector(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn truncated_payload_errors() {
        let v = vec![1.0_f32, 2.0, 3.0];
        let mut bytes = encode_vector(&v);
        bytes.truncate(bytes.len() - 1);
        assert!(decode_vector(&bytes).is_err());
    }

    #[test]
    fn unknown_kind_errors() {
        let mut bytes = encode_vector(&[1.0_f32]);
        // Replace element kind byte (right after 1-byte varint) with 0xFF.
        bytes[1] = 0xFF;
        assert!(decode_vector(&bytes).is_err());
    }

    #[test]
    fn empty_input_errors() {
        assert!(decode_vector(&[]).is_err());
    }

    #[test]
    fn reserved_kinds_error_until_implemented() {
        let mut bytes = encode_vector(&[1.0_f32]);
        bytes[1] = ElementKind::F16 as u8;
        assert!(decode_vector(&bytes).is_err());
        bytes[1] = ElementKind::I8 as u8;
        assert!(decode_vector(&bytes).is_err());
    }
}
