//! Phase 6 R1-C — Two-Tier ScalarProgram VM (Tier 0).
//!
//! Compiles a `sqlparser::ast::Expr` into a flat bytecode program that a
//! stack interpreter evaluates per row. The interpreter is a tight
//! `match` over `u8`-sized opcodes; constants and function references are
//! stored in side tables so opcode operands stay small.
//!
//! This is the Tier-0 surface only. The compiler returns `Ok(None)` for
//! any expression shape it does not understand so callers can fall back
//! to the AST walker (`super::eval_scalar`). The executor switch itself
//! lands in a later round; this module is shipped as a parallel
//! implementation with its own integration test.
//!
//! Semantics intentionally mirror `crate::exec::expr` so a differential
//! test can swap evaluators row-for-row without divergence on supported
//! shapes (literals, identifiers, parameters, arithmetic, comparison,
//! AND/OR/NOT, `||` concat, `lower`/`upper`/`length`/`abs`, CASE).

// This module is shipped ahead of the AST→bytecode fallback wiring; the
// crate-level build sees every item as unused until a later round flips
// the executor over. Silence the dead-code noise until then.
#![allow(dead_code)]

use std::cmp::Ordering;
use std::sync::Arc;

use smallvec::SmallVec;
use sqlparser::ast::{
    BinaryOperator, CaseWhen, Expr, FunctionArg, FunctionArgExpr, FunctionArguments, UnaryOperator,
    Value,
};

use crate::error::{Error, Result};
use crate::value::SqlValue;

/// Hard cap on the evaluation stack depth. Programs whose maximum stack
/// height exceeds this are rejected at compile time; the interpreter
/// preallocates a `SmallVec` of this size and never grows past it, so a
/// runtime stack overflow is unreachable.
pub const MAX_STACK: usize = 32;

/// Maximum size of any side table addressed by a `u8` operand.
pub const MAX_CONST_TABLE: usize = u8::MAX as usize;

/// Bytecode opcodes for the scalar expression VM. Variants that carry an
/// operand index keep it inline (`u8`) so the enum stays compact; jump
/// targets use `u16` for branch ranges longer than 256 ops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    LoadConstI64(u8),
    LoadConstReal(u8),
    LoadConstText(u8),
    LoadConstNull,
    LoadCol(u8),
    LoadBinding(u8),

    AddI64,
    AddReal,
    SubI64,
    SubReal,
    MulI64,
    MulReal,
    DivI64,
    DivReal,
    ModI64,

    CmpEq,
    CmpNe,
    CmpLt,
    CmpLe,
    CmpGt,
    CmpGe,

    LogAnd,
    LogOr,
    LogNot,

    // Unary numeric negation. Pops 1, pushes 1.
    Neg,

    Concat,

    CallScalar1(u8),

    JumpIfFalse(u16),
    Jump(u16),

    Return,
}

/// Built-in scalar functions the Tier-0 compiler recognises. Keeping the
/// set narrow lets the interpreter dispatch via a small `match` rather
/// than a vtable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarFn {
    Abs,
    Length,
    Lower,
    Upper,
}

/// Compiled scalar expression. `ops` is the linear bytecode; the const
/// pools hold operand-addressed literals so opcodes themselves stay
/// `Copy + Clone + u8`-sized.
#[derive(Debug, Clone)]
pub struct ScalarProgram {
    pub ops: Vec<Op>,
    pub const_i64: SmallVec<[i64; 8]>,
    pub const_real: SmallVec<[f64; 4]>,
    pub const_text: SmallVec<[Arc<str>; 4]>,
    pub fn_table: SmallVec<[ScalarFn; 4]>,
    pub max_stack: usize,
}

impl ScalarProgram {
    fn new() -> Self {
        Self {
            ops: Vec::new(),
            const_i64: SmallVec::new(),
            const_real: SmallVec::new(),
            const_text: SmallVec::new(),
            fn_table: SmallVec::new(),
            max_stack: 0,
        }
    }
}

/// Compile-time context. Maps a column-reference name (lowercased) to a
/// row ordinal. Caller passes an empty map when the expression cannot
/// reference columns.
pub struct CompileCtx<'a> {
    pub columns: &'a [(String, usize)],
}

impl<'a> CompileCtx<'a> {
    pub fn empty() -> Self {
        Self { columns: &[] }
    }

    fn lookup(&self, name: &str) -> Option<usize> {
        let lower = name.to_ascii_lowercase();
        self.columns
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(&lower))
            .map(|(_, idx)| *idx)
    }
}

/// Compile `expr` to bytecode. Returns `Ok(None)` when any sub-shape is
/// outside Tier-0 support; the caller treats that as a signal to fall
/// back to the AST evaluator. Never panics.
pub fn compile(expr: &Expr, ctx: &CompileCtx<'_>) -> Result<Option<ScalarProgram>> {
    let mut program = ScalarProgram::new();
    let mut state = CompileState {
        stack: 0,
        max_stack: 0,
    };
    if !compile_into(expr, ctx, &mut program, &mut state)? {
        return Ok(None);
    }
    program.ops.push(Op::Return);
    program.max_stack = state.max_stack;
    if program.max_stack > MAX_STACK {
        return Ok(None);
    }
    Ok(Some(program))
}

struct CompileState {
    stack: usize,
    max_stack: usize,
}

impl CompileState {
    fn push(&mut self, n: usize) {
        self.stack += n;
        if self.stack > self.max_stack {
            self.max_stack = self.stack;
        }
    }
    fn pop(&mut self, n: usize) {
        debug_assert!(self.stack >= n, "compile-time stack underflow");
        self.stack -= n;
    }
}

fn intern_i64(p: &mut ScalarProgram, v: i64) -> Option<u8> {
    if let Some(idx) = p.const_i64.iter().position(|x| *x == v) {
        return Some(idx as u8);
    }
    if p.const_i64.len() >= MAX_CONST_TABLE {
        return None;
    }
    let idx = p.const_i64.len() as u8;
    p.const_i64.push(v);
    Some(idx)
}

fn intern_real(p: &mut ScalarProgram, v: f64) -> Option<u8> {
    if let Some(idx) = p.const_real.iter().position(|x| x.to_bits() == v.to_bits()) {
        return Some(idx as u8);
    }
    if p.const_real.len() >= MAX_CONST_TABLE {
        return None;
    }
    let idx = p.const_real.len() as u8;
    p.const_real.push(v);
    Some(idx)
}

fn intern_text(p: &mut ScalarProgram, v: &str) -> Option<u8> {
    if let Some(idx) = p.const_text.iter().position(|x| x.as_ref() == v) {
        return Some(idx as u8);
    }
    if p.const_text.len() >= MAX_CONST_TABLE {
        return None;
    }
    let idx = p.const_text.len() as u8;
    p.const_text.push(Arc::from(v));
    Some(idx)
}

fn intern_fn(p: &mut ScalarProgram, f: ScalarFn) -> Option<u8> {
    if let Some(idx) = p.fn_table.iter().position(|x| *x == f) {
        return Some(idx as u8);
    }
    if p.fn_table.len() >= MAX_CONST_TABLE {
        return None;
    }
    let idx = p.fn_table.len() as u8;
    p.fn_table.push(f);
    Some(idx)
}

/// Returns `Ok(true)` on success, `Ok(false)` for an unsupported AST shape.
fn compile_into(
    expr: &Expr,
    ctx: &CompileCtx<'_>,
    p: &mut ScalarProgram,
    s: &mut CompileState,
) -> Result<bool> {
    match expr {
        Expr::Nested(inner) => compile_into(inner, ctx, p, s),
        Expr::Value(v) => compile_value(&v.value, p, s),
        Expr::Identifier(id) => compile_identifier(&id.value, ctx, p, s),
        Expr::CompoundIdentifier(parts) => match parts.as_slice() {
            [id] => compile_identifier(&id.value, ctx, p, s),
            _ => Ok(false),
        },
        Expr::UnaryOp { op, expr } => compile_unary(op, expr, ctx, p, s),
        Expr::BinaryOp { left, op, right } => compile_binary(left, op, right, ctx, p, s),
        Expr::Function(func) => compile_function(func, ctx, p, s),
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            if operand.is_some() {
                return Ok(false);
            }
            compile_searched_case(conditions, else_result.as_deref(), ctx, p, s)
        }
        _ => Ok(false),
    }
}

fn compile_value(v: &Value, p: &mut ScalarProgram, s: &mut CompileState) -> Result<bool> {
    // Bind parameters arrive here as `Value::Placeholder("?N")`.
    if let Some(name) = crate::parser::bind::as_bind_name(v) {
        if let Some(rest) = name.strip_prefix('?') {
            if let Ok(slot) = rest.parse::<usize>() {
                if slot > u8::MAX as usize + 1 || slot == 0 {
                    return Ok(false);
                }
                p.ops.push(Op::LoadBinding((slot - 1) as u8));
                s.push(1);
                return Ok(true);
            }
        }
        return Ok(false);
    }
    match v {
        Value::Null => {
            p.ops.push(Op::LoadConstNull);
            s.push(1);
            Ok(true)
        }
        Value::Boolean(b) => {
            let idx = match intern_i64(p, if *b { 1 } else { 0 }) {
                Some(i) => i,
                None => return Ok(false),
            };
            p.ops.push(Op::LoadConstI64(idx));
            s.push(1);
            Ok(true)
        }
        Value::Number(n, _) => {
            if let Ok(i) = n.parse::<i64>() {
                let idx = match intern_i64(p, i) {
                    Some(i) => i,
                    None => return Ok(false),
                };
                p.ops.push(Op::LoadConstI64(idx));
                s.push(1);
                Ok(true)
            } else if let Ok(f) = n.parse::<f64>() {
                let idx = match intern_real(p, f) {
                    Some(i) => i,
                    None => return Ok(false),
                };
                p.ops.push(Op::LoadConstReal(idx));
                s.push(1);
                Ok(true)
            } else {
                Ok(false)
            }
        }
        Value::SingleQuotedString(t)
        | Value::DoubleQuotedString(t)
        | Value::EscapedStringLiteral(t) => {
            let idx = match intern_text(p, t) {
                Some(i) => i,
                None => return Ok(false),
            };
            p.ops.push(Op::LoadConstText(idx));
            s.push(1);
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn compile_identifier(
    name: &str,
    ctx: &CompileCtx<'_>,
    p: &mut ScalarProgram,
    s: &mut CompileState,
) -> Result<bool> {
    let Some(col) = ctx.lookup(name) else {
        return Ok(false);
    };
    if col > u8::MAX as usize {
        return Ok(false);
    }
    p.ops.push(Op::LoadCol(col as u8));
    s.push(1);
    Ok(true)
}

fn compile_unary(
    op: &UnaryOperator,
    expr: &Expr,
    ctx: &CompileCtx<'_>,
    p: &mut ScalarProgram,
    s: &mut CompileState,
) -> Result<bool> {
    if !compile_into(expr, ctx, p, s)? {
        return Ok(false);
    }
    match op {
        UnaryOperator::Not => {
            p.ops.push(Op::LogNot);
            // pop 1, push 1 — no net stack change
            Ok(true)
        }
        UnaryOperator::Plus => Ok(true),
        UnaryOperator::Minus => {
            // Operand is already on top of the stack; the `Neg` opcode
            // pops it, applies SQLite-style negation, and pushes back.
            p.ops.push(Op::Neg);
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn compile_binary(
    left: &Expr,
    op: &BinaryOperator,
    right: &Expr,
    ctx: &CompileCtx<'_>,
    p: &mut ScalarProgram,
    s: &mut CompileState,
) -> Result<bool> {
    let arith_op = match op {
        BinaryOperator::Plus => Some(Op::AddI64),
        BinaryOperator::Minus => Some(Op::SubI64),
        BinaryOperator::Multiply => Some(Op::MulI64),
        BinaryOperator::Divide => Some(Op::DivI64),
        BinaryOperator::Modulo => Some(Op::ModI64),
        _ => None,
    };
    if let Some(op_code) = arith_op {
        if !compile_into(left, ctx, p, s)? {
            return Ok(false);
        }
        if !compile_into(right, ctx, p, s)? {
            return Ok(false);
        }
        p.ops.push(op_code);
        s.pop(1);
        return Ok(true);
    }
    let cmp_op = match op {
        BinaryOperator::Eq => Some(Op::CmpEq),
        BinaryOperator::NotEq => Some(Op::CmpNe),
        BinaryOperator::Lt => Some(Op::CmpLt),
        BinaryOperator::LtEq => Some(Op::CmpLe),
        BinaryOperator::Gt => Some(Op::CmpGt),
        BinaryOperator::GtEq => Some(Op::CmpGe),
        _ => None,
    };
    if let Some(op_code) = cmp_op {
        if !compile_into(left, ctx, p, s)? {
            return Ok(false);
        }
        if !compile_into(right, ctx, p, s)? {
            return Ok(false);
        }
        p.ops.push(op_code);
        s.pop(1);
        return Ok(true);
    }
    match op {
        BinaryOperator::And => {
            if !compile_into(left, ctx, p, s)? {
                return Ok(false);
            }
            if !compile_into(right, ctx, p, s)? {
                return Ok(false);
            }
            p.ops.push(Op::LogAnd);
            s.pop(1);
            Ok(true)
        }
        BinaryOperator::Or => {
            if !compile_into(left, ctx, p, s)? {
                return Ok(false);
            }
            if !compile_into(right, ctx, p, s)? {
                return Ok(false);
            }
            p.ops.push(Op::LogOr);
            s.pop(1);
            Ok(true)
        }
        BinaryOperator::StringConcat => {
            if !compile_into(left, ctx, p, s)? {
                return Ok(false);
            }
            if !compile_into(right, ctx, p, s)? {
                return Ok(false);
            }
            p.ops.push(Op::Concat);
            s.pop(1);
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn compile_function(
    func: &sqlparser::ast::Function,
    ctx: &CompileCtx<'_>,
    p: &mut ScalarProgram,
    s: &mut CompileState,
) -> Result<bool> {
    let name = func.name.to_string().to_ascii_lowercase();
    let fn_kind = match name.as_str() {
        "abs" => ScalarFn::Abs,
        "length" => ScalarFn::Length,
        "lower" => ScalarFn::Lower,
        "upper" => ScalarFn::Upper,
        _ => return Ok(false),
    };
    let list = match &func.args {
        FunctionArguments::List(list) if list.args.len() == 1 => list,
        _ => return Ok(false),
    };
    let arg = match &list.args[0] {
        FunctionArg::Unnamed(FunctionArgExpr::Expr(e)) => e,
        _ => return Ok(false),
    };
    if !compile_into(arg, ctx, p, s)? {
        return Ok(false);
    }
    let idx = match intern_fn(p, fn_kind) {
        Some(i) => i,
        None => return Ok(false),
    };
    p.ops.push(Op::CallScalar1(idx));
    // pop 1, push 1 — no net change
    Ok(true)
}

fn compile_searched_case(
    conditions: &[CaseWhen],
    else_result: Option<&Expr>,
    ctx: &CompileCtx<'_>,
    p: &mut ScalarProgram,
    s: &mut CompileState,
) -> Result<bool> {
    // For each WHEN: eval condition; JumpIfFalse → next-when; eval result;
    // Jump → end. Else branch (or NULL) at the tail.
    let mut end_patches: Vec<usize> = Vec::with_capacity(conditions.len());
    for when in conditions {
        if !compile_into(&when.condition, ctx, p, s)? {
            return Ok(false);
        }
        // JumpIfFalse pops one.
        let jf_idx = p.ops.len();
        p.ops.push(Op::JumpIfFalse(0));
        s.pop(1);
        let stack_before_result = s.stack;
        if !compile_into(&when.result, ctx, p, s)? {
            return Ok(false);
        }
        // Result push leaves stack at +1. Record end patch.
        let jmp_idx = p.ops.len();
        p.ops.push(Op::Jump(0));
        end_patches.push(jmp_idx);
        // Patch JumpIfFalse target to the op *after* the Jump we just
        // emitted (i.e. the start of the next when / else branch).
        let target = p.ops.len();
        if target > u16::MAX as usize {
            return Ok(false);
        }
        if let Op::JumpIfFalse(slot) = &mut p.ops[jf_idx] {
            *slot = target as u16;
        }
        // Reset compile-time stack tracker to the pre-result state — the
        // alternative branch produces a single value just like this one.
        s.stack = stack_before_result;
    }
    // Else branch.
    if let Some(else_expr) = else_result {
        if !compile_into(else_expr, ctx, p, s)? {
            return Ok(false);
        }
    } else {
        p.ops.push(Op::LoadConstNull);
        s.push(1);
    }
    // Patch all end-jumps to the current PC.
    let end_target = p.ops.len();
    if end_target > u16::MAX as usize {
        return Ok(false);
    }
    for jmp_idx in end_patches {
        if let Op::Jump(slot) = &mut p.ops[jmp_idx] {
            *slot = end_target as u16;
        }
    }
    Ok(true)
}

// ── Interpreter ─────────────────────────────────────────────────────────────

/// Evaluate a compiled program against a flat row and binding slice.
/// Row values are addressed by ordinal; bindings by 0-based slot.
pub fn evaluate(
    program: &ScalarProgram,
    row: &[SqlValue],
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    let mut stack: SmallVec<[SqlValue; MAX_STACK]> = SmallVec::new();
    let mut pc = 0usize;
    while pc < program.ops.len() {
        let op = program.ops[pc];
        pc += 1;
        match op {
            Op::LoadConstI64(i) => stack.push(SqlValue::Integer(program.const_i64[i as usize])),
            Op::LoadConstReal(i) => stack.push(SqlValue::Real(program.const_real[i as usize])),
            Op::LoadConstText(i) => {
                stack.push(SqlValue::Text(program.const_text[i as usize].clone()))
            }
            Op::LoadConstNull => stack.push(SqlValue::Null),
            Op::LoadCol(i) => {
                let v = row.get(i as usize).cloned().unwrap_or(SqlValue::Null);
                stack.push(v);
            }
            Op::LoadBinding(i) => {
                let v = bindings
                    .get(i as usize)
                    .and_then(|slot| slot.clone())
                    .unwrap_or(SqlValue::Null);
                stack.push(v);
            }
            Op::AddI64 => {
                let r = stack.pop().unwrap();
                let l = stack.pop().unwrap();
                stack.push(arith(
                    l,
                    r,
                    |a, b| Some(a.wrapping_add(b)),
                    |a, b| Some(a + b),
                )?);
            }
            Op::SubI64 => {
                let r = stack.pop().unwrap();
                let l = stack.pop().unwrap();
                stack.push(arith(
                    l,
                    r,
                    |a, b| Some(a.wrapping_sub(b)),
                    |a, b| Some(a - b),
                )?);
            }
            Op::MulI64 => {
                let r = stack.pop().unwrap();
                let l = stack.pop().unwrap();
                stack.push(arith(
                    l,
                    r,
                    |a, b| Some(a.wrapping_mul(b)),
                    |a, b| Some(a * b),
                )?);
            }
            Op::DivI64 => {
                let r = stack.pop().unwrap();
                let l = stack.pop().unwrap();
                stack.push(arith(
                    l,
                    r,
                    |a, b| if b == 0 { None } else { a.checked_div(b) },
                    |a, b| if b == 0.0 { None } else { Some(a / b) },
                )?);
            }
            Op::ModI64 => {
                let r = stack.pop().unwrap();
                let l = stack.pop().unwrap();
                stack.push(arith(
                    l,
                    r,
                    |a, b| if b == 0 { None } else { a.checked_rem(b) },
                    |a, b| if b == 0.0 { None } else { Some(a % b) },
                )?);
            }
            // Real-typed arithmetic opcodes alias the generic ones — the
            // arith helper already promotes to real on mixed types. They
            // are reserved for a Tier-1 specialiser.
            Op::AddReal | Op::SubReal | Op::MulReal | Op::DivReal => {
                return Err(Error::UnsupportedSql(
                    "real-specialised arithmetic opcodes are reserved for Tier 1".to_owned(),
                ));
            }
            Op::CmpEq => binary_cmp(&mut stack, |o| o == Ordering::Equal),
            Op::CmpNe => binary_cmp(&mut stack, |o| o != Ordering::Equal),
            Op::CmpLt => binary_cmp(&mut stack, |o| o == Ordering::Less),
            Op::CmpLe => binary_cmp(&mut stack, |o| o != Ordering::Greater),
            Op::CmpGt => binary_cmp(&mut stack, |o| o == Ordering::Greater),
            Op::CmpGe => binary_cmp(&mut stack, |o| o != Ordering::Less),
            Op::LogAnd => {
                let r = stack.pop().unwrap();
                let l = stack.pop().unwrap();
                stack.push(logical_and(&l, &r));
            }
            Op::LogOr => {
                let r = stack.pop().unwrap();
                let l = stack.pop().unwrap();
                stack.push(logical_or(&l, &r));
            }
            Op::LogNot => {
                let v = stack.pop().unwrap();
                stack.push(logical_not(&v));
            }
            Op::Neg => {
                let v = stack.pop().unwrap();
                stack.push(negate(v)?);
            }
            Op::Concat => {
                let r = stack.pop().unwrap();
                let l = stack.pop().unwrap();
                stack.push(concat(&l, &r));
            }
            Op::CallScalar1(i) => {
                let v = stack.pop().unwrap();
                let f = program.fn_table[i as usize];
                stack.push(call_scalar1(f, v)?);
            }
            Op::JumpIfFalse(target) => {
                let v = stack.pop().unwrap();
                if !truthy_strict(&v) {
                    pc = target as usize;
                }
            }
            Op::Jump(target) => {
                pc = target as usize;
            }
            Op::Return => break,
        }
    }
    stack
        .pop()
        .ok_or_else(|| Error::UnsupportedSql("scalar program produced no value".to_owned()))
}

fn binary_cmp(stack: &mut SmallVec<[SqlValue; MAX_STACK]>, accept: fn(Ordering) -> bool) {
    let r = stack.pop().unwrap();
    let l = stack.pop().unwrap();
    if matches!(l, SqlValue::Null) || matches!(r, SqlValue::Null) {
        stack.push(SqlValue::Null);
    } else {
        let ord = compare_values_local(&l, &r);
        stack.push(SqlValue::Integer(if accept(ord) { 1 } else { 0 }));
    }
}

fn compare_values_local(l: &SqlValue, r: &SqlValue) -> Ordering {
    crate::value::compare_values(l, r)
}

fn truthy_opt(v: &SqlValue) -> Option<bool> {
    match v {
        SqlValue::Null => None,
        _ => Some(crate::value::is_truthy(v)),
    }
}

fn truthy_strict(v: &SqlValue) -> bool {
    matches!(truthy_opt(v), Some(true))
}

fn logical_and(l: &SqlValue, r: &SqlValue) -> SqlValue {
    match (truthy_opt(l), truthy_opt(r)) {
        (Some(false), _) | (_, Some(false)) => SqlValue::Integer(0),
        (Some(true), Some(true)) => SqlValue::Integer(1),
        _ => SqlValue::Null,
    }
}

fn logical_or(l: &SqlValue, r: &SqlValue) -> SqlValue {
    match (truthy_opt(l), truthy_opt(r)) {
        (Some(true), _) | (_, Some(true)) => SqlValue::Integer(1),
        (Some(false), Some(false)) => SqlValue::Integer(0),
        _ => SqlValue::Null,
    }
}

fn logical_not(v: &SqlValue) -> SqlValue {
    match truthy_opt(v) {
        Some(b) => SqlValue::Integer(if !b { 1 } else { 0 }),
        None => SqlValue::Null,
    }
}

fn concat(l: &SqlValue, r: &SqlValue) -> SqlValue {
    if matches!(l, SqlValue::Null) || matches!(r, SqlValue::Null) {
        return SqlValue::Null;
    }
    SqlValue::Text(Arc::from(format!(
        "{}{}",
        value_to_string(l),
        value_to_string(r)
    )))
}

fn value_to_string(v: &SqlValue) -> String {
    match v {
        SqlValue::Null => String::new(),
        SqlValue::Integer(n) => n.to_string(),
        SqlValue::Real(f) => {
            // Match AST evaluator's concat path: it routes through
            // `value_to_string` which delegates to the same SQLite-style
            // formatter. Re-use the public helper.
            crate::format_real_sqlite(*f)
        }
        SqlValue::Text(s) => s.as_ref().to_owned(),
        SqlValue::Blob(b) => String::from_utf8_lossy(b).into_owned(),
    }
}

fn numeric_value(v: &SqlValue) -> Result<f64> {
    match v {
        SqlValue::Null => Ok(0.0),
        SqlValue::Integer(v) => Ok(*v as f64),
        SqlValue::Real(v) => Ok(*v),
        SqlValue::Text(v) => v.trim().parse::<f64>().map_err(|_| Error::DatatypeMismatch),
        SqlValue::Blob(v) => String::from_utf8_lossy(v)
            .trim()
            .parse::<f64>()
            .map_err(|_| Error::DatatypeMismatch),
    }
}

fn arith(
    left: SqlValue,
    right: SqlValue,
    int_op: impl FnOnce(i64, i64) -> Option<i64>,
    real_op: impl FnOnce(f64, f64) -> Option<f64>,
) -> Result<SqlValue> {
    if matches!(left, SqlValue::Null) || matches!(right, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    fn lift_int(opt: Option<i64>) -> SqlValue {
        match opt {
            Some(v) => SqlValue::Integer(v),
            None => SqlValue::Null,
        }
    }
    fn lift_real(opt: Option<f64>) -> SqlValue {
        match opt {
            Some(v) => SqlValue::Real(v),
            None => SqlValue::Null,
        }
    }
    match (left, right) {
        (SqlValue::Integer(a), SqlValue::Integer(b)) => Ok(lift_int(int_op(a, b))),
        (SqlValue::Integer(a), SqlValue::Real(b)) => Ok(lift_real(real_op(a as f64, b))),
        (SqlValue::Real(a), SqlValue::Integer(b)) => Ok(lift_real(real_op(a, b as f64))),
        (SqlValue::Real(a), SqlValue::Real(b)) => Ok(lift_real(real_op(a, b))),
        (SqlValue::Text(a), SqlValue::Text(b)) => {
            let a = a
                .trim()
                .parse::<f64>()
                .map_err(|_| Error::DatatypeMismatch)?;
            let b = b
                .trim()
                .parse::<f64>()
                .map_err(|_| Error::DatatypeMismatch)?;
            Ok(lift_real(real_op(a, b)))
        }
        _ => Err(Error::DatatypeMismatch),
    }
}

fn negate(v: SqlValue) -> Result<SqlValue> {
    match v {
        SqlValue::Null => Ok(SqlValue::Null),
        SqlValue::Integer(n) => Ok(SqlValue::Integer(0i64.wrapping_sub(n))),
        SqlValue::Real(f) => Ok(SqlValue::Real(-f)),
        _ => Err(Error::DatatypeMismatch),
    }
}

fn call_scalar1(f: ScalarFn, v: SqlValue) -> Result<SqlValue> {
    match f {
        ScalarFn::Abs => match v {
            SqlValue::Null => Ok(SqlValue::Null),
            SqlValue::Integer(n) => Ok(SqlValue::Integer(n.wrapping_abs())),
            SqlValue::Real(f) => Ok(SqlValue::Real(f.abs())),
            other => {
                let n = numeric_value(&other)?;
                Ok(SqlValue::Real(n.abs()))
            }
        },
        ScalarFn::Length => match v {
            SqlValue::Null => Ok(SqlValue::Null),
            SqlValue::Text(s) => Ok(SqlValue::Integer(s.chars().count() as i64)),
            SqlValue::Blob(b) => Ok(SqlValue::Integer(b.len() as i64)),
            SqlValue::Integer(n) => Ok(SqlValue::Integer(n.to_string().len() as i64)),
            SqlValue::Real(f) => Ok(SqlValue::Integer(crate::format_real_sqlite(f).len() as i64)),
        },
        ScalarFn::Lower => match v {
            SqlValue::Null => Ok(SqlValue::Null),
            SqlValue::Text(s) => Ok(SqlValue::Text(Arc::from(s.to_lowercase()))),
            other => Ok(SqlValue::Text(Arc::from(
                value_to_string(&other).to_lowercase(),
            ))),
        },
        ScalarFn::Upper => match v {
            SqlValue::Null => Ok(SqlValue::Null),
            SqlValue::Text(s) => Ok(SqlValue::Text(Arc::from(s.to_uppercase()))),
            other => Ok(SqlValue::Text(Arc::from(
                value_to_string(&other).to_uppercase(),
            ))),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlparser::dialect::GenericDialect;
    use sqlparser::parser::Parser;

    fn parse_expr(sql: &str) -> Expr {
        let mut stmt_sql = String::with_capacity(7 + sql.len());
        stmt_sql.push_str("SELECT ");
        stmt_sql.push_str(sql);
        let stmts = Parser::parse_sql(&GenericDialect {}, &stmt_sql).expect("parse");
        let sqlparser::ast::Statement::Query(q) = &stmts[0] else {
            panic!("expected query")
        };
        let sqlparser::ast::SetExpr::Select(sel) = q.body.as_ref() else {
            panic!("expected select")
        };
        match &sel.projection[0] {
            sqlparser::ast::SelectItem::UnnamedExpr(e) => e.clone(),
            sqlparser::ast::SelectItem::ExprWithAlias { expr, .. } => expr.clone(),
            other => panic!("unexpected projection: {other:?}"),
        }
    }

    fn run_eval(sql: &str) -> SqlValue {
        let expr = parse_expr(sql);
        let ctx = CompileCtx::empty();
        let prog = compile(&expr, &ctx).unwrap().expect("compile");
        evaluate(&prog, &[], &[]).unwrap()
    }

    #[test]
    fn literal_integer() {
        assert_eq!(run_eval("42"), SqlValue::Integer(42));
    }

    #[test]
    fn literal_null() {
        assert_eq!(run_eval("NULL"), SqlValue::Null);
    }

    #[test]
    fn add_two_integers() {
        assert_eq!(run_eval("1 + 2"), SqlValue::Integer(3));
    }

    #[test]
    fn case_when_branch() {
        assert_eq!(
            run_eval("CASE WHEN 1 THEN 'yes' ELSE 'no' END"),
            SqlValue::Text(Arc::from("yes"))
        );
    }
}
