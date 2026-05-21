//! Lane SQL-D phase 10: text collations (BINARY / NOCASE / RTRIM).
//!
//! SQLite supports three built-in collations. We model them here so the
//! executor can apply a collation when an explicit `COLLATE` clause appears
//! or a column-level collation is declared. Comparisons fall back to the
//! standard byte-wise compare in the absence of any collation.
//!
//! Custom collations registered through `sqlite3_create_collation*` flow
//! through the `Custom { name }` variant and dispatch via
//! `crate::udf::call_registered_collation` at compare time.

use std::cmp::Ordering;

use crate::value::SqlValue;

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub enum Collation {
    #[default]
    Binary,
    NoCase,
    RTrim,
    Uint,
    /// Externally registered collation looked up by name through the FFI
    /// collation registry at compare time.
    Custom(String),
}

impl Collation {
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_uppercase().as_str() {
            "BINARY" | "" => Some(Self::Binary),
            "NOCASE" => Some(Self::NoCase),
            "RTRIM" => Some(Self::RTrim),
            "UINT" => Some(Self::Uint),
            other => Some(Self::Custom(other.to_owned())),
        }
    }

    pub fn compare_text(&self, a: &str, b: &str) -> Ordering {
        match self {
            Self::Binary => a.cmp(b),
            Self::NoCase => {
                let mut ai = a.bytes();
                let mut bi = b.bytes();
                loop {
                    match (ai.next(), bi.next()) {
                        (Some(x), Some(y)) => {
                            let xa = x.to_ascii_lowercase();
                            let ya = y.to_ascii_lowercase();
                            match xa.cmp(&ya) {
                                Ordering::Equal => continue,
                                non_eq => return non_eq,
                            }
                        }
                        (None, None) => return Ordering::Equal,
                        (None, _) => return Ordering::Less,
                        (_, None) => return Ordering::Greater,
                    }
                }
            }
            Self::RTrim => a.trim_end_matches(' ').cmp(b.trim_end_matches(' ')),
            Self::Uint => compare_uint_text(a, b),
            Self::Custom(name) => {
                match crate::udf::call_registered_collation(crate::udf::current_db(), name, a, b) {
                    Some(ord) => ord,
                    None => a.cmp(b),
                }
            }
        }
    }

    pub fn compare_values(&self, left: &SqlValue, right: &SqlValue) -> Option<Ordering> {
        match (left, right) {
            (SqlValue::Text(a), SqlValue::Text(b)) => Some(self.compare_text(a, b)),
            _ => None,
        }
    }
}

fn compare_uint_text(a: &str, b: &str) -> Ordering {
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(da), Some(db)) if da.is_ascii_digit() && db.is_ascii_digit() => {
                let a_num = take_uint(&mut ai);
                let b_num = take_uint(&mut bi);
                match a_num.cmp(&b_num) {
                    Ordering::Equal => continue,
                    other => return other,
                }
            }
            (Some(_), Some(_)) => {
                let ca = ai.next().unwrap();
                let cb = bi.next().unwrap();
                match ca.cmp(&cb) {
                    Ordering::Equal => continue,
                    other => return other,
                }
            }
        }
    }
}

fn take_uint(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> (usize, usize) {
    let mut value: usize = 0;
    let mut digits = 0usize;
    while let Some(ch) = chars.peek().copied() {
        if !ch.is_ascii_digit() {
            break;
        }
        value = value
            .saturating_mul(10)
            .saturating_add(ch.to_digit(10).unwrap_or(0) as usize);
        digits += 1;
        chars.next();
    }
    (value, digits)
}
