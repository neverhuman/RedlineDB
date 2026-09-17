use std::cmp::Ordering;
use std::sync::Arc;

#[allow(unused_imports)]
pub use redlinedb_kernel::catalog::{
    Affinity, EvalScratch, ExprAst, OwnedValue, RecordRef, RecordScratch, StorageClass, ValueRef,
    apply_affinity, derive_affinity, encode_record,
};

pub type SqlValue = OwnedValue;
pub type SqlValueRef<'a> = ValueRef<'a>;

pub fn compare_values(left: &SqlValue, right: &SqlValue) -> Ordering {
    use OwnedValue::*;
    match (left, right) {
        (Null, Null) => Ordering::Equal,
        (Null, _) => Ordering::Less,
        (_, Null) => Ordering::Greater,
        (Integer(a), Integer(b)) => a.cmp(b),
        (Real(a), Real(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
        (Integer(a), Real(b)) => (*a as f64).partial_cmp(b).unwrap_or(Ordering::Equal),
        (Real(a), Integer(b)) => a.partial_cmp(&(*b as f64)).unwrap_or(Ordering::Equal),
        (Integer(_) | Real(_), Text(_) | Blob(_)) => Ordering::Less,
        (Text(_) | Blob(_), Integer(_) | Real(_)) => Ordering::Greater,
        (Text(a), Text(b)) => a.as_ref().cmp(b.as_ref()),
        (Blob(a), Blob(b)) => a.as_ref().cmp(b.as_ref()),
        (Text(_), Blob(_)) => Ordering::Less,
        (Blob(_), Text(_)) => Ordering::Greater,
    }
}

pub fn is_truthy(value: &SqlValue) -> bool {
    match value {
        OwnedValue::Null => false,
        OwnedValue::Integer(v) => *v != 0,
        OwnedValue::Real(v) => *v != 0.0,
        OwnedValue::Text(v) => sqlite_truthy_str(v.as_ref()),
        // A33: avoid the `String::from_utf8_lossy` allocation. The
        // previous code allocated a fresh String for every Blob truthy
        // check that contained non-UTF8 bytes. `sqlite_truthy_str` only
        // checks for parseable i64 / f64 patterns, neither of which can
        // include replacement chars (U+FFFD) — so any non-UTF8 byte
        // sequence in the blob must yield `false` anyway. Short-circuit
        // it directly; for valid-UTF8 blobs we borrow the slice without
        // allocating.
        OwnedValue::Blob(v) => match std::str::from_utf8(v) {
            Ok(s) => sqlite_truthy_str(s),
            Err(_) => false,
        },
    }
}

fn sqlite_truthy_str(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    if let Ok(v) = trimmed.parse::<i64>() {
        return v != 0;
    }
    if let Ok(v) = trimmed.parse::<f64>() {
        return v != 0.0;
    }
    false
}

pub fn canonicalize(value: SqlValue) -> SqlValue {
    match value {
        OwnedValue::Real(0.0) => OwnedValue::Real(0.0),
        OwnedValue::Real(v) if v.is_nan() => OwnedValue::Real(f64::NAN),
        other => other,
    }
}

#[allow(dead_code)]
pub fn text_value(value: impl Into<Arc<str>>) -> SqlValue {
    OwnedValue::Text(value.into())
}
