#![allow(dead_code)]

use std::cmp::Ordering;
use std::sync::Arc;

use super::value::OwnedValue;

#[derive(Debug, Clone, PartialEq)]
pub enum ExprAst {
    Const(OwnedValue),
    Column(u16),
    Not(Box<ExprAst>),
    And(Box<ExprAst>, Box<ExprAst>),
    Or(Box<ExprAst>, Box<ExprAst>),
    Eq(Box<ExprAst>, Box<ExprAst>),
    Ne(Box<ExprAst>, Box<ExprAst>),
    Lt(Box<ExprAst>, Box<ExprAst>),
    Le(Box<ExprAst>, Box<ExprAst>),
    Gt(Box<ExprAst>, Box<ExprAst>),
    Ge(Box<ExprAst>, Box<ExprAst>),
    /// Length of the operand interpreted as a Blob, in bytes.
    ///
    /// Returns `Null` if the operand is not a `Blob`. This is the minimum
    /// surface needed for the SQL layer to express byte-exact vector dimension
    /// CHECK constraints (`BlobLen(col) == K`) without bringing the full
    /// `length()` SQL function into the kernel evaluator. Added in phase-10
    /// Lane V1.
    BlobLen(Box<ExprAst>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprOp {
    Const(OwnedValue),
    Column(u16),
    Not,
    And,
    Or,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    BlobLen,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledExpr {
    pub bytecode: Box<[ExprOp]>,
    pub referenced_cols: Vec<u16>,
}

#[derive(Debug, Default)]
pub struct EvalScratch {
    stack: Vec<OwnedValue>,
}

pub trait RowValueSource {
    fn value_at(&self, col: u16) -> Option<OwnedValue>;
}

#[derive(Debug, thiserror::Error)]
pub enum ExprError {
    #[error("stack underflow")]
    StackUnderflow,
    #[error("unknown column")]
    UnknownColumn,
}

pub fn compile_expr(expr: &ExprAst) -> Arc<CompiledExpr> {
    let mut bytecode = Vec::new();
    let mut cols = Vec::new();
    compile_inner(expr, &mut bytecode, &mut cols);
    Arc::new(CompiledExpr {
        bytecode: bytecode.into_boxed_slice(),
        referenced_cols: cols,
    })
}

pub fn eval_expr(
    expr: &CompiledExpr,
    row: &dyn RowValueSource,
    scratch: &mut EvalScratch,
) -> Result<OwnedValue, ExprError> {
    scratch.stack.clear();
    for op in expr.bytecode.iter() {
        match op {
            ExprOp::Const(v) => scratch.stack.push(v.clone()),
            ExprOp::Column(col) => scratch
                .stack
                .push(row.value_at(*col).ok_or(ExprError::UnknownColumn)?),
            ExprOp::Not => {
                let v = scratch.stack.pop().ok_or(ExprError::StackUnderflow)?;
                scratch.stack.push(bool_value(!truthy(&v)));
            }
            ExprOp::And => binary_bool(&mut scratch.stack, |a, b| a && b)?,
            ExprOp::Or => binary_bool(&mut scratch.stack, |a, b| a || b)?,
            ExprOp::Eq => compare_into(&mut scratch.stack, |o| o == Ordering::Equal)?,
            ExprOp::Ne => compare_into(&mut scratch.stack, |o| o != Ordering::Equal)?,
            ExprOp::Lt => compare_into(&mut scratch.stack, |o| o == Ordering::Less)?,
            ExprOp::Le => compare_into(&mut scratch.stack, |o| o != Ordering::Greater)?,
            ExprOp::Gt => compare_into(&mut scratch.stack, |o| o == Ordering::Greater)?,
            ExprOp::Ge => compare_into(&mut scratch.stack, |o| o != Ordering::Less)?,
            ExprOp::BlobLen => {
                let v = scratch.stack.pop().ok_or(ExprError::StackUnderflow)?;
                let result = match v {
                    OwnedValue::Blob(b) => OwnedValue::Integer(b.len() as i64),
                    // Null propagates so that a CHECK like
                    // `BlobLen(col) = K` is satisfied for nullable columns
                    // — matching SQLite's "Null in a CHECK is satisfied"
                    // semantics enforced by `apply_constraints`.
                    OwnedValue::Null => OwnedValue::Null,
                    // Non-blob, non-null value: surface a sentinel `-1` so
                    // that comparisons against any non-negative expected
                    // length deterministically fail (`-1 = K` → 0). This is
                    // how vector-dimension enforcement rejects an INTEGER /
                    // TEXT / REAL written into a `VECTOR(N)` column.
                    _ => OwnedValue::Integer(-1),
                };
                scratch.stack.push(result);
            }
        }
    }
    Ok(scratch.stack.pop().unwrap_or(OwnedValue::Null))
}

fn compile_inner(expr: &ExprAst, bytecode: &mut Vec<ExprOp>, cols: &mut Vec<u16>) {
    match expr {
        ExprAst::Const(v) => bytecode.push(ExprOp::Const(v.clone())),
        ExprAst::Column(col) => {
            bytecode.push(ExprOp::Column(*col));
            cols.push(*col);
        }
        ExprAst::Not(expr) => {
            compile_inner(expr, bytecode, cols);
            bytecode.push(ExprOp::Not);
        }
        ExprAst::And(left, right) => {
            compile_inner(left, bytecode, cols);
            compile_inner(right, bytecode, cols);
            bytecode.push(ExprOp::And);
        }
        ExprAst::Or(left, right) => {
            compile_inner(left, bytecode, cols);
            compile_inner(right, bytecode, cols);
            bytecode.push(ExprOp::Or);
        }
        ExprAst::Eq(left, right) => {
            compile_inner(left, bytecode, cols);
            compile_inner(right, bytecode, cols);
            bytecode.push(ExprOp::Eq);
        }
        ExprAst::Ne(left, right) => {
            compile_inner(left, bytecode, cols);
            compile_inner(right, bytecode, cols);
            bytecode.push(ExprOp::Ne);
        }
        ExprAst::Lt(left, right) => {
            compile_inner(left, bytecode, cols);
            compile_inner(right, bytecode, cols);
            bytecode.push(ExprOp::Lt);
        }
        ExprAst::Le(left, right) => {
            compile_inner(left, bytecode, cols);
            compile_inner(right, bytecode, cols);
            bytecode.push(ExprOp::Le);
        }
        ExprAst::Gt(left, right) => {
            compile_inner(left, bytecode, cols);
            compile_inner(right, bytecode, cols);
            bytecode.push(ExprOp::Gt);
        }
        ExprAst::Ge(left, right) => {
            compile_inner(left, bytecode, cols);
            compile_inner(right, bytecode, cols);
            bytecode.push(ExprOp::Ge);
        }
        ExprAst::BlobLen(operand) => {
            compile_inner(operand, bytecode, cols);
            bytecode.push(ExprOp::BlobLen);
        }
    }
}

fn binary_bool(
    stack: &mut Vec<OwnedValue>,
    combine: impl FnOnce(bool, bool) -> bool,
) -> Result<(), ExprError> {
    let right = truthy(&stack.pop().ok_or(ExprError::StackUnderflow)?);
    let left = truthy(&stack.pop().ok_or(ExprError::StackUnderflow)?);
    stack.push(bool_value(combine(left, right)));
    Ok(())
}

fn compare_into(
    stack: &mut Vec<OwnedValue>,
    accept: impl FnOnce(Ordering) -> bool,
) -> Result<(), ExprError> {
    let right = stack.pop().ok_or(ExprError::StackUnderflow)?;
    let left = stack.pop().ok_or(ExprError::StackUnderflow)?;
    // SQL three-valued logic: any comparison with NULL produces NULL. The
    // SQL `apply_constraints` treats NULL as "constraint satisfied", which is
    // what callers depend on (e.g. the auto-emitted `BlobLen(col) = K` for
    // a nullable VECTOR column passes when col is NULL).
    if matches!(left, OwnedValue::Null) || matches!(right, OwnedValue::Null) {
        stack.push(OwnedValue::Null);
        return Ok(());
    }
    let ord = compare_values(&left, &right);
    stack.push(bool_value(accept(ord)));
    Ok(())
}

fn compare_values(left: &OwnedValue, right: &OwnedValue) -> Ordering {
    use OwnedValue::*;
    match (left, right) {
        (Null, Null) => Ordering::Equal,
        (Null, _) => Ordering::Less,
        (_, Null) => Ordering::Greater,
        (Integer(a), Integer(b)) => a.cmp(b),
        (Real(a), Real(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
        (Integer(a), Real(b)) => (*a as f64).partial_cmp(b).unwrap_or(Ordering::Equal),
        (Real(a), Integer(b)) => a.partial_cmp(&(*b as f64)).unwrap_or(Ordering::Equal),
        (Text(a), Text(b)) => a.as_ref().cmp(b.as_ref()),
        (Blob(a), Blob(b)) => a.as_ref().cmp(b.as_ref()),
        (Integer(_) | Real(_), Text(_) | Blob(_)) => Ordering::Less,
        (Text(_) | Blob(_), Integer(_) | Real(_)) => Ordering::Greater,
        (Text(_), Blob(_)) => Ordering::Less,
        (Blob(_), Text(_)) => Ordering::Greater,
    }
}

fn truthy(value: &OwnedValue) -> bool {
    match value {
        OwnedValue::Null => false,
        OwnedValue::Integer(v) => *v != 0,
        OwnedValue::Real(v) => *v != 0.0,
        OwnedValue::Text(v) => !v.is_empty(),
        OwnedValue::Blob(v) => !v.is_empty(),
    }
}

fn bool_value(v: bool) -> OwnedValue {
    OwnedValue::Integer(if v { 1 } else { 0 })
}
