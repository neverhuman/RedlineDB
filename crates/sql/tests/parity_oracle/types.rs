//! Result and error-class types used by the parity oracle harness.
//!
//! Kept tiny so each engine runner (`run::run_oracle`, `run::run_redline`)
//! depends on one self-contained vocabulary file.

#![allow(dead_code)]

use redlinedb_sql::SqlValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    SyntaxError,
    NoSuchTable,
    ConstraintViolation,
    TypeError,
    Generic,
}

#[derive(Debug, Clone)]
pub struct OracleResult {
    pub rows: Vec<Vec<SqlValue>>,
    pub err_class: Option<ErrorClass>,
    pub raw_err: Option<String>,
}

impl OracleResult {
    pub fn ok(rows: Vec<Vec<SqlValue>>) -> Self {
        Self {
            rows,
            err_class: None,
            raw_err: None,
        }
    }

    pub fn err(class: ErrorClass, raw: String) -> Self {
        Self {
            rows: Vec::new(),
            err_class: Some(class),
            raw_err: Some(raw),
        }
    }
}

/// Classify a free-form error message into one of the five [`ErrorClass`]
/// values. Used for both engines so the oracle gate can compare semantics
/// without depending on identical error strings.
pub fn classify_err(msg: &str) -> ErrorClass {
    let lower = msg.to_ascii_lowercase();
    if lower.contains("no such table")
        || lower.contains("no such column")
        || lower.contains("unknown table")
        || lower.contains("unknown column")
    {
        return ErrorClass::NoSuchTable;
    }
    if lower.contains("syntax error")
        || lower.contains("parse error")
        || lower.contains("near \"")
        || lower.contains("unsupported sql")
    {
        return ErrorClass::SyntaxError;
    }
    if lower.contains("constraint")
        || lower.contains("unique")
        || lower.contains("not null")
        || lower.contains("primary key")
        || lower.contains("cannot store")
    {
        return ErrorClass::ConstraintViolation;
    }
    if lower.contains("datatype")
        || lower.contains("type mismatch")
        || lower.contains("incompatible")
    {
        return ErrorClass::TypeError;
    }
    ErrorClass::Generic
}
