//! Row iteration types returned from prepared statements.
//!
//! `Step` / `OwnedStep` distinguish the borrowed and owned cursor surfaces,
//! and `Row` exposes typed column access via the `FromValue` trait.

use std::sync::Arc;

use crate::error::{Error, ErrorCode, Result};
use crate::statement::Statement;
use crate::value::{Value, ValueRef};

/// Borrowed cursor position returned by [`Statement::step`].
pub enum Step<'a> {
    Row(Row<'a>),
    Done,
}

/// Owned cursor position returned by `OwnedStatement::step`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnedStep {
    Row,
    Done,
}

/// Borrowed reference to the current row of a stepping `Statement`.
pub struct Row<'stmt> {
    pub(crate) stmt: &'stmt Statement<'stmt>,
}

impl<'stmt> Row<'stmt> {
    pub fn get<T: FromValue>(&self, index: usize) -> Result<T> {
        T::from_statement(self.stmt, index)
    }

    pub fn get_ref(&self, index: usize) -> Result<ValueRef<'_>> {
        self.stmt.inner.column_ref(index)
    }
}

/// Trait for column-typed extraction from a borrowed [`Statement`] row.
pub trait FromValue: Sized {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self>;
}

/// Trait for mapping a full [`Row`] into a Rust value (typically a struct
/// or tuple). Used by `Connection::query_row` / `query_row_opt`.
///
/// A blanket impl maps any `T: FromValue` to column 0 — convenient for
/// single-column scalar queries.
pub trait FromRow: Sized {
    fn from_row(row: &Row<'_>) -> Result<Self>;
}

impl<T: FromValue> FromRow for T {
    fn from_row(row: &Row<'_>) -> Result<Self> {
        row.get::<T>(0)
    }
}

impl FromValue for i64 {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        stmt.inner.column_i64(index)
    }
}

impl FromValue for f64 {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        stmt.inner.column_f64(index)
    }
}

impl FromValue for String {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        Ok(stmt.inner.column_text(index)?.to_owned())
    }
}

impl FromValue for Value {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        Ok(match stmt.inner.column_ref(index)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(value) => Value::Integer(value),
            ValueRef::Real(value) => Value::Real(value),
            ValueRef::Text(value) => Value::Text(Arc::from(value)),
            ValueRef::Blob(value) => Value::Blob(Arc::from(value)),
        })
    }
}

/// Narrowing integer extraction; fails with [`ErrorCode::Mismatch`] if the
/// underlying `i64` does not fit the target type.
impl FromValue for i32 {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        i32::try_from(stmt.inner.column_i64(index)?)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "integer does not fit i32"))
    }
}

impl FromValue for i16 {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        i16::try_from(stmt.inner.column_i64(index)?)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "integer does not fit i16"))
    }
}

impl FromValue for i8 {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        i8::try_from(stmt.inner.column_i64(index)?)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "integer does not fit i8"))
    }
}

impl FromValue for u64 {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        u64::try_from(stmt.inner.column_i64(index)?)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "integer does not fit u64"))
    }
}

impl FromValue for u32 {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        u32::try_from(stmt.inner.column_i64(index)?)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "integer does not fit u32"))
    }
}

impl FromValue for u16 {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        u16::try_from(stmt.inner.column_i64(index)?)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "integer does not fit u16"))
    }
}

impl FromValue for u8 {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        u8::try_from(stmt.inner.column_i64(index)?)
            .map_err(|_| Error::new(ErrorCode::Mismatch, "integer does not fit u8"))
    }
}

/// `INTEGER` storage class with sqlite truthy convention: any nonzero
/// integer is `true`, `0` is `false`.
impl FromValue for bool {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        stmt.inner.column_i64(index).map(|v| v != 0)
    }
}

impl FromValue for f32 {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        #[allow(clippy::cast_possible_truncation)]
        Ok(stmt.inner.column_f64(index)? as f32)
    }
}

impl FromValue for Vec<u8> {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        Ok(stmt.inner.column_blob(index)?.to_vec())
    }
}

/// `Option<T>` returns `None` for `NULL`, else delegates to `T`.
impl<T: FromValue> FromValue for Option<T> {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        match stmt.inner.column_ref(index)? {
            ValueRef::Null => Ok(None),
            _ => T::from_statement(stmt, index).map(Some),
        }
    }
}
