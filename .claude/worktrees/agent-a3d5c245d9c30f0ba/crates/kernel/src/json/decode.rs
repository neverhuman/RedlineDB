//! JSONB decoder: bytes -> `serde_json::Value`.
//!
//! The decoder is defensive: any malformed input results in
//! [`Error::InvalidJsonb`] and never panics. Round-trip is guaranteed for
//! every value the encoder emits.

use serde_json::{Map, Value};

use super::wire::{
    ArrayIter, NodeKind, ObjectIter, parse_preamble, read_bytes_payload, read_tag, read_varint,
    tag, zigzag_decode,
};
use crate::{Error, Result};

/// Decode a JSONB byte slice into a `serde_json::Value`.
pub fn decode(bytes: &[u8]) -> Result<Value> {
    let body_off = parse_preamble(bytes)?;
    let (value, consumed) = decode_node(bytes, body_off)?;
    if body_off + consumed != bytes.len() {
        return Err(Error::InvalidJsonb("trailing bytes after root"));
    }
    Ok(value)
}

pub(crate) fn decode_node(buf: &[u8], offset: usize) -> Result<(Value, usize)> {
    let t = read_tag(buf, offset)?;
    match t {
        tag::NULL => Ok((Value::Null, 1)),
        tag::TRUE => Ok((Value::Bool(true), 1)),
        tag::FALSE => Ok((Value::Bool(false), 1)),
        tag::INTEGER => {
            let (raw, n) = read_varint(buf, offset + 1)?;
            let int = zigzag_decode(raw);
            Ok((Value::Number(int.into()), 1 + n))
        }
        tag::REAL => {
            if buf.len() < offset + 9 {
                return Err(Error::InvalidJsonb("real truncated"));
            }
            let mut arr = [0_u8; 8];
            arr.copy_from_slice(&buf[offset + 1..offset + 9]);
            let f = f64::from_le_bytes(arr);
            // serde_json::Number cannot represent NaN/Inf — fall back to Null
            // so the round-trip path never panics. Encoders should avoid
            // emitting non-finite REALs for canonical JSON.
            let value = serde_json::Number::from_f64(f)
                .map(Value::Number)
                .unwrap_or(Value::Null);
            Ok((value, 9))
        }
        tag::TEXT => {
            let (bytes, total) = read_bytes_payload(buf, offset)?;
            let s = std::str::from_utf8(bytes)
                .map_err(|_| Error::InvalidJsonb("text not utf-8"))?
                .to_owned();
            Ok((Value::String(s), total))
        }
        tag::BLOB => {
            // BLOBs aren't first-class JSON values; surface them as base16
            // strings prefixed with "0x" so they survive a JSON round-trip.
            let (bytes, total) = read_bytes_payload(buf, offset)?;
            let mut hex = String::with_capacity(2 + bytes.len() * 2);
            hex.push_str("0x");
            for b in bytes {
                use std::fmt::Write as _;
                let _ = write!(&mut hex, "{:02x}", b);
            }
            Ok((Value::String(hex), total))
        }
        tag::ARRAY => decode_array(buf, offset),
        tag::OBJECT => decode_object(buf, offset),
        _ => Err(Error::InvalidJsonb("unknown tag")),
    }
}

fn decode_array(buf: &[u8], offset: usize) -> Result<(Value, usize)> {
    let iter = ArrayIter::new(buf, offset)?;
    let mut total_node_len: usize = 0;
    let mut items = Vec::with_capacity(iter.child_count() as usize);
    let count = iter.child_count() as usize;
    let mut start = None;
    let mut last_end = None;
    for entry in iter {
        let (value_off, value_total) = entry?;
        if start.is_none() {
            start = Some(value_off);
        }
        last_end = Some(value_off + value_total);
        let (val, _) = decode_node(buf, value_off)?;
        items.push(val);
    }
    if items.len() != count {
        return Err(Error::InvalidJsonb("array count mismatch"));
    }
    // Compute total = (header) + (body span). Recompute via tag header.
    total_node_len += 1; // tag
    let (_, n_count) = read_varint(buf, offset + 1)?;
    let (body_len, n_body) = read_varint(buf, offset + 1 + n_count)?;
    total_node_len += n_count + n_body + body_len as usize;
    // Sanity: ensure the body span we walked matches the declared body length.
    if let (Some(s), Some(e)) = (start, last_end) {
        let walked = e - s;
        if walked as u64 != body_len {
            return Err(Error::InvalidJsonb("array body length mismatch"));
        }
    } else if body_len != 0 {
        return Err(Error::InvalidJsonb("array body length mismatch"));
    }
    Ok((Value::Array(items), total_node_len))
}

fn decode_object(buf: &[u8], offset: usize) -> Result<(Value, usize)> {
    let iter = ObjectIter::new(buf, offset)?;
    let count = iter.child_count() as usize;
    let mut map = Map::with_capacity(count);
    let mut start = None;
    let mut last_end = None;
    for entry in iter {
        let (key_bytes, value_off, value_total) = entry?;
        if start.is_none() {
            // start of body = start of first key; we need its offset which
            // sits just before the key node, but the iterator already
            // returned key bytes — recompute via offsets we have:
            // value_off = key_node_end, so key_node_start = value_off -
            // (1 + varint(len) + len). We don't need to track it precisely
            // for the body span check; we use last_end - first value_off + key.
            start = Some(value_off);
        }
        last_end = Some(value_off + value_total);
        let key = std::str::from_utf8(key_bytes)
            .map_err(|_| Error::InvalidJsonb("object key not utf-8"))?
            .to_owned();
        let (val, _) = decode_node(buf, value_off)?;
        map.insert(key, val);
    }
    if map.len() != count {
        // duplicate key collapse → mismatch
        return Err(Error::InvalidJsonb("object count mismatch"));
    }
    let mut total_node_len: usize = 1;
    let (_, n_count) = read_varint(buf, offset + 1)?;
    let (body_len, n_body) = read_varint(buf, offset + 1 + n_count)?;
    total_node_len += n_count + n_body + body_len as usize;
    let _ = (start, last_end); // body length already validated by ObjectIter consuming exactly body_len
    Ok((Value::Object(map), total_node_len))
}

/// Identify the node kind at the root of `bytes` without fully decoding it.
pub fn root_kind(bytes: &[u8]) -> Result<NodeKind> {
    let off = parse_preamble(bytes)?;
    let t = read_tag(bytes, off)?;
    let kind = match t {
        tag::NULL => NodeKind::Null,
        tag::TRUE => NodeKind::Bool(true),
        tag::FALSE => NodeKind::Bool(false),
        tag::INTEGER => NodeKind::Integer,
        tag::REAL => NodeKind::Real,
        tag::TEXT => NodeKind::Text,
        tag::ARRAY => NodeKind::Array,
        tag::OBJECT => NodeKind::Object,
        tag::BLOB => NodeKind::Blob,
        _ => return Err(Error::InvalidJsonb("unknown tag")),
    };
    Ok(kind)
}

#[cfg(test)]
mod tests {
    use super::super::encode::encode;
    use super::*;
    use serde_json::json;

    fn roundtrip(v: Value) {
        let bytes = encode(&v);
        let back = decode(&bytes).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn primitives_round_trip() {
        roundtrip(Value::Null);
        roundtrip(json!(true));
        roundtrip(json!(false));
        roundtrip(json!(0));
        roundtrip(json!(42));
        roundtrip(json!(-1));
        roundtrip(json!(i64::MAX));
        roundtrip(json!(i64::MIN));
        roundtrip(json!(""));
        roundtrip(json!("hello"));
    }

    #[test]
    fn nested_round_trip() {
        roundtrip(json!({"a": [1, 2, {"b": null, "c": true}], "d": "x"}));
    }

    #[test]
    fn malformed_returns_err() {
        assert!(decode(&[]).is_err());
        assert!(decode(&[0x00, 0x01]).is_err());
        assert!(decode(&[super::super::wire::MAGIC, 0xFF]).is_err());
    }
}
