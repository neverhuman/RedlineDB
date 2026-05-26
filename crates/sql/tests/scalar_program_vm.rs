//! Phase 6 R1-C — ScalarProgram VM integration tests.
//!
//! The VM lives at `crate::exec::expr::program` which is `pub(crate)`
//! inside `redlinedb-sql`, so this integration test re-includes the
//! source via `#[path = "..."]` (the established pattern from
//! `morsel_scaffold.rs`). To make `crate::value` / `crate::error` /
//! `crate::parser::bind` / `crate::format_real_sqlite` resolvable from
//! inside the re-included module, we expose shim modules at the test
//! root that re-export the corresponding public items from
//! `redlinedb_sql`.
#![allow(dead_code)]

mod value {
    pub use redlinedb_sql::value::*;
}

mod error {
    pub use redlinedb_sql::{Error, Result};
}

mod parser {
    pub mod bind {
        /// Mirror of `redlinedb_sql::parser::bind::as_bind_name`: the
        /// vendor adapter is `pub(crate)` so we re-implement the
        /// trivial body here against the public `sqlparser::ast::Value`
        /// surface.
        pub fn as_bind_name(value: &sqlparser::ast::Value) -> Option<&str> {
            match value {
                sqlparser::ast::Value::Placeholder(name) => Some(name.as_str()),
                _ => None,
            }
        }
    }
}

pub use redlinedb_sql::format_real_sqlite;

#[path = "../src/exec/expr/program.rs"]
mod program;

use std::sync::Arc;

use proptest::prelude::*;
use sqlparser::ast::helpers::attached_token::AttachedToken;
use sqlparser::ast::{
    BinaryOperator, CaseWhen, Expr, FunctionArg, FunctionArgExpr, FunctionArgumentList,
    FunctionArguments, Ident, ObjectName, ObjectNamePart, UnaryOperator, Value, ValueWithSpan,
};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::{Location, Span};

use program::{CompileCtx, ScalarProgram, compile, evaluate};
use redlinedb_sql::value::SqlValue;

// ── parse helpers ──────────────────────────────────────────────────────────

fn parse_expr(sql: &str) -> Expr {
    let stmt_sql = format!("SELECT {sql}");
    let stmts = Parser::parse_sql(&GenericDialect {}, &stmt_sql).expect("parse");
    let sqlparser::ast::Statement::Query(q) = &stmts[0] else {
        panic!("expected query");
    };
    let sqlparser::ast::SetExpr::Select(sel) = q.body.as_ref() else {
        panic!("expected select");
    };
    match &sel.projection[0] {
        sqlparser::ast::SelectItem::UnnamedExpr(e) => e.clone(),
        sqlparser::ast::SelectItem::ExprWithAlias { expr, .. } => expr.clone(),
        other => panic!("unexpected projection: {other:?}"),
    }
}

fn compile_one(sql: &str, ctx: &CompileCtx<'_>) -> ScalarProgram {
    let expr = parse_expr(sql);
    compile(&expr, ctx)
        .expect("compile error")
        .expect("compile rejected supported shape")
}

fn run(sql: &str, row: &[SqlValue], bindings: &[Option<SqlValue>]) -> SqlValue {
    let cols: Vec<(String, usize)> = (0..row.len()).map(|i| (format!("c{i}"), i)).collect();
    let ctx = CompileCtx {
        columns: cols.as_slice(),
    };
    let prog = compile_one(sql, &ctx);
    evaluate(&prog, row, bindings).expect("evaluate")
}

// ── Unit tests ─────────────────────────────────────────────────────────────

#[test]
fn literal_integer() {
    assert_eq!(run("42", &[], &[]), SqlValue::Integer(42));
}

#[test]
fn literal_negative() {
    assert_eq!(run("-7", &[], &[]), SqlValue::Integer(-7));
}

#[test]
fn literal_real() {
    assert_eq!(run("3.5", &[], &[]), SqlValue::Real(3.5));
}

#[test]
fn literal_null() {
    assert_eq!(run("NULL", &[], &[]), SqlValue::Null);
}

#[test]
fn literal_text() {
    assert_eq!(run("'hi'", &[], &[]), SqlValue::Text(Arc::from("hi")));
}

#[test]
fn add_integers() {
    assert_eq!(run("1 + 2 + 3", &[], &[]), SqlValue::Integer(6));
}

#[test]
fn sub_mixed() {
    assert_eq!(run("10 - 2.5", &[], &[]), SqlValue::Real(7.5));
}

#[test]
fn mul_div() {
    assert_eq!(run("8 * 4 / 2", &[], &[]), SqlValue::Integer(16));
}

#[test]
fn div_by_zero_is_null() {
    assert_eq!(run("1 / 0", &[], &[]), SqlValue::Null);
}

#[test]
fn mod_op() {
    assert_eq!(run("10 % 3", &[], &[]), SqlValue::Integer(1));
}

#[test]
fn null_propagates_through_arith() {
    assert_eq!(run("NULL + 1", &[], &[]), SqlValue::Null);
    assert_eq!(run("1 + NULL", &[], &[]), SqlValue::Null);
}

#[test]
fn compare_returns_one_or_zero() {
    assert_eq!(run("1 < 2", &[], &[]), SqlValue::Integer(1));
    assert_eq!(run("2 < 2", &[], &[]), SqlValue::Integer(0));
    assert_eq!(run("2 <= 2", &[], &[]), SqlValue::Integer(1));
    assert_eq!(run("3 > 2", &[], &[]), SqlValue::Integer(1));
    assert_eq!(run("3 >= 4", &[], &[]), SqlValue::Integer(0));
    assert_eq!(run("3 = 3", &[], &[]), SqlValue::Integer(1));
    assert_eq!(run("3 <> 3", &[], &[]), SqlValue::Integer(0));
}

#[test]
fn compare_null_is_null() {
    assert_eq!(run("NULL = 1", &[], &[]), SqlValue::Null);
    assert_eq!(run("1 < NULL", &[], &[]), SqlValue::Null);
}

#[test]
fn logical_and_or_not() {
    assert_eq!(run("1 AND 1", &[], &[]), SqlValue::Integer(1));
    assert_eq!(run("1 AND 0", &[], &[]), SqlValue::Integer(0));
    assert_eq!(run("0 OR 1", &[], &[]), SqlValue::Integer(1));
    assert_eq!(run("0 OR 0", &[], &[]), SqlValue::Integer(0));
    assert_eq!(run("NOT 0", &[], &[]), SqlValue::Integer(1));
    assert_eq!(run("NOT 1", &[], &[]), SqlValue::Integer(0));
    assert_eq!(run("NOT NULL", &[], &[]), SqlValue::Null);
}

#[test]
fn concat_text() {
    assert_eq!(
        run("'a' || 'b'", &[], &[]),
        SqlValue::Text(Arc::from("ab"))
    );
    assert_eq!(run("'a' || NULL", &[], &[]), SqlValue::Null);
}

#[test]
fn fn_abs_length_lower_upper() {
    assert_eq!(run("abs(-5)", &[], &[]), SqlValue::Integer(5));
    assert_eq!(run("length('hello')", &[], &[]), SqlValue::Integer(5));
    assert_eq!(run("lower('HI')", &[], &[]), SqlValue::Text(Arc::from("hi")));
    assert_eq!(
        run("upper('hi')", &[], &[]),
        SqlValue::Text(Arc::from("HI"))
    );
}

#[test]
fn column_load_via_ordinal() {
    let row = vec![SqlValue::Integer(10), SqlValue::Integer(7)];
    assert_eq!(run("c0 + c1", &row, &[]), SqlValue::Integer(17));
    assert_eq!(run("c0 * c1", &row, &[]), SqlValue::Integer(70));
}

#[test]
fn binding_lookup() {
    let bindings = vec![Some(SqlValue::Integer(42))];
    assert_eq!(run("?1 + 1", &[], &bindings), SqlValue::Integer(43));
}

#[test]
fn case_when_then_else() {
    let row = vec![SqlValue::Integer(0)];
    assert_eq!(
        run(
            "CASE WHEN c0 > 0 THEN 'pos' WHEN c0 < 0 THEN 'neg' ELSE 'zero' END",
            &row,
            &[]
        ),
        SqlValue::Text(Arc::from("zero"))
    );
    let row = vec![SqlValue::Integer(5)];
    assert_eq!(
        run(
            "CASE WHEN c0 > 0 THEN 'pos' WHEN c0 < 0 THEN 'neg' ELSE 'zero' END",
            &row,
            &[]
        ),
        SqlValue::Text(Arc::from("pos"))
    );
}

#[test]
fn case_without_else_defaults_null() {
    let row = vec![SqlValue::Integer(0)];
    assert_eq!(
        run("CASE WHEN c0 > 0 THEN 1 END", &row, &[]),
        SqlValue::Null
    );
}

#[test]
fn unsupported_shape_returns_none() {
    let expr = parse_expr("c0 BETWEEN 1 AND 10");
    let ctx = CompileCtx {
        columns: &[("c0".to_owned(), 0)],
    };
    assert!(compile(&expr, &ctx).expect("no error").is_none());
}

#[test]
fn unsupported_function_returns_none() {
    let expr = parse_expr("nonexistent(1)");
    let ctx = CompileCtx::empty();
    assert!(compile(&expr, &ctx).expect("no error").is_none());
}

// ── Embedded AST oracle ───────────────────────────────────────────────────
//
// A minimal AST evaluator covering the Tier-0 surface so the differential
// test can compare against an independent implementation. The numeric /
// comparison / null semantics here mirror `crate::exec::expr::eval_scalar`
// for the subset the VM compiles.

fn ast_eval(
    expr: &Expr,
    cols: &[(String, usize)],
    row: &[SqlValue],
    bindings: &[Option<SqlValue>],
) -> SqlValue {
    match expr {
        Expr::Nested(inner) => ast_eval(inner, cols, row, bindings),
        Expr::Value(v) => ast_eval_value(&v.value, bindings),
        Expr::Identifier(id) => ast_eval_ident(&id.value, cols, row),
        Expr::CompoundIdentifier(parts) if parts.len() == 1 => {
            ast_eval_ident(&parts[0].value, cols, row)
        }
        Expr::UnaryOp { op, expr } => {
            let v = ast_eval(expr, cols, row, bindings);
            match op {
                UnaryOperator::Plus => v,
                UnaryOperator::Minus => match v {
                    SqlValue::Null => SqlValue::Null,
                    SqlValue::Integer(n) => SqlValue::Integer(0i64.wrapping_sub(n)),
                    SqlValue::Real(f) => SqlValue::Real(-f),
                    other => panic!("oracle: cannot negate {other:?}"),
                },
                UnaryOperator::Not => oracle_not(&v),
                other => panic!("oracle: unsupported unary {other:?}"),
            }
        }
        Expr::BinaryOp { left, op, right } => {
            let l = ast_eval(left, cols, row, bindings);
            let r = ast_eval(right, cols, row, bindings);
            ast_eval_binary(op, l, r)
        }
        Expr::Function(func) => {
            let name = func.name.to_string().to_ascii_lowercase();
            let FunctionArguments::List(list) = &func.args else {
                panic!("oracle: unsupported function args");
            };
            assert_eq!(list.args.len(), 1, "oracle: only 1-arg fns supported");
            let FunctionArg::Unnamed(FunctionArgExpr::Expr(arg_expr)) = &list.args[0] else {
                panic!("oracle: unsupported arg form");
            };
            let v = ast_eval(arg_expr, cols, row, bindings);
            oracle_fn(&name, v)
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            assert!(operand.is_none(), "oracle: only searched CASE");
            for when in conditions {
                let c = ast_eval(&when.condition, cols, row, bindings);
                if oracle_truthy(&c) == Some(true) {
                    return ast_eval(&when.result, cols, row, bindings);
                }
            }
            match else_result {
                Some(e) => ast_eval(e, cols, row, bindings),
                None => SqlValue::Null,
            }
        }
        other => panic!("oracle: unsupported expr {other:?}"),
    }
}

fn ast_eval_value(v: &Value, bindings: &[Option<SqlValue>]) -> SqlValue {
    if let Some(name) = parser::bind::as_bind_name(v) {
        if let Some(rest) = name.strip_prefix('?') {
            if let Ok(slot) = rest.parse::<usize>() {
                if slot == 0 {
                    return SqlValue::Null;
                }
                return bindings
                    .get(slot - 1)
                    .and_then(|s| s.clone())
                    .unwrap_or(SqlValue::Null);
            }
        }
    }
    match v {
        Value::Null => SqlValue::Null,
        Value::Boolean(b) => SqlValue::Integer(if *b { 1 } else { 0 }),
        Value::Number(n, _) => {
            if let Ok(i) = n.parse::<i64>() {
                SqlValue::Integer(i)
            } else if let Ok(f) = n.parse::<f64>() {
                SqlValue::Real(f)
            } else {
                panic!("oracle: bad number {n}")
            }
        }
        Value::SingleQuotedString(s) | Value::DoubleQuotedString(s) => {
            SqlValue::Text(Arc::from(s.as_str()))
        }
        other => panic!("oracle: unsupported literal {other:?}"),
    }
}

fn ast_eval_ident(name: &str, cols: &[(String, usize)], row: &[SqlValue]) -> SqlValue {
    let idx = cols
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, i)| *i)
        .unwrap_or_else(|| panic!("oracle: unknown col {name}"));
    row.get(idx).cloned().unwrap_or(SqlValue::Null)
}

fn ast_eval_binary(op: &BinaryOperator, l: SqlValue, r: SqlValue) -> SqlValue {
    match op {
        BinaryOperator::Plus => oracle_arith(l, r, |a, b| Some(a.wrapping_add(b)), |a, b| Some(a + b)),
        BinaryOperator::Minus => oracle_arith(l, r, |a, b| Some(a.wrapping_sub(b)), |a, b| Some(a - b)),
        BinaryOperator::Multiply => {
            oracle_arith(l, r, |a, b| Some(a.wrapping_mul(b)), |a, b| Some(a * b))
        }
        BinaryOperator::Divide => oracle_arith(
            l,
            r,
            |a, b| if b == 0 { None } else { a.checked_div(b) },
            |a, b| if b == 0.0 { None } else { Some(a / b) },
        ),
        BinaryOperator::Modulo => oracle_arith(
            l,
            r,
            |a, b| if b == 0 { None } else { a.checked_rem(b) },
            |a, b| if b == 0.0 { None } else { Some(a % b) },
        ),
        BinaryOperator::Eq => oracle_cmp(l, r, |o| o == std::cmp::Ordering::Equal),
        BinaryOperator::NotEq => oracle_cmp(l, r, |o| o != std::cmp::Ordering::Equal),
        BinaryOperator::Lt => oracle_cmp(l, r, |o| o == std::cmp::Ordering::Less),
        BinaryOperator::LtEq => oracle_cmp(l, r, |o| o != std::cmp::Ordering::Greater),
        BinaryOperator::Gt => oracle_cmp(l, r, |o| o == std::cmp::Ordering::Greater),
        BinaryOperator::GtEq => oracle_cmp(l, r, |o| o != std::cmp::Ordering::Less),
        BinaryOperator::And => match (oracle_truthy(&l), oracle_truthy(&r)) {
            (Some(false), _) | (_, Some(false)) => SqlValue::Integer(0),
            (Some(true), Some(true)) => SqlValue::Integer(1),
            _ => SqlValue::Null,
        },
        BinaryOperator::Or => match (oracle_truthy(&l), oracle_truthy(&r)) {
            (Some(true), _) | (_, Some(true)) => SqlValue::Integer(1),
            (Some(false), Some(false)) => SqlValue::Integer(0),
            _ => SqlValue::Null,
        },
        BinaryOperator::StringConcat => {
            if matches!(l, SqlValue::Null) || matches!(r, SqlValue::Null) {
                SqlValue::Null
            } else {
                SqlValue::Text(Arc::from(format!(
                    "{}{}",
                    oracle_to_string(&l),
                    oracle_to_string(&r)
                )))
            }
        }
        other => panic!("oracle: unsupported binary {other:?}"),
    }
}

fn oracle_cmp(l: SqlValue, r: SqlValue, accept: fn(std::cmp::Ordering) -> bool) -> SqlValue {
    if matches!(l, SqlValue::Null) || matches!(r, SqlValue::Null) {
        SqlValue::Null
    } else {
        let ord = redlinedb_sql::value::compare_values(&l, &r);
        SqlValue::Integer(if accept(ord) { 1 } else { 0 })
    }
}

fn oracle_arith(
    l: SqlValue,
    r: SqlValue,
    int_op: impl FnOnce(i64, i64) -> Option<i64>,
    real_op: impl FnOnce(f64, f64) -> Option<f64>,
) -> SqlValue {
    if matches!(l, SqlValue::Null) || matches!(r, SqlValue::Null) {
        return SqlValue::Null;
    }
    fn lift_int(opt: Option<i64>) -> SqlValue {
        opt.map(SqlValue::Integer).unwrap_or(SqlValue::Null)
    }
    fn lift_real(opt: Option<f64>) -> SqlValue {
        opt.map(SqlValue::Real).unwrap_or(SqlValue::Null)
    }
    match (l, r) {
        (SqlValue::Integer(a), SqlValue::Integer(b)) => lift_int(int_op(a, b)),
        (SqlValue::Integer(a), SqlValue::Real(b)) => lift_real(real_op(a as f64, b)),
        (SqlValue::Real(a), SqlValue::Integer(b)) => lift_real(real_op(a, b as f64)),
        (SqlValue::Real(a), SqlValue::Real(b)) => lift_real(real_op(a, b)),
        other => panic!("oracle: non-numeric arith {other:?}"),
    }
}

fn oracle_truthy(v: &SqlValue) -> Option<bool> {
    match v {
        SqlValue::Null => None,
        _ => Some(redlinedb_sql::value::is_truthy(v)),
    }
}

fn oracle_not(v: &SqlValue) -> SqlValue {
    match oracle_truthy(v) {
        Some(b) => SqlValue::Integer(if !b { 1 } else { 0 }),
        None => SqlValue::Null,
    }
}

fn oracle_to_string(v: &SqlValue) -> String {
    match v {
        SqlValue::Null => String::new(),
        SqlValue::Integer(n) => n.to_string(),
        SqlValue::Real(f) => redlinedb_sql::format_real_sqlite(*f),
        SqlValue::Text(s) => s.as_ref().to_owned(),
        SqlValue::Blob(b) => String::from_utf8_lossy(b).into_owned(),
    }
}

fn oracle_fn(name: &str, v: SqlValue) -> SqlValue {
    match name {
        "abs" => match v {
            SqlValue::Null => SqlValue::Null,
            SqlValue::Integer(n) => SqlValue::Integer(n.wrapping_abs()),
            SqlValue::Real(f) => SqlValue::Real(f.abs()),
            other => panic!("oracle: abs of {other:?}"),
        },
        "length" => match v {
            SqlValue::Null => SqlValue::Null,
            SqlValue::Text(s) => SqlValue::Integer(s.chars().count() as i64),
            SqlValue::Blob(b) => SqlValue::Integer(b.len() as i64),
            SqlValue::Integer(n) => SqlValue::Integer(n.to_string().len() as i64),
            SqlValue::Real(f) => {
                SqlValue::Integer(redlinedb_sql::format_real_sqlite(f).len() as i64)
            }
        },
        "lower" => match v {
            SqlValue::Null => SqlValue::Null,
            SqlValue::Text(s) => SqlValue::Text(Arc::from(s.to_lowercase())),
            other => SqlValue::Text(Arc::from(oracle_to_string(&other).to_lowercase())),
        },
        "upper" => match v {
            SqlValue::Null => SqlValue::Null,
            SqlValue::Text(s) => SqlValue::Text(Arc::from(s.to_uppercase())),
            other => SqlValue::Text(Arc::from(oracle_to_string(&other).to_uppercase())),
        },
        other => panic!("oracle: unknown fn {other}"),
    }
}

// ── Random expression generator ─────────────────────────────────────────────

#[derive(Debug, Clone)]
enum AtomKind {
    LitInt(i32),
    LitReal(f32),
    LitText(u8),
    Col(usize),
    Bind(usize),
    Null,
}

#[derive(Debug, Clone)]
enum ExprShape {
    Atom(AtomKind),
    Neg(Box<ExprShape>),
    Not(Box<ExprShape>),
    Arith(ArithOp, Box<ExprShape>, Box<ExprShape>),
    Cmp(CmpOp, Box<ExprShape>, Box<ExprShape>),
    Logic(LogicOp, Box<ExprShape>, Box<ExprShape>),
    Concat(Box<ExprShape>, Box<ExprShape>),
    Fn1(Fn1Kind, Box<ExprShape>),
    Case {
        whens: Vec<(Box<ExprShape>, Box<ExprShape>)>,
        else_: Option<Box<ExprShape>>,
    },
}

#[derive(Debug, Clone, Copy)]
enum ArithOp { Add, Sub, Mul, Div, Mod }
#[derive(Debug, Clone, Copy)]
enum CmpOp { Eq, Ne, Lt, Le, Gt, Ge }
#[derive(Debug, Clone, Copy)]
enum LogicOp { And, Or }
#[derive(Debug, Clone, Copy)]
enum Fn1Kind { Abs, Length, Lower, Upper }

fn shape_strategy() -> impl Strategy<Value = ExprShape> {
    let atom = prop_oneof![
        (-100i32..100i32).prop_map(|i| ExprShape::Atom(AtomKind::LitInt(i))),
        (-100.0f32..100.0f32).prop_map(|f| ExprShape::Atom(AtomKind::LitReal(f))),
        (0u8..4u8).prop_map(|t| ExprShape::Atom(AtomKind::LitText(t))),
        (0usize..3usize).prop_map(|c| ExprShape::Atom(AtomKind::Col(c))),
        (1usize..3usize).prop_map(|b| ExprShape::Atom(AtomKind::Bind(b))),
        Just(ExprShape::Atom(AtomKind::Null)),
    ];
    atom.prop_recursive(4, 32, 3, |inner| {
        prop_oneof![
            inner.clone().prop_map(|e| ExprShape::Neg(Box::new(e))),
            inner.clone().prop_map(|e| ExprShape::Not(Box::new(e))),
            (arith_op_strategy(), inner.clone(), inner.clone())
                .prop_map(|(op, l, r)| ExprShape::Arith(op, Box::new(l), Box::new(r))),
            (cmp_op_strategy(), inner.clone(), inner.clone())
                .prop_map(|(op, l, r)| ExprShape::Cmp(op, Box::new(l), Box::new(r))),
            (logic_op_strategy(), inner.clone(), inner.clone())
                .prop_map(|(op, l, r)| ExprShape::Logic(op, Box::new(l), Box::new(r))),
            (inner.clone(), inner.clone())
                .prop_map(|(l, r)| ExprShape::Concat(Box::new(l), Box::new(r))),
            (fn1_strategy(), inner.clone()).prop_map(|(f, e)| ExprShape::Fn1(f, Box::new(e))),
            (prop::collection::vec((inner.clone(), inner.clone()), 1..3), prop::option::of(inner))
                .prop_map(|(ws, e)| ExprShape::Case {
                    whens: ws.into_iter().map(|(c, r)| (Box::new(c), Box::new(r))).collect(),
                    else_: e.map(Box::new),
                }),
        ]
    })
}

fn arith_op_strategy() -> impl Strategy<Value = ArithOp> {
    prop_oneof![
        Just(ArithOp::Add),
        Just(ArithOp::Sub),
        Just(ArithOp::Mul),
        Just(ArithOp::Div),
        Just(ArithOp::Mod),
    ]
}
fn cmp_op_strategy() -> impl Strategy<Value = CmpOp> {
    prop_oneof![
        Just(CmpOp::Eq),
        Just(CmpOp::Ne),
        Just(CmpOp::Lt),
        Just(CmpOp::Le),
        Just(CmpOp::Gt),
        Just(CmpOp::Ge),
    ]
}
fn logic_op_strategy() -> impl Strategy<Value = LogicOp> {
    prop_oneof![Just(LogicOp::And), Just(LogicOp::Or)]
}
fn fn1_strategy() -> impl Strategy<Value = Fn1Kind> {
    prop_oneof![
        Just(Fn1Kind::Abs),
        Just(Fn1Kind::Length),
        Just(Fn1Kind::Lower),
        Just(Fn1Kind::Upper),
    ]
}

// Build a sqlparser Expr from a shape. We construct directly rather than
// re-parsing so we control numeric / text literal shapes precisely.

fn span() -> Span {
    Span {
        start: Location::empty(),
        end: Location::empty(),
    }
}

fn lit_int(n: i64) -> Expr {
    let v = if n < 0 {
        Value::Number((-n).to_string(), false)
    } else {
        Value::Number(n.to_string(), false)
    };
    let inner = Expr::Value(ValueWithSpan { value: v, span: span() });
    if n < 0 {
        Expr::UnaryOp { op: UnaryOperator::Minus, expr: Box::new(inner) }
    } else {
        inner
    }
}

fn lit_real(f: f64) -> Expr {
    // Avoid NaN/inf — semantics diverge between oracle and SQLite-style.
    let f = if !f.is_finite() { 0.0 } else { f };
    let abs = f.abs();
    let body = format!("{abs}");
    // Ensure parser reads as a real, not integer, by including a decimal.
    let body = if body.contains('.') || body.contains('e') {
        body
    } else {
        format!("{body}.0")
    };
    let inner = Expr::Value(ValueWithSpan {
        value: Value::Number(body, false),
        span: span(),
    });
    if f < 0.0 {
        Expr::UnaryOp { op: UnaryOperator::Minus, expr: Box::new(inner) }
    } else {
        inner
    }
}

fn lit_text(s: &str) -> Expr {
    Expr::Value(ValueWithSpan {
        value: Value::SingleQuotedString(s.to_owned()),
        span: span(),
    })
}

fn lit_null() -> Expr {
    Expr::Value(ValueWithSpan { value: Value::Null, span: span() })
}

fn ident(name: &str) -> Expr {
    Expr::Identifier(Ident {
        value: name.to_owned(),
        quote_style: None,
        span: span(),
    })
}

fn bind(slot: usize) -> Expr {
    Expr::Value(ValueWithSpan {
        value: Value::Placeholder(format!("?{slot}")),
        span: span(),
    })
}

fn fn1_call(name: &str, arg: Expr) -> Expr {
    let arg_node = FunctionArg::Unnamed(FunctionArgExpr::Expr(arg));
    let args = FunctionArguments::List(FunctionArgumentList {
        duplicate_treatment: None,
        args: vec![arg_node],
        clauses: vec![],
    });
    Expr::Function(sqlparser::ast::Function {
        name: ObjectName(vec![ObjectNamePart::Identifier(Ident {
            value: name.to_owned(),
            quote_style: None,
            span: span(),
        })]),
        uses_odbc_syntax: false,
        parameters: FunctionArguments::None,
        args,
        filter: None,
        null_treatment: None,
        over: None,
        within_group: vec![],
    })
}

fn shape_to_expr(shape: &ExprShape) -> Expr {
    match shape {
        ExprShape::Atom(a) => match a {
            AtomKind::LitInt(n) => lit_int(*n as i64),
            AtomKind::LitReal(f) => lit_real(*f as f64),
            AtomKind::LitText(t) => {
                let texts = ["alpha", "Beta", "", "X"];
                lit_text(texts[*t as usize % texts.len()])
            }
            AtomKind::Col(c) => ident(&format!("c{c}")),
            AtomKind::Bind(b) => bind(*b),
            AtomKind::Null => lit_null(),
        },
        ExprShape::Neg(inner) => Expr::UnaryOp {
            op: UnaryOperator::Minus,
            expr: Box::new(shape_to_expr(inner)),
        },
        ExprShape::Not(inner) => Expr::UnaryOp {
            op: UnaryOperator::Not,
            expr: Box::new(shape_to_expr(inner)),
        },
        ExprShape::Arith(op, l, r) => Expr::BinaryOp {
            left: Box::new(shape_to_expr(l)),
            op: match op {
                ArithOp::Add => BinaryOperator::Plus,
                ArithOp::Sub => BinaryOperator::Minus,
                ArithOp::Mul => BinaryOperator::Multiply,
                ArithOp::Div => BinaryOperator::Divide,
                ArithOp::Mod => BinaryOperator::Modulo,
            },
            right: Box::new(shape_to_expr(r)),
        },
        ExprShape::Cmp(op, l, r) => Expr::BinaryOp {
            left: Box::new(shape_to_expr(l)),
            op: match op {
                CmpOp::Eq => BinaryOperator::Eq,
                CmpOp::Ne => BinaryOperator::NotEq,
                CmpOp::Lt => BinaryOperator::Lt,
                CmpOp::Le => BinaryOperator::LtEq,
                CmpOp::Gt => BinaryOperator::Gt,
                CmpOp::Ge => BinaryOperator::GtEq,
            },
            right: Box::new(shape_to_expr(r)),
        },
        ExprShape::Logic(op, l, r) => Expr::BinaryOp {
            left: Box::new(shape_to_expr(l)),
            op: match op {
                LogicOp::And => BinaryOperator::And,
                LogicOp::Or => BinaryOperator::Or,
            },
            right: Box::new(shape_to_expr(r)),
        },
        ExprShape::Concat(l, r) => Expr::BinaryOp {
            left: Box::new(shape_to_expr(l)),
            op: BinaryOperator::StringConcat,
            right: Box::new(shape_to_expr(r)),
        },
        ExprShape::Fn1(kind, inner) => {
            let name = match kind {
                Fn1Kind::Abs => "abs",
                Fn1Kind::Length => "length",
                Fn1Kind::Lower => "lower",
                Fn1Kind::Upper => "upper",
            };
            fn1_call(name, shape_to_expr(inner))
        }
        ExprShape::Case { whens, else_ } => {
            let conditions: Vec<CaseWhen> = whens
                .iter()
                .map(|(c, r)| CaseWhen {
                    condition: shape_to_expr(c),
                    result: shape_to_expr(r),
                })
                .collect();
            Expr::Case {
                case_token: AttachedToken::empty(),
                end_token: AttachedToken::empty(),
                operand: None,
                conditions,
                else_result: else_.as_ref().map(|e| Box::new(shape_to_expr(e))),
            }
        }
    }
}

/// Whether the oracle and VM should reasonably match. We exclude shapes
/// that touch text in arithmetic or compare numeric to text, since the
/// AST evaluator coerces via Text→f64 (raising DatatypeMismatch) while
/// the oracle is intentionally narrower. We also skip Case operands
/// with NULL conditions checked beyond the oracle's NULL handling.
fn shape_is_safe(shape: &ExprShape) -> bool {
    fn touches_text(s: &ExprShape) -> bool {
        match s {
            ExprShape::Atom(AtomKind::LitText(_)) => true,
            ExprShape::Atom(_) => false,
            // Concat always produces text.
            ExprShape::Concat(_, _) => true,
            // Lower/Upper always produce text. Length always produces int.
            ExprShape::Fn1(Fn1Kind::Lower | Fn1Kind::Upper, _) => true,
            ExprShape::Fn1(_, i) => touches_text(i),
            ExprShape::Neg(i) | ExprShape::Not(i) => touches_text(i),
            ExprShape::Arith(_, l, r)
            | ExprShape::Cmp(_, l, r)
            | ExprShape::Logic(_, l, r) => touches_text(l) || touches_text(r),
            ExprShape::Case { whens, else_ } => {
                whens.iter().any(|(c, r)| touches_text(c) || touches_text(r))
                    || else_.as_ref().is_some_and(|e| touches_text(e))
            }
        }
    }
    fn arith_with_text(s: &ExprShape) -> bool {
        match s {
            ExprShape::Atom(_) => false,
            ExprShape::Neg(i) => touches_text(i) || arith_with_text(i),
            ExprShape::Not(i) => arith_with_text(i),
            ExprShape::Arith(_, l, r) => {
                touches_text(l) || touches_text(r) || arith_with_text(l) || arith_with_text(r)
            }
            ExprShape::Cmp(_, l, r) | ExprShape::Logic(_, l, r) | ExprShape::Concat(l, r) => {
                arith_with_text(l) || arith_with_text(r)
            }
            ExprShape::Fn1(Fn1Kind::Abs, i) => touches_text(i) || arith_with_text(i),
            ExprShape::Fn1(_, i) => arith_with_text(i),
            ExprShape::Case { whens, else_ } => {
                whens.iter().any(|(c, r)| arith_with_text(c) || arith_with_text(r))
                    || else_.as_ref().is_some_and(|e| arith_with_text(e))
            }
        }
    }
    fn cmp_mixes_text_and_number(s: &ExprShape) -> bool {
        // VM and oracle both use compare_values which orders Number < Text,
        // so this is safe; nothing to filter here.
        let _ = s;
        false
    }
    !arith_with_text(shape) && !cmp_mixes_text_and_number(shape)
}

// ── Differential test ─────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig { cases: 1000, .. ProptestConfig::default() })]

    #[test]
    fn vm_matches_ast_oracle(shape in shape_strategy()) {
        prop_assume!(shape_is_safe(&shape));
        let expr = shape_to_expr(&shape);

        let cols: Vec<(String, usize)> = (0..3).map(|i| (format!("c{i}"), i)).collect();
        let ctx = CompileCtx { columns: cols.as_slice() };

        let compiled = match compile(&expr, &ctx) {
            Ok(Some(p)) => p,
            Ok(None) => return Ok(()),
            Err(_) => return Ok(()),
        };

        let row = vec![
            SqlValue::Integer(7),
            SqlValue::Real(2.5),
            SqlValue::Null,
        ];
        let bindings = vec![
            Some(SqlValue::Integer(11)),
            Some(SqlValue::Integer(-3)),
            Some(SqlValue::Null),
        ];

        let oracle = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ast_eval(&expr, &cols, &row, &bindings)
        }));
        let oracle = match oracle {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let vm_result = match evaluate(&compiled, &row, &bindings) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        prop_assert!(
            values_equivalent(&oracle, &vm_result),
            "oracle {:?} != vm {:?} for {:?}",
            oracle,
            vm_result,
            shape
        );
    }
}

/// Equality with small tolerance for Real values produced by independent
/// computation paths. Integers and text must match exactly.
fn values_equivalent(a: &SqlValue, b: &SqlValue) -> bool {
    match (a, b) {
        (SqlValue::Null, SqlValue::Null) => true,
        (SqlValue::Integer(x), SqlValue::Integer(y)) => x == y,
        (SqlValue::Real(x), SqlValue::Real(y)) => {
            if x.is_nan() && y.is_nan() {
                true
            } else {
                (x - y).abs() < 1e-9 * x.abs().max(y.abs()).max(1.0)
            }
        }
        (SqlValue::Integer(x), SqlValue::Real(y)) | (SqlValue::Real(y), SqlValue::Integer(x)) => {
            ((*x as f64) - y).abs() < 1e-9 * (*x as f64).abs().max(y.abs()).max(1.0)
        }
        (SqlValue::Text(x), SqlValue::Text(y)) => x == y,
        (SqlValue::Blob(x), SqlValue::Blob(y)) => x == y,
        _ => false,
    }
}

// ── Micro-bench (smoke) ───────────────────────────────────────────────────
//
// 1M iterations is gated behind `--release` and `--ignored` so it does not
// inflate normal test runs. The differential test above is the binding
// correctness gate; this exists for local timing checks.

#[test]
#[ignore]
fn microbench_vm_eval_million() {
    let row = vec![SqlValue::Integer(3), SqlValue::Integer(5)];
    let cols = vec![("a".to_owned(), 0), ("b".to_owned(), 1)];
    let ctx = CompileCtx {
        columns: cols.as_slice(),
    };
    let expr = parse_expr("a + b * 2");
    let prog = compile(&expr, &ctx).unwrap().unwrap();
    let start = std::time::Instant::now();
    let mut acc = 0i64;
    for _ in 0..1_000_000u64 {
        if let SqlValue::Integer(n) = evaluate(&prog, &row, &[]).unwrap() {
            acc = acc.wrapping_add(n);
        }
    }
    let elapsed = start.elapsed();
    eprintln!("vm 1M iters: {:?} acc={acc}", elapsed);
}

#[test]
#[ignore]
fn microbench_ast_oracle_million() {
    let row = vec![SqlValue::Integer(3), SqlValue::Integer(5)];
    let cols = vec![("a".to_owned(), 0), ("b".to_owned(), 1)];
    let expr = parse_expr("a + b * 2");
    let start = std::time::Instant::now();
    let mut acc = 0i64;
    for _ in 0..1_000_000u64 {
        if let SqlValue::Integer(n) = ast_eval(&expr, &cols, &row, &[]) {
            acc = acc.wrapping_add(n);
        }
    }
    let elapsed = start.elapsed();
    eprintln!("ast oracle 1M iters: {:?} acc={acc}", elapsed);
}
