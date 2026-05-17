//! Normalisation helpers for the parity oracle.
//!
//! When the SQL has no top-level `ORDER BY`, rows are sorted
//! lexicographically by their string-formatted form. Floats compare with a
//! 1e-9 absolute-or-relative epsilon.

#![allow(dead_code)]

use std::cmp::Ordering;
use std::sync::Arc;

use redlinedb_sql::SqlValue;
use rusqlite::types::Value as RuValue;

pub(crate) const FLOAT_EPSILON: f64 = 1e-9;

/// Split a multi-statement script into (setup_batch, final_query). The
/// final statement is the one whose rows we compare; everything before it
/// is fired as setup via `execute_batch`.
pub fn split_script(sql: &str) -> (String, String) {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    let mut pieces: Vec<&str> = Vec::new();
    let mut start = 0usize;
    let bytes = trimmed.as_bytes();
    let mut in_str: Option<u8> = None;
    for i in 0..bytes.len() {
        match (in_str, bytes[i]) {
            (None, b';') => {
                let chunk = trimmed[start..i].trim();
                if !chunk.is_empty() {
                    pieces.push(chunk);
                }
                start = i + 1;
            }
            (None, q @ (b'\'' | b'"')) => in_str = Some(q),
            (Some(q), c) if c == q => in_str = None,
            _ => {}
        }
    }
    let tail = trimmed[start..].trim();
    if !tail.is_empty() {
        pieces.push(tail);
    }
    if pieces.is_empty() {
        return (String::new(), String::new());
    }
    let final_q = pieces.pop().unwrap().to_owned();
    let setup = pieces.join(";\n");
    let setup = if setup.is_empty() {
        String::new()
    } else {
        format!("{};", setup)
    };
    (setup, final_q)
}

/// Coerce a rusqlite Value into a `redlinedb_sql::SqlValue` so two row-sets
/// produced by different engines can compare apples-to-apples.
pub fn to_sql_value(value: RuValue) -> SqlValue {
    match value {
        RuValue::Null => SqlValue::Null,
        RuValue::Integer(i) => SqlValue::Integer(i),
        RuValue::Real(r) => SqlValue::Real(r),
        RuValue::Text(t) => SqlValue::Text(Arc::from(t)),
        RuValue::Blob(b) => SqlValue::Blob(Arc::from(b)),
    }
}

fn row_key(row: &[SqlValue]) -> String {
    let mut s = String::new();
    for v in row {
        s.push_str(&match v {
            SqlValue::Null => "NULL".to_owned(),
            SqlValue::Integer(i) => format!("I{}", i),
            SqlValue::Real(r) => format!("R{:.9}", r),
            SqlValue::Text(t) => format!("T{}", t),
            SqlValue::Blob(b) => format!("B{:?}", b),
        });
        s.push('|');
    }
    s
}

/// Sort rows lexicographically when the SQL has no top-level `ORDER BY`.
pub fn maybe_sort(sql: &str, rows: &mut Vec<Vec<SqlValue>>) {
    let upper = sql.to_ascii_uppercase();
    if !upper.contains("ORDER BY") {
        rows.sort_by(|a, b| row_key(a).cmp(&row_key(b)));
    }
}

/// Float-aware equality. NULLs compare equal to NULLs, integers and reals
/// cross-compare with epsilon, everything else falls through to standard
/// equality on the `SqlValue` variant.
pub fn values_eq(left: &SqlValue, right: &SqlValue) -> bool {
    match (left, right) {
        (SqlValue::Null, SqlValue::Null) => true,
        (SqlValue::Integer(a), SqlValue::Integer(b)) => a == b,
        (SqlValue::Real(a), SqlValue::Real(b)) => float_eq(*a, *b),
        (SqlValue::Integer(a), SqlValue::Real(b)) => float_eq(*a as f64, *b),
        (SqlValue::Real(a), SqlValue::Integer(b)) => float_eq(*a, *b as f64),
        (SqlValue::Text(a), SqlValue::Text(b)) => a.as_ref() == b.as_ref(),
        (SqlValue::Blob(a), SqlValue::Blob(b)) => a.as_ref() == b.as_ref(),
        _ => false,
    }
}

fn float_eq(a: f64, b: f64) -> bool {
    if a.is_nan() && b.is_nan() {
        return true;
    }
    (a - b).abs() <= FLOAT_EPSILON
        || (a != 0.0 && ((a - b) / a).abs() <= FLOAT_EPSILON)
}

pub fn rows_equal(left: &[Vec<SqlValue>], right: &[Vec<SqlValue>]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    for (l, r) in left.iter().zip(right.iter()) {
        if l.len() != r.len() {
            return false;
        }
        for (lv, rv) in l.iter().zip(r.iter()) {
            if !values_eq(lv, rv) {
                return false;
            }
        }
    }
    true
}

/// Compare the lexical sort key (string form) of two rows.
pub fn cmp_rows(a: &[SqlValue], b: &[SqlValue]) -> Ordering {
    row_key(a).cmp(&row_key(b))
}
