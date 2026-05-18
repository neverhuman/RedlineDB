use std::sync::Arc;

use super::value::OwnedValue;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum Affinity {
    Blob = 0,
    Text = 1,
    Numeric = 2,
    Integer = 3,
    Real = 4,
}

impl Affinity {
    pub fn from_u8(value: u8) -> Result<Self, CoerceError> {
        match value {
            0 => Ok(Self::Blob),
            1 => Ok(Self::Text),
            2 => Ok(Self::Numeric),
            3 => Ok(Self::Integer),
            4 => Ok(Self::Real),
            _ => Err(CoerceError::DatatypeMismatch),
        }
    }
}

pub fn derive_affinity(declared_type: Option<&str>) -> Affinity {
    let Some(declared_type) = declared_type else {
        return Affinity::Blob;
    };
    let declared_type = declared_type.trim();
    if declared_type.is_empty() {
        return Affinity::Blob;
    }
    if contains_ascii_insensitive(declared_type, "INT") {
        return Affinity::Integer;
    }
    if contains_ascii_insensitive(declared_type, "CHAR")
        || contains_ascii_insensitive(declared_type, "CLOB")
        || contains_ascii_insensitive(declared_type, "TEXT")
    {
        return Affinity::Text;
    }
    if contains_ascii_insensitive(declared_type, "BLOB") {
        return Affinity::Blob;
    }
    if contains_ascii_insensitive(declared_type, "REAL")
        || contains_ascii_insensitive(declared_type, "FLOA")
        || contains_ascii_insensitive(declared_type, "DOUB")
    {
        return Affinity::Real;
    }
    Affinity::Numeric
}

pub fn apply_affinity(value: OwnedValue, affinity: Affinity) -> Result<OwnedValue, CoerceError> {
    use OwnedValue::*;
    Ok(match affinity {
        Affinity::Blob => value,
        Affinity::Text => match value {
            Null | Text(_) | Blob(_) => value,
            Integer(v) => Text(Arc::from(v.to_string())),
            Real(v) => Text(Arc::from(format_float(v))),
        },
        Affinity::Numeric | Affinity::Integer => match value {
            Text(s) => parse_numeric_text(&s).unwrap_or(Text(s)),
            Real(v) if real_is_exact_i64(v) => Integer(v as i64),
            other => other,
        },
        Affinity::Real => match value {
            Integer(v) => Real(v as f64),
            Text(s) => match parse_numeric_text(&s) {
                Some(Integer(i)) => Real(i as f64),
                Some(Real(r)) => Real(r),
                Some(other) => other,
                None => Text(s),
            },
            other => other,
        },
    })
}

#[derive(Debug, thiserror::Error)]
pub enum CoerceError {
    #[error("datatype mismatch")]
    DatatypeMismatch,
}

fn contains_ascii_insensitive(haystack: &str, needle: &str) -> bool {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() || n.len() > h.len() {
        return false;
    }
    h.windows(n.len()).any(|w| {
        w.iter()
            .zip(n.iter())
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
    })
}

fn parse_numeric_text(s: &str) -> Option<OwnedValue> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(v) = trimmed.parse::<i64>() {
        return Some(OwnedValue::Integer(v));
    }
    if let Ok(v) = trimmed.parse::<f64>() {
        if v.is_finite() && real_is_exact_i64(v) {
            return Some(OwnedValue::Integer(v as i64));
        }
        return Some(OwnedValue::Real(v));
    }
    None
}

fn real_is_exact_i64(v: f64) -> bool {
    v.is_finite() && v >= i64::MIN as f64 && v <= i64::MAX as f64 && (v as i64) as f64 == v
}

fn format_float(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{:.1}", v)
    } else {
        v.to_string()
    }
}
