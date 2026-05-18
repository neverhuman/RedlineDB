//! Row iteration types returned from prepared statements.
//!
//! `Step` / `OwnedStep` distinguish the borrowed and owned cursor surfaces,
//! and `Row` exposes typed column access via the `FromValue` trait.

use std::sync::Arc;

use crate::error::Result;
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
