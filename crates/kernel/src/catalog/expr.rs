#![allow(dead_code)]

use std::cmp::Ordering;
use std::sync::Arc;

use super::value::OwnedValue;

#[derive(Debug, Clone, PartialEq)]
pub enum ExprAst {
    Const(OwnedValue),
    Column(u16),
    CurrentDate,
    CurrentTime,
    CurrentTimestamp,
    Not(Box<ExprAst>),
    And(Box<ExprAst>, Box<ExprAst>),
    Or(Box<ExprAst>, Box<ExprAst>),
    Eq(Box<ExprAst>, Box<ExprAst>),
    Ne(Box<ExprAst>, Box<ExprAst>),
    Lt(Box<ExprAst>, Box<ExprAst>),
    Le(Box<ExprAst>, Box<ExprAst>),
    Gt(Box<ExprAst>, Box<ExprAst>),
    Ge(Box<ExprAst>, Box<ExprAst>),
    Like {
        negated: bool,
        value: Box<ExprAst>,
        pattern: Box<ExprAst>,
        escape: Option<char>,
    },
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
    CurrentDate,
    CurrentTime,
    CurrentTimestamp,
    Not,
    And,
    Or,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Like { negated: bool, escape: Option<char> },
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
    like_case_sensitive: bool,
) -> Result<OwnedValue, ExprError> {
    scratch.stack.clear();
    for op in expr.bytecode.iter() {
        match op {
            ExprOp::Const(v) => scratch.stack.push(v.clone()),
            ExprOp::Column(col) => scratch
                .stack
                .push(row.value_at(*col).ok_or(ExprError::UnknownColumn)?),
            ExprOp::CurrentDate => scratch.stack.push(OwnedValue::Text(Arc::from(
                UtcDateTime::now().format_date(),
            ))),
            ExprOp::CurrentTime => scratch.stack.push(OwnedValue::Text(Arc::from(
                UtcDateTime::now().format_time(),
            ))),
            ExprOp::CurrentTimestamp => scratch.stack.push(OwnedValue::Text(Arc::from(
                UtcDateTime::now().format_timestamp(),
            ))),
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
            ExprOp::Like { negated, escape } => {
                let pattern = scratch.stack.pop().ok_or(ExprError::StackUnderflow)?;
                let value = scratch.stack.pop().ok_or(ExprError::StackUnderflow)?;
                let result = like_result(&value, &pattern, *negated, *escape, like_case_sensitive);
                scratch.stack.push(result);
            }
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
        ExprAst::CurrentDate => bytecode.push(ExprOp::CurrentDate),
        ExprAst::CurrentTime => bytecode.push(ExprOp::CurrentTime),
        ExprAst::CurrentTimestamp => bytecode.push(ExprOp::CurrentTimestamp),
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
        ExprAst::Like {
            negated,
            value,
            pattern,
            escape,
        } => {
            compile_inner(value, bytecode, cols);
            compile_inner(pattern, bytecode, cols);
            bytecode.push(ExprOp::Like {
                negated: *negated,
                escape: *escape,
            });
        }
        ExprAst::BlobLen(operand) => {
            compile_inner(operand, bytecode, cols);
            bytecode.push(ExprOp::BlobLen);
        }
    }
}

fn like_result(
    value: &OwnedValue,
    pattern: &OwnedValue,
    negated: bool,
    escape: Option<char>,
    case_sensitive: bool,
) -> OwnedValue {
    if matches!(value, OwnedValue::Null) || matches!(pattern, OwnedValue::Null) {
        return OwnedValue::Null;
    }
    let text = owned_value_to_like_text(value);
    let pattern = owned_value_to_like_text(pattern);
    let matched = like_match(&text, &pattern, escape, case_sensitive);
    OwnedValue::Integer(if matched ^ negated { 1 } else { 0 })
}

fn owned_value_to_like_text(value: &OwnedValue) -> String {
    match value {
        OwnedValue::Null => String::new(),
        OwnedValue::Integer(v) => v.to_string(),
        OwnedValue::Real(v) => v.to_string(),
        OwnedValue::Text(v) => v.as_ref().to_owned(),
        OwnedValue::Blob(v) => String::from_utf8_lossy(v.as_ref()).into_owned(),
    }
}

fn like_match(text: &str, pattern: &str, escape: Option<char>, case_sensitive: bool) -> bool {
    let text = if case_sensitive {
        text.to_owned()
    } else {
        text.to_ascii_lowercase()
    };
    let pattern = if case_sensitive {
        pattern.to_owned()
    } else {
        pattern.to_ascii_lowercase()
    };
    like_match_inner(text.as_bytes(), pattern.as_bytes(), escape.map(|c| if case_sensitive { c } else { c.to_ascii_lowercase() }))
}

fn like_match_inner(text: &[u8], pattern: &[u8], escape: Option<char>) -> bool {
    fn inner(text: &[u8], pattern: &[u8], escape: Option<u8>) -> bool {
        let mut ti = 0usize;
        let mut pi = 0usize;
        while pi < pattern.len() {
            match pattern[pi] {
                b'%' => {
                    pi += 1;
                    if pi == pattern.len() {
                        return true;
                    }
                    while ti <= text.len() {
                        if inner(&text[ti..], &pattern[pi..], escape) {
                            return true;
                        }
                        if ti == text.len() {
                            break;
                        }
                        ti += 1;
                    }
                    return false;
                }
                b'_' => {
                    if ti == text.len() {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
                ch if Some(ch) == escape => {
                    pi += 1;
                    if pi >= pattern.len() || ti >= text.len() || pattern[pi] != text[ti] {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
                ch => {
                    if ti >= text.len() || text[ti] != ch {
                        return false;
                    }
                    ti += 1;
                    pi += 1;
                }
            }
        }
        ti == text.len()
    }

    inner(text, pattern, escape.map(|c| c as u8))
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

#[derive(Debug, Clone, Copy)]
struct UtcDateTime {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
}

impl UtcDateTime {
    fn now() -> Self {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};

        let dur = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(dur) => dur,
            Err(_) => Duration::default(),
        };
        Self::from_unix(dur.as_secs() as i64)
    }

    fn from_unix(secs: i64) -> Self {
        let mut s = secs.rem_euclid(86_400);
        let mut d = secs.div_euclid(86_400);
        let hour = (s / 3600) as u32;
        s %= 3600;
        let minute = (s / 60) as u32;
        let second = (s % 60) as u32;
        d += 719_468;
        let era = d.div_euclid(146_097);
        let doe = d.rem_euclid(146_097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 {
            (mp + 3) as u32
        } else {
            (mp - 9) as u32
        };
        let year = (y + if month <= 2 { 1 } else { 0 }) as i32;
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    fn format_date(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    fn format_time(&self) -> String {
        format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }

    fn format_timestamp(&self) -> String {
        format!("{} {}", self.format_date(), self.format_time())
    }
}
