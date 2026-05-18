use std::cmp::Ordering;

use super::value::ValueRef;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SortDir {
    Asc,
    Desc,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum NullOrder {
    First,
    Last,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedIndexKey {
    pub bytes: Vec<u8>,
    pub contains_null: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexKeyDef {
    pub ordinal: u16,
    pub source: IndexKeySource,
    pub sort_dir: SortDir,
    pub null_order: NullOrder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexKeySource {
    Column { attnum: u16 },
}

pub fn encode_index_key(
    parts: &[ValueRef<'_>],
    dirs: &[SortDir],
    out: &mut Vec<u8>,
) -> EncodedIndexKey {
    debug_assert_eq!(parts.len(), dirs.len());
    out.clear();
    let mut contains_null = false;
    for (&value, &dir) in parts.iter().zip(dirs.iter()) {
        let start = out.len();
        contains_null |= matches!(value, ValueRef::Null);
        encode_part(value, out);
        if dir == SortDir::Desc {
            for byte in &mut out[start..] {
                *byte = !*byte;
            }
        }
        out.push(0xff);
    }
    EncodedIndexKey {
        bytes: out.clone(),
        contains_null,
    }
}

pub fn compare_index_keys(left: &[u8], right: &[u8]) -> Ordering {
    left.cmp(right)
}

fn encode_part(value: ValueRef<'_>, out: &mut Vec<u8>) {
    match value {
        ValueRef::Null => out.push(0x00),
        ValueRef::Integer(v) => {
            out.push(0x10);
            let sortable = (v as u64) ^ 0x8000_0000_0000_0000;
            out.extend_from_slice(&sortable.to_be_bytes());
        }
        ValueRef::Real(v) => {
            out.push(0x20);
            let bits = if v == 0.0 { 0.0 } else { v }.to_bits();
            let sortable = if bits & 0x8000_0000_0000_0000 != 0 {
                !bits
            } else {
                bits ^ 0x8000_0000_0000_0000
            };
            out.extend_from_slice(&sortable.to_be_bytes());
        }
        ValueRef::Text(v) => {
            out.push(0x30);
            encode_bytes(v.as_bytes(), out);
        }
        ValueRef::Blob(v) => {
            out.push(0x40);
            encode_bytes(v, out);
        }
    }
}

fn encode_bytes(bytes: &[u8], out: &mut Vec<u8>) {
    for &byte in bytes {
        if byte == 0 {
            out.push(0);
            out.push(0xff);
        } else {
            out.push(byte);
        }
    }
    out.push(0);
    out.push(0);
}
