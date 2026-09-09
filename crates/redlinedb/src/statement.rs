//! Prepared-statement surfaces.
//!
//! `Prepared` is a cacheable template; `OwnedStatement` is a connection-bound
//! cursor decoupled from the borrow-checker; `Statement<'conn>` is the
//! borrowed wrapper that ties a cursor to its parent [`Connection`]; and
//! `Rows<'conn>` owns a `Statement` so callers can iterate without naming
//! the connection lifetime.

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

use crate::Connection;
use crate::error::{Error, ErrorCode, Result};
use crate::iter::{OwnedStep, Row, Step};
use crate::value::{Value, ValueRef};

/// Cached prepared-statement template detached from any live connection.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub(crate) template: Arc<redlinedb_sql::PreparedTemplate>,
}

impl Prepared {
    pub fn sql(&self) -> &str {
        self.template.sql.as_ref()
    }

    pub fn parameter_count(&self) -> usize {
        self.template.param_layout.count()
    }

    pub fn column_count(&self) -> usize {
        self.template.output_columns.len()
    }

    pub fn column_name(&self, index: usize) -> &str {
        self.template.output_columns[index].as_str()
    }

    pub fn is_readonly(&self) -> bool {
        self.template.readonly
    }
}

/// Owned prepared statement cursor that carries its own interrupt flag.
///
/// Construct via [`Connection::prepare_owned`]. Unlike [`Statement`], it is
/// not tied to the parent connection's lifetime via the borrow checker, so
/// it can be stored in long-lived structures (e.g. async tasks, FFI handles).
#[derive(Debug)]
pub struct OwnedStatement {
    pub(crate) inner: redlinedb_sql::Statement,
    pub(crate) interrupted: Arc<AtomicBool>,
    pub(crate) _marker: Rc<()>,
}

/// Borrowed prepared statement tied to one live [`Connection`].
///
/// `Statement` is intentionally not pooled or shared across threads. It
/// carries a mutable borrow of the parent connection, so keep it on the
/// thread that owns that connection and drop it before handing the
/// connection back to a pool.
pub struct Statement<'conn> {
    pub(crate) inner: OwnedStatement,
    pub(crate) _conn: std::marker::PhantomData<&'conn mut Connection>,
}

/// Owning wrapper around a [`Statement`] for ergonomic row iteration.
pub struct Rows<'conn> {
    pub(crate) stmt: Statement<'conn>,
}

impl<'conn> Statement<'conn> {
    pub fn bind_all<P: crate::params::Params>(&mut self, params: P) -> Result<()> {
        params.bind_into(self)
    }

    pub fn bind_null(&mut self, index: usize) -> Result<()> {
        self.inner.bind_null(index)
    }

    pub fn bind_i64(&mut self, index: usize, value: i64) -> Result<()> {
        self.inner.bind_i64(index, value)
    }

    pub fn bind_f64(&mut self, index: usize, value: f64) -> Result<()> {
        self.inner.bind_f64(index, value)
    }

    pub fn bind_text(&mut self, index: usize, value: impl Into<Arc<str>>) -> Result<()> {
        self.inner.bind_text(index, value)
    }

    pub fn bind_blob(&mut self, index: usize, value: impl Into<Arc<[u8]>>) -> Result<()> {
        self.inner.bind_blob(index, value)
    }

    pub fn bind_value(&mut self, index: usize, value: Value) -> Result<()> {
        match value {
            Value::Null => self.bind_null(index),
            Value::Integer(value) => self.bind_i64(index, value),
            Value::Real(value) => self.bind_f64(index, value),
            Value::Text(value) => self.bind_text(index, value),
            Value::Blob(value) => self.bind_blob(index, value),
        }
    }

    pub fn bind_named(&mut self, name: &str, value: Value) -> Result<()> {
        self.inner.bind_named(name, value)?;
        Ok(())
    }

    pub fn reset(&mut self) -> Result<()> {
        self.inner.reset()?;
        Ok(())
    }

    pub fn clear_bindings(&mut self) {
        self.inner.clear_bindings();
    }

    pub fn step(&mut self) -> Result<Step<'_>> {
        match self.inner.step()? {
            OwnedStep::Row => Ok(Step::Row(Row { stmt: self })),
            OwnedStep::Done => Ok(Step::Done),
        }
    }

    /// Bind parameters and iterate mapped rows via an ordered `step()` loop.
    /// Rows are fetched one-at-a-time and mapped in source order.
    pub fn query_map<P, T, F>(
        &mut self,
        params: P,
        f: F,
    ) -> Result<crate::iter::QueryMap<'_, 'conn, T, F>>
    where
        P: crate::params::Params,
        F: for<'row> FnMut(&Row<'row>) -> Result<T>,
    {
        self.bind_all(params)?;
        Ok(crate::iter::QueryMap::new(self, f))
    }

    pub fn is_readonly(&self) -> bool {
        self.inner.is_readonly()
    }

    pub fn affected_rows(&self) -> usize {
        self.inner.affected_rows()
    }

    pub fn parameter_count(&self) -> usize {
        self.inner.parameter_count()
    }

    pub fn parameter_index(&self, name: &str) -> Option<usize> {
        self.inner.parameter_index(name)
    }

    pub fn column_count(&self) -> usize {
        self.inner.column_count()
    }

    pub fn column_name(&self, index: usize) -> &str {
        self.inner.column_name(index)
    }

    pub fn template(&self) -> Arc<redlinedb_sql::PreparedTemplate> {
        self.inner.template()
    }
}

impl OwnedStatement {
    pub fn bind_null(&mut self, index: usize) -> Result<()> {
        Ok(self.inner.bind_null(index)?)
    }

    pub fn bind_i64(&mut self, index: usize, value: i64) -> Result<()> {
        Ok(self.inner.bind_i64(index, value)?)
    }

    pub fn bind_f64(&mut self, index: usize, value: f64) -> Result<()> {
        Ok(self.inner.bind_f64(index, value)?)
    }

    pub fn bind_text(&mut self, index: usize, value: impl Into<Arc<str>>) -> Result<()> {
        Ok(self.inner.bind_text(index, value)?)
    }

    pub fn bind_blob(&mut self, index: usize, value: impl Into<Arc<[u8]>>) -> Result<()> {
        Ok(self.inner.bind_blob(index, value)?)
    }

    pub fn bind_value(&mut self, index: usize, value: Value) -> Result<()> {
        match value {
            Value::Null => self.bind_null(index),
            Value::Integer(value) => self.bind_i64(index, value),
            Value::Real(value) => self.bind_f64(index, value),
            Value::Text(value) => self.bind_text(index, value),
            Value::Blob(value) => self.bind_blob(index, value),
        }
    }

    pub fn bind_named(&mut self, name: &str, value: Value) -> Result<()> {
        self.inner.bind_named(name, value.into())?;
        Ok(())
    }

    pub fn reset(&mut self) -> Result<()> {
        self.inner.reset()?;
        Ok(())
    }

    pub fn clear_bindings(&mut self) {
        self.inner.clear_bindings();
    }

    pub fn step(&mut self) -> Result<OwnedStep> {
        if self.interrupted.load(AtomicOrdering::Relaxed) {
            return Err(Error::new(ErrorCode::Interrupt, "interrupted"));
        }
        Ok(match self.inner.step()? {
            redlinedb_sql::Step::Row => OwnedStep::Row,
            redlinedb_sql::Step::Done => OwnedStep::Done,
        })
    }

    pub fn is_readonly(&self) -> bool {
        self.inner.is_readonly()
    }

    pub fn affected_rows(&self) -> usize {
        self.inner.affected_rows()
    }

    pub fn parameter_count(&self) -> usize {
        self.inner.parameter_count()
    }

    pub fn parameter_index(&self, name: &str) -> Option<usize> {
        self.inner.parameter_index(name)
    }

    pub fn column_count(&self) -> usize {
        self.inner.column_count()
    }

    pub fn column_name(&self, index: usize) -> &str {
        self.inner.column_name(index)
    }

    pub fn column_ref(&self, index: usize) -> Result<ValueRef<'_>> {
        Ok(match self.inner.column_value(index)? {
            redlinedb_sql::SqlValue::Null => ValueRef::Null,
            redlinedb_sql::SqlValue::Integer(value) => ValueRef::Integer(*value),
            redlinedb_sql::SqlValue::Real(value) => ValueRef::Real(*value),
            redlinedb_sql::SqlValue::Text(value) => ValueRef::Text(value.as_ref()),
            redlinedb_sql::SqlValue::Blob(value) => ValueRef::Blob(value.as_ref()),
        })
    }

    pub fn column_i64(&self, index: usize) -> Result<i64> {
        Ok(self.inner.column_i64(index)?)
    }

    pub fn column_f64(&self, index: usize) -> Result<f64> {
        Ok(self.inner.column_f64(index)?)
    }

    pub fn column_text(&self, index: usize) -> Result<&str> {
        Ok(self.inner.column_text(index)?)
    }

    pub fn column_blob(&self, index: usize) -> Result<&[u8]> {
        Ok(self.inner.column_blob(index)?)
    }

    pub fn template(&self) -> Arc<redlinedb_sql::PreparedTemplate> {
        self.inner.template()
    }
}

impl<'conn> Rows<'conn> {
    pub fn step(&mut self) -> Result<Step<'_>> {
        self.stmt.step()
    }

    pub fn statement(&mut self) -> &mut Statement<'conn> {
        &mut self.stmt
    }
}
