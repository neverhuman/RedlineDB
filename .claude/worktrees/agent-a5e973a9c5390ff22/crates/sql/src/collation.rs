//! Lane SQL-D phase 10: text collations (BINARY / NOCASE / RTRIM).
//!
//! SQLite supports three built-in collations. We model them here so the
//! executor can apply a collation when an explicit `COLLATE` clause appears
//! or a column-level collation is declared. Comparisons fall back to the
//! standard byte-wise compare in the absence of any collation.

use std::cmp::Ordering;

use crate::value::SqlValue;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum Collation {
    #[default]
    Binary,
    NoCase,
    RTrim,
}

impl Collation {
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_uppercase().as_str() {
            "BINARY" | "" => Some(Self::Binary),
            "NOCASE" => Some(Self::NoCase),
            "RTRIM" => Some(Self::RTrim),
            _ => None,
        }
    }

    pub fn compare_text(self, a: &str, b: &str) -> Ordering {
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
        }
    }

    pub fn compare_values(self, left: &SqlValue, right: &SqlValue) -> Option<Ordering> {
        match (left, right) {
            (SqlValue::Text(a), SqlValue::Text(b)) => Some(self.compare_text(a, b)),
            _ => None,
        }
    }
}
