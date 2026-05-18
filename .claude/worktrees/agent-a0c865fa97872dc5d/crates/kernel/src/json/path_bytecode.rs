//! JSON path compiler + interpreter.
//!
//! Path syntax (subset compatible with SQLite's `json_extract`):
//!
//! ```text
//! path        := '$' segment*
//! segment     := '.' identifier
//!              | '[' integer ']'
//!              | '.' '"' quoted_key '"'
//! identifier  := [A-Za-z_][A-Za-z0-9_]*
//! quoted_key  := any chars except '"' (no escape processing yet)
//! integer     := unsigned decimal
//! ```
//!
//! The compiled bytecode is a `Vec<Op>` consumed linearly; keys reference a
//! string literal table so the interpreter can lend them as `&[u8]` for SIMD
//! comparisons without re-parsing.

use std::sync::Arc;

use super::simd_key::{key_eq, should_use_simd};
use super::wire::{ArrayIter, NodeKind, ObjectIter, node_span, parse_preamble, read_tag, tag};
use crate::{Error, Result};

/// Bytecode opcodes produced by [`compile_path`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// Begin at the document root. Always the first instruction.
    Root,
    /// Descend into the value associated with the literal key.
    LoadObjKey(u32),
    /// Descend into the array element at the given zero-based index.
    LoadArrIdx(u32),
    /// End of program; cursor's current node is the result.
    Return,
}

/// Output of `compile_path`. Cheaply clonable via `Arc`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledPath {
    pub ops: Vec<Op>,
    pub literals: Vec<String>,
}

impl CompiledPath {
    pub fn key_literal(&self, idx: u32) -> Option<&str> {
        self.literals.get(idx as usize).map(String::as_str)
    }
}

/// Compile a path string into bytecode.
pub fn compile_path(path: &str) -> Result<Arc<CompiledPath>> {
    let bytes = path.as_bytes();
    if bytes.first() != Some(&b'$') {
        return Err(Error::InvalidJsonPath("path must start with '$'"));
    }

    let mut ops = vec![Op::Root];
    let mut literals: Vec<String> = Vec::new();
    let mut i = 1_usize;
    while i < bytes.len() {
        match bytes[i] {
            b'.' => {
                i += 1;
                if i >= bytes.len() {
                    return Err(Error::InvalidJsonPath("trailing dot"));
                }
                if bytes[i] == b'"' {
                    // Quoted key — read until next '"' (no escapes for now).
                    i += 1;
                    let start = i;
                    while i < bytes.len() && bytes[i] != b'"' {
                        i += 1;
                    }
                    if i >= bytes.len() {
                        return Err(Error::InvalidJsonPath("unterminated quoted key"));
                    }
                    let key = std::str::from_utf8(&bytes[start..i])
                        .map_err(|_| Error::InvalidJsonPath("non-utf8 quoted key"))?
                        .to_owned();
                    i += 1; // consume closing quote
                    let id = intern_literal(&mut literals, key);
                    ops.push(Op::LoadObjKey(id));
                } else {
                    // Bare identifier.
                    let start = i;
                    while i < bytes.len() && is_ident(bytes[i]) {
                        i += 1;
                    }
                    if start == i {
                        return Err(Error::InvalidJsonPath("expected identifier after '.'"));
                    }
                    let key = std::str::from_utf8(&bytes[start..i])
                        .map_err(|_| Error::InvalidJsonPath("non-utf8 identifier"))?
                        .to_owned();
                    let id = intern_literal(&mut literals, key);
                    ops.push(Op::LoadObjKey(id));
                }
            }
            b'[' => {
                i += 1;
                let start = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if start == i {
                    return Err(Error::InvalidJsonPath("expected array index"));
                }
                let idx_str = std::str::from_utf8(&bytes[start..i])
                    .map_err(|_| Error::InvalidJsonPath("non-utf8 index"))?;
                let idx: u32 = idx_str
                    .parse()
                    .map_err(|_| Error::InvalidJsonPath("array index out of range"))?;
                if i >= bytes.len() || bytes[i] != b']' {
                    return Err(Error::InvalidJsonPath("missing ']'"));
                }
                i += 1;
                ops.push(Op::LoadArrIdx(idx));
            }
            _ => return Err(Error::InvalidJsonPath("unexpected character")),
        }
    }
    ops.push(Op::Return);
    Ok(Arc::new(CompiledPath { ops, literals }))
}

fn intern_literal(literals: &mut Vec<String>, key: String) -> u32 {
    for (i, existing) in literals.iter().enumerate() {
        if *existing == key {
            return i as u32;
        }
    }
    let id = literals.len() as u32;
    literals.push(key);
    id
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Evaluate a compiled path against JSONB bytes.
///
/// On a successful match returns a slice covering the matched node (including
/// its tag byte) so callers can re-decode or reinterpret it. Returns
/// `Ok(None)` when the path doesn't resolve (missing key, out-of-range
/// index, type mismatch).
pub fn path_eval<'a>(jsonb: &'a [u8], compiled: &CompiledPath) -> Result<Option<&'a [u8]>> {
    let body_off = parse_preamble(jsonb)?;
    let mut cursor = body_off;
    for op in &compiled.ops {
        match op {
            Op::Root => {
                // Cursor already at root.
            }
            Op::LoadObjKey(id) => {
                let key = compiled
                    .key_literal(*id)
                    .ok_or(Error::InvalidJsonPath("missing key literal"))?;
                let t = read_tag(jsonb, cursor)?;
                if t != tag::OBJECT {
                    return Ok(None);
                }
                match find_object_value(jsonb, cursor, key.as_bytes())? {
                    Some(off) => cursor = off,
                    None => return Ok(None),
                }
            }
            Op::LoadArrIdx(idx) => {
                let t = read_tag(jsonb, cursor)?;
                if t != tag::ARRAY {
                    return Ok(None);
                }
                match find_array_value(jsonb, cursor, *idx)? {
                    Some(off) => cursor = off,
                    None => return Ok(None),
                }
            }
            Op::Return => {
                let (total, _) = node_span(jsonb, cursor)?;
                return Ok(Some(&jsonb[cursor..cursor + total]));
            }
        }
    }
    // Implicit return if last op wasn't Return (shouldn't happen for compiled paths).
    let (total, _) = node_span(jsonb, cursor)?;
    Ok(Some(&jsonb[cursor..cursor + total]))
}

/// Locate the value offset for `key` inside the OBJECT at `obj_off`.
fn find_object_value(jsonb: &[u8], obj_off: usize, key: &[u8]) -> Result<Option<usize>> {
    let iter = ObjectIter::new(jsonb, obj_off)?;
    let count = iter.child_count() as usize;
    let use_simd = should_use_simd(count, key.len());
    if use_simd {
        for entry in iter {
            let (k_bytes, val_off, _) = entry?;
            if key_eq(k_bytes, key) {
                return Ok(Some(val_off));
            }
        }
    } else {
        for entry in iter {
            let (k_bytes, val_off, _) = entry?;
            if k_bytes == key {
                return Ok(Some(val_off));
            }
        }
    }
    Ok(None)
}

fn find_array_value(jsonb: &[u8], arr_off: usize, idx: u32) -> Result<Option<usize>> {
    let iter = ArrayIter::new(jsonb, arr_off)?;
    for (i, entry) in iter.enumerate() {
        let (val_off, _) = entry?;
        if i as u64 == idx as u64 {
            return Ok(Some(val_off));
        }
    }
    Ok(None)
}

/// Helper: returns the [`NodeKind`] of the matched value, if any, without
/// allocating.
pub fn path_kind(jsonb: &[u8], compiled: &CompiledPath) -> Result<Option<NodeKind>> {
    match path_eval(jsonb, compiled)? {
        None => Ok(None),
        Some(slice) => {
            let (_, kind) = node_span(slice, 0)?;
            Ok(Some(kind))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::encode::encode;
    use super::*;
    use serde_json::json;

    #[test]
    fn compile_root_only() {
        let p = compile_path("$").unwrap();
        assert_eq!(p.ops, vec![Op::Root, Op::Return]);
    }

    #[test]
    fn compile_dot_and_index() {
        let p = compile_path("$.users[2].name").unwrap();
        assert_eq!(p.literals, vec!["users".to_string(), "name".to_string()]);
        assert_eq!(
            p.ops,
            vec![
                Op::Root,
                Op::LoadObjKey(0),
                Op::LoadArrIdx(2),
                Op::LoadObjKey(1),
                Op::Return,
            ]
        );
    }

    #[test]
    fn compile_quoted_key() {
        let p = compile_path("$.\"a b\"").unwrap();
        assert_eq!(p.literals, vec!["a b".to_string()]);
        assert_eq!(p.ops, vec![Op::Root, Op::LoadObjKey(0), Op::Return]);
    }

    #[test]
    fn compile_rejects_garbage() {
        assert!(compile_path("").is_err());
        assert!(compile_path("foo").is_err());
        assert!(compile_path("$.").is_err());
        assert!(compile_path("$[").is_err());
        assert!(compile_path("$[abc]").is_err());
        assert!(compile_path("$.\"abc").is_err());
    }

    #[test]
    fn eval_happy_path() {
        let v = json!({"users": [{"name": "ada"}, {"name": "lin"}, {"name": "yo"}]});
        let bytes = encode(&v);
        let p = compile_path("$.users[2].name").unwrap();
        let slice = path_eval(&bytes, &p).unwrap().unwrap();
        // The slice should be a TEXT node encoding "yo".
        assert_eq!(slice[0] & tag::TYPE_MASK, tag::TEXT);
    }

    #[test]
    fn eval_missing_returns_none() {
        let v = json!({"a": 1});
        let bytes = encode(&v);
        let p = compile_path("$.b").unwrap();
        assert!(path_eval(&bytes, &p).unwrap().is_none());
    }

    #[test]
    fn eval_array_oor_returns_none() {
        let v = json!([1, 2]);
        let bytes = encode(&v);
        let p = compile_path("$[5]").unwrap();
        assert!(path_eval(&bytes, &p).unwrap().is_none());
    }
}
