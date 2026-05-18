//! JSONB encoder: `serde_json::Value` -> Vec<u8>.
//!
//! The encoder is deterministic — encoding the same logical JSON value twice
//! yields identical bytes. Object keys are emitted in their map iteration
//! order; we rely on `serde_json` having `preserve_order` semantics or a
//! deterministic BTreeMap-backed `Map` so the output is stable for canonical
//! JSON inputs.

use serde_json::Value;

use super::wire::{FORMAT_VERSION, MAGIC, tag, write_varint, zigzag_encode};

/// Encode a `serde_json::Value` into RedlineDB JSONB bytes.
pub fn encode(value: &Value) -> Vec<u8> {
    let mut out = Vec::with_capacity(estimate_size(value) + 2);
    out.push(MAGIC);
    out.push(FORMAT_VERSION);
    encode_node(value, &mut out);
    out
}

/// Encode just the node bytes (no preamble) starting at the current end of
/// `out`. Used internally for nested composites.
pub(crate) fn encode_node(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Null => out.push(tag::NULL),
        Value::Bool(true) => out.push(tag::TRUE),
        Value::Bool(false) => out.push(tag::FALSE),
        Value::Number(n) => encode_number(n, out),
        Value::String(s) => encode_text(s.as_bytes(), out),
        Value::Array(items) => encode_array(items, out),
        Value::Object(map) => encode_object(map, out),
    }
}

fn encode_number(n: &serde_json::Number, out: &mut Vec<u8>) {
    if let Some(i) = n.as_i64() {
        out.push(tag::INTEGER);
        write_varint(out, zigzag_encode(i));
        return;
    }
    if let Some(u) = n.as_u64() {
        // Values above i64::MAX are stored as REAL — JSON can't lose precision
        // round-tripping in that range anyway, and our INTEGER slot is i64.
        let f = u as f64;
        out.push(tag::REAL);
        out.extend_from_slice(&f.to_le_bytes());
        return;
    }
    let f = n.as_f64().unwrap_or(0.0);
    out.push(tag::REAL);
    out.extend_from_slice(&f.to_le_bytes());
}

pub(crate) fn encode_text(bytes: &[u8], out: &mut Vec<u8>) {
    out.push(tag::TEXT);
    write_varint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

/// Encode a raw BLOB node. JSONB has no native binary type; this is the
/// extension hook used by callers that store opaque bytes alongside JSON
/// (e.g. the SQL `JSONB(blob)` constructor).
pub fn encode_blob(bytes: &[u8], out: &mut Vec<u8>) {
    out.push(tag::BLOB);
    write_varint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

/// Encode a top-level BLOB document (preamble + BLOB node).
pub fn encode_blob_value(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + 1 + 5 + bytes.len());
    out.push(MAGIC);
    out.push(FORMAT_VERSION);
    encode_blob(bytes, &mut out);
    out
}

fn encode_array(items: &[Value], out: &mut Vec<u8>) {
    // Encode children into a scratch buffer first so we know the body length.
    let mut body = Vec::with_capacity(items.len() * 4);
    for item in items {
        encode_node(item, &mut body);
    }
    out.push(tag::ARRAY);
    write_varint(out, items.len() as u64);
    write_varint(out, body.len() as u64);
    out.extend_from_slice(&body);
}

fn encode_object(map: &serde_json::Map<String, Value>, out: &mut Vec<u8>) {
    let mut body = Vec::with_capacity(map.len() * 8);
    for (key, value) in map {
        encode_text(key.as_bytes(), &mut body);
        encode_node(value, &mut body);
    }
    out.push(tag::OBJECT);
    write_varint(out, map.len() as u64);
    write_varint(out, body.len() as u64);
    out.extend_from_slice(&body);
}

fn estimate_size(value: &Value) -> usize {
    match value {
        Value::Null | Value::Bool(_) => 1,
        Value::Number(_) => 9,
        Value::String(s) => 1 + 5 + s.len(),
        Value::Array(items) => 1 + 5 + 5 + items.iter().map(estimate_size).sum::<usize>(),
        Value::Object(map) => {
            1 + 5
                + 5
                + map
                    .iter()
                    .map(|(k, v)| 1 + 5 + k.len() + estimate_size(v))
                    .sum::<usize>()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn preamble_is_present() {
        let bytes = encode(&Value::Null);
        assert_eq!(bytes[0], MAGIC);
        assert_eq!(bytes[1], FORMAT_VERSION);
    }

    #[test]
    fn small_int_is_compact() {
        let bytes = encode(&json!(0));
        // preamble (2) + tag (1) + varint zigzag(0) (1) = 4
        assert_eq!(bytes.len(), 4);
    }

    #[test]
    fn deterministic_encoding() {
        let v = json!({"a": 1, "b": [true, null, "x"]});
        let a = encode(&v);
        let b = encode(&v);
        assert_eq!(a, b);
    }
}
