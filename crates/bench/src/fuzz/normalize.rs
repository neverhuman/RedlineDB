//! Row-set normalization and error-class taxonomy for fuzz parity diff.
//!
//! Two engines can legitimately disagree on the literal byte representation
//! of the same logical outcome (e.g. row order without ORDER BY, REAL
//! precision, error wording). Anything `compare_outcomes` returns as
//! `Match` is a verified parity hit. Anything it returns as `Divergence` is
//! a real divergence the harness pretty-prints and fails on.
//!
//! Float comparison uses a relative epsilon (1e-9). That tolerance is
//! generous enough to absorb single-rounding differences and tight enough
//! to catch algorithmic divergence (off-by-one accumulator order, etc.).

/// A single SQL value as observed from either engine. Carries enough type
/// information that two `NULL`s compare equal and a `Real(1.0)` does NOT
/// compare equal to `Integer(1)` (SQLite's affinity rules treat them as
/// equivalent in some operators but the engines must agree at this layer
/// or we lose signal).
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl Cell {
    /// Float-aware equality. Two `Real` cells compare equal when both are
    /// NaN OR when `|a-b| <= 1e-9 * max(|a|, |b|, 1)`. Everything else
    /// uses `PartialEq`.
    fn approx_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Cell::Real(a), Cell::Real(b)) => {
                if a.is_nan() && b.is_nan() {
                    return true;
                }
                let tolerance = 1e-9_f64 * a.abs().max(b.abs()).max(1.0);
                (a - b).abs() <= tolerance
            }
            _ => self == other,
        }
    }
}

/// Outcome of executing a generated statement on one engine. Either rows
/// (for SELECTs) OR an error (lexer/parser/exec failure). The harness
/// classifies the error so we don't false-positive on "SQLite says
/// 'no such column' and RedlineDB says 'unknown column'".
#[derive(Clone, Debug)]
pub enum Outcome {
    Rows(Vec<Vec<Cell>>),
    /// Statement returned no rows (DML or DDL that succeeded).
    Done,
    /// Statement was rejected. `class` is the canonicalized error class;
    /// `raw` is the engine's own message for debug printing.
    Error {
        class: ErrorClass,
        raw: String,
    },
}

/// Canonical buckets covering every error a fresh app might see. Two
/// engines that produce errors in the same bucket are considered to agree
/// for parity-gate purposes. Cross-bucket disagreement is a divergence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorClass {
    /// SQL syntax / parse error (unbalanced parens, bad keyword, etc.).
    Syntax,
    /// Schema-level error: missing table/column/index reference.
    NoSuchObject,
    /// Constraint violation (PK, UNIQUE, NOT NULL, CHECK, FK).
    Constraint,
    /// Type mismatch / coercion failure (e.g. text to integer).
    Mismatch,
    /// Anything else (busy, IO, range, etc.). Two engines reporting
    /// `Other` are considered equivalent — we do not subdivide further
    /// because cross-engine wording differs aggressively here.
    Other,
}

/// Classify a free-form error message into an `ErrorClass`. The
/// substring matches below are deliberately broad so both `rusqlite`'s
/// SQLite-derived wording and RedlineDB's Rust-derived wording land in
/// the same bucket whenever they describe the same logical condition.
pub fn classify(msg: &str) -> ErrorClass {
    let lower = msg.to_ascii_lowercase();
    if lower.contains("syntax")
        || lower.contains("incomplete")
        || lower.contains("near \"")
        || lower.contains("parse error")
        || lower.contains("unrecognized")
        || lower.contains("unexpected token")
    {
        ErrorClass::Syntax
    } else if lower.contains("no such table")
        || lower.contains("no such column")
        || lower.contains("no such index")
        || lower.contains("unknown table")
        || lower.contains("unknown column")
        || lower.contains("not found")
    {
        ErrorClass::NoSuchObject
    } else if lower.contains("unique")
        || lower.contains("primary key")
        || lower.contains("not null")
        || lower.contains("foreign key")
        || lower.contains("check constraint")
        || lower.contains("constraint")
    {
        ErrorClass::Constraint
    } else if lower.contains("mismatch")
        || lower.contains("type")
        || lower.contains("cannot convert")
    {
        ErrorClass::Mismatch
    } else {
        ErrorClass::Other
    }
}

/// Normalize a row-set for cross-engine comparison. When `ordered` is
/// false (no ORDER BY in the source SQL), rows are sorted lexicographically
/// by their cell representation so engines that legitimately return rows
/// in a different physical order still compare equal. When `ordered` is
/// true, the row order is preserved.
fn normalize_rows(mut rows: Vec<Vec<Cell>>, ordered: bool) -> Vec<Vec<Cell>> {
    if !ordered {
        rows.sort_by(|a, b| {
            let a_key: Vec<String> = a.iter().map(cell_sort_key).collect();
            let b_key: Vec<String> = b.iter().map(cell_sort_key).collect();
            a_key.cmp(&b_key)
        });
    }
    rows
}

fn cell_sort_key(cell: &Cell) -> String {
    // Format every cell the same way regardless of its variant so the
    // sort comparator works across heterogeneous rows. Using a string
    // key avoids implementing PartialOrd on Cell, which would have to
    // pick an arbitrary ordering across Integer/Real/Text/Blob/Null.
    match cell {
        Cell::Null => "0:null".to_string(),
        Cell::Integer(v) => format!("1:{v:020}"),
        Cell::Real(v) => format!("2:{v:020.10}"),
        Cell::Text(s) => format!("3:{s}"),
        Cell::Blob(b) => format!("4:{}", hex(b)),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Structured divergence with both engines' outputs. The caller pretty-
/// prints this when an iteration fails the parity gate.
#[derive(Clone, Debug)]
pub struct Divergence {
    pub reason: &'static str,
    pub sqlite: Outcome,
    pub redline: Outcome,
}

/// Compare two outcomes. `ordered` toggles row-order significance.
pub fn compare_outcomes(
    sqlite: Outcome,
    redline: Outcome,
    ordered: bool,
) -> Result<(), Divergence> {
    match (&sqlite, &redline) {
        (Outcome::Done, Outcome::Done) => Ok(()),
        (Outcome::Rows(a_rows), Outcome::Rows(b_rows)) => {
            let a = normalize_rows(a_rows.clone(), ordered);
            let b = normalize_rows(b_rows.clone(), ordered);
            if a.len() != b.len() {
                return Err(Divergence {
                    reason: "row count differs",
                    sqlite,
                    redline,
                });
            }
            for (ra, rb) in a.iter().zip(b.iter()) {
                if ra.len() != rb.len() {
                    return Err(Divergence {
                        reason: "row width differs",
                        sqlite,
                        redline,
                    });
                }
                for (ca, cb) in ra.iter().zip(rb.iter()) {
                    if !ca.approx_eq(cb) {
                        return Err(Divergence {
                            reason: "cell value differs",
                            sqlite,
                            redline,
                        });
                    }
                }
            }
            Ok(())
        }
        (Outcome::Error { class: a, .. }, Outcome::Error { class: b, .. }) => {
            if a == b {
                Ok(())
            } else {
                Err(Divergence {
                    reason: "error class differs",
                    sqlite,
                    redline,
                })
            }
        }
        // Done-vs-Rows is OK only when the Rows side is empty (some
        // engines surface an empty result-set for DML pass-through).
        (Outcome::Done, Outcome::Rows(rows)) | (Outcome::Rows(rows), Outcome::Done)
            if rows.is_empty() =>
        {
            Ok(())
        }
        _ => Err(Divergence {
            reason: "outcome kind differs",
            sqlite,
            redline,
        }),
    }
}

/// Heuristic: was the generated SQL written with an explicit ORDER BY?
/// Conservative — matches the case-insensitive substring " order by ".
pub fn is_ordered(sql: &str) -> bool {
    let lower = format!(" {} ", sql.to_ascii_lowercase());
    lower.contains(" order by ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approx_eq_treats_close_floats_as_equal() {
        let a = Cell::Real(1.0);
        let b = Cell::Real(1.0 + 1e-12);
        assert!(a.approx_eq(&b));
    }

    #[test]
    fn approx_eq_rejects_large_float_drift() {
        let a = Cell::Real(1.0);
        let b = Cell::Real(2.0);
        assert!(!a.approx_eq(&b));
    }

    #[test]
    fn unordered_rows_compare_equal_after_sort() {
        let s = Outcome::Rows(vec![
            vec![Cell::Integer(1)],
            vec![Cell::Integer(2)],
            vec![Cell::Integer(3)],
        ]);
        let r = Outcome::Rows(vec![
            vec![Cell::Integer(3)],
            vec![Cell::Integer(1)],
            vec![Cell::Integer(2)],
        ]);
        assert!(compare_outcomes(s, r, false).is_ok());
    }

    #[test]
    fn ordered_mismatch_diverges() {
        let s = Outcome::Rows(vec![vec![Cell::Integer(1)], vec![Cell::Integer(2)]]);
        let r = Outcome::Rows(vec![vec![Cell::Integer(2)], vec![Cell::Integer(1)]]);
        assert!(compare_outcomes(s, r, true).is_err());
    }

    #[test]
    fn classify_syntax() {
        assert_eq!(classify("syntax error near \"WHERE\""), ErrorClass::Syntax);
    }

    #[test]
    fn classify_no_such_object() {
        assert_eq!(classify("no such table: foo"), ErrorClass::NoSuchObject);
        assert_eq!(classify("Unknown column 'x'"), ErrorClass::NoSuchObject);
    }

    #[test]
    fn is_ordered_detects_clause() {
        assert!(is_ordered("SELECT 1 FROM t ORDER BY a"));
        assert!(is_ordered("select 1 from t order by a"));
        assert!(!is_ordered("SELECT 1 FROM t"));
    }
}
