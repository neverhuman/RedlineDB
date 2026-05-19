#![allow(missing_docs)]

use std::sync::Arc;

use crate::{Error, ErrorCode, Result, Value};

#[derive(Debug, Clone)]
pub struct AsyncRow {
    pub(crate) columns: Vec<Value>,
    pub(crate) names: Arc<[String]>,
}

impl AsyncRow {
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&Value> {
        self.columns.get(index)
    }

    pub fn take(mut self, index: usize) -> Option<Value> {
        if index >= self.columns.len() {
            return None;
        }
        Some(self.columns.swap_remove(index))
    }

    pub fn column_name(&self, index: usize) -> Option<&str> {
        self.names.get(index).map(String::as_str)
    }

    pub fn column_names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    pub fn try_get_i64(&self, index: usize) -> Result<i64> {
        match self.get_required(index)? {
            Value::Integer(v) => Ok(*v),
            other => Err(mismatch(index, "Integer", other)),
        }
    }

    pub fn try_get_f64(&self, index: usize) -> Result<f64> {
        match self.get_required(index)? {
            Value::Real(v) => Ok(*v),
            other => Err(mismatch(index, "Real", other)),
        }
    }

    pub fn try_get_text(&self, index: usize) -> Result<&str> {
        match self.get_required(index)? {
            Value::Text(v) => Ok(v.as_ref()),
            other => Err(mismatch(index, "Text", other)),
        }
    }

    pub fn try_get_blob(&self, index: usize) -> Result<&[u8]> {
        match self.get_required(index)? {
            Value::Blob(v) => Ok(v.as_ref()),
            other => Err(mismatch(index, "Blob", other)),
        }
    }

    pub fn try_get_optional_i64(&self, index: usize) -> Result<Option<i64>> {
        match self.get_required(index)? {
            Value::Null => Ok(None),
            Value::Integer(v) => Ok(Some(*v)),
            other => Err(mismatch(index, "Integer or Null", other)),
        }
    }

    pub fn try_get_optional_text(&self, index: usize) -> Result<Option<&str>> {
        match self.get_required(index)? {
            Value::Null => Ok(None),
            Value::Text(v) => Ok(Some(v.as_ref())),
            other => Err(mismatch(index, "Text or Null", other)),
        }
    }

    fn get_required(&self, index: usize) -> Result<&Value> {
        match self.columns.get(index) {
            Some(value) => Ok(value),
            None => Err(column_out_of_bounds(index, self.columns.len())),
        }
    }
}

fn column_out_of_bounds(index: usize, len: usize) -> Error {
    Error::new(
        ErrorCode::Range,
        format!("column index {index} out of bounds (row has {len} columns)"),
    )
}

fn mismatch(index: usize, expected: &str, got: &Value) -> Error {
    let kind = match got {
        Value::Null => "Null",
        Value::Integer(_) => "Integer",
        Value::Real(_) => "Real",
        Value::Text(_) => "Text",
        Value::Blob(_) => "Blob",
    };
    Error::new(
        ErrorCode::Mismatch,
        format!("column {index} is {kind}, expected {expected}"),
    )
}
