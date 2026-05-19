use std::sync::Arc;

use redlinedb_kernel::catalog::{DbName, ExprAst, OwnedValue, QualifiedName};
use sqlparser::ast::{BinaryOperator, Expr, ObjectName, UnaryOperator, Value, ValueWithSpan};

use crate::error::{Error, Result};

use super::table::object_name_part_to_string;

pub(crate) fn expr_to_kernel_ast(
    expr: &Expr,
    column_lookup: &std::collections::HashMap<String, usize>,
) -> Result<ExprAst> {
    Ok(match expr {
        Expr::Value(v) => sql_value_to_kernel_ast(v)?,
        Expr::Identifier(ident) => {
            ExprAst::Column(resolve_column_ordinal_name(column_lookup, &ident.value)? as u16)
        }
        Expr::CompoundIdentifier(parts) => {
            let last = match parts.last() {
                Some(p) => p,
                None => return Err(Error::UnknownColumn("empty identifier".to_owned())),
            };
            ExprAst::Column(resolve_column_ordinal_name(column_lookup, last.value.as_str())? as u16)
        }
        Expr::Nested(expr) => expr_to_kernel_ast(expr, column_lookup)?,
        Expr::UnaryOp { op, expr } => match op {
            UnaryOperator::Not => ExprAst::Not(Box::new(expr_to_kernel_ast(expr, column_lookup)?)),
            UnaryOperator::Plus => expr_to_kernel_ast(expr, column_lookup)?,
            UnaryOperator::Minus => {
                return Err(Error::UnsupportedSql(
                    "negative numeric expressions are not supported in DDL".to_owned(),
                ));
            }
            _ => {
                return Err(Error::UnsupportedSql(format!(
                    "unsupported unary operator in DDL: {op:?}"
                )));
            }
        },
        Expr::BinaryOp { left, op, right } => match op {
            BinaryOperator::And => ExprAst::And(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            BinaryOperator::Or => ExprAst::Or(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            BinaryOperator::Eq => ExprAst::Eq(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            BinaryOperator::NotEq | BinaryOperator::Spaceship => ExprAst::Ne(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            BinaryOperator::Lt => ExprAst::Lt(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            BinaryOperator::LtEq => ExprAst::Le(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            BinaryOperator::Gt => ExprAst::Gt(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            BinaryOperator::GtEq => ExprAst::Ge(
                Box::new(expr_to_kernel_ast(left, column_lookup)?),
                Box::new(expr_to_kernel_ast(right, column_lookup)?),
            ),
            _ => {
                return Err(Error::UnsupportedSql(format!(
                    "unsupported binary operator in DDL: {op:?}"
                )));
            }
        },
        Expr::IsNull(expr) => ExprAst::Eq(
            Box::new(expr_to_kernel_ast(expr, column_lookup)?),
            Box::new(ExprAst::Const(OwnedValue::Null)),
        ),
        Expr::IsNotNull(expr) => ExprAst::Ne(
            Box::new(expr_to_kernel_ast(expr, column_lookup)?),
            Box::new(ExprAst::Const(OwnedValue::Null)),
        ),
        Expr::Cast { expr, .. } => expr_to_kernel_ast(expr, column_lookup)?,
        Expr::InList {
            expr,
            list,
            negated,
        } => in_list_to_kernel_ast(expr, list, *negated, column_lookup)?,
        other => {
            return Err(Error::UnsupportedSql(format!(
                "unsupported DDL expression: {other:?}"
            )));
        }
    })
}

fn in_list_to_kernel_ast(
    expr: &Expr,
    list: &[Expr],
    negated: bool,
    column_lookup: &std::collections::HashMap<String, usize>,
) -> Result<ExprAst> {
    let left = expr_to_kernel_ast(expr, column_lookup)?;
    let Some((first, rest)) = list.split_first() else {
        return Ok(ExprAst::Const(OwnedValue::Integer(if negated {
            1
        } else {
            0
        })));
    };
    let mut acc = in_list_term(&left, first, negated, column_lookup)?;
    for item in rest {
        let term = in_list_term(&left, item, negated, column_lookup)?;
        acc = if negated {
            ExprAst::And(Box::new(acc), Box::new(term))
        } else {
            ExprAst::Or(Box::new(acc), Box::new(term))
        };
    }
    Ok(acc)
}

fn in_list_term(
    left: &ExprAst,
    item: &Expr,
    negated: bool,
    column_lookup: &std::collections::HashMap<String, usize>,
) -> Result<ExprAst> {
    let item = expr_to_kernel_ast(item, column_lookup)?;
    Ok(if negated {
        ExprAst::Ne(Box::new(left.clone()), Box::new(item))
    } else {
        ExprAst::Eq(Box::new(left.clone()), Box::new(item))
    })
}

pub(crate) fn sql_value_to_kernel_ast(v: &ValueWithSpan) -> Result<ExprAst> {
    if let Some(name) = crate::parser::bind::as_bind_name(&v.value) {
        return Err(Error::UnsupportedSql(format!(
            "bind markers are not allowed in DDL expressions: {name}"
        )));
    }
    Ok(ExprAst::Const(match &v.value {
        Value::Null => OwnedValue::Null,
        Value::Boolean(v) => OwnedValue::Integer(if *v { 1 } else { 0 }),
        Value::Number(num, _) => parse_numeric_value(num)?,
        Value::SingleQuotedString(s)
        | Value::DoubleQuotedString(s)
        | Value::EscapedStringLiteral(s)
        | Value::TripleSingleQuotedString(s)
        | Value::TripleDoubleQuotedString(s)
        | Value::UnicodeStringLiteral(s)
        | Value::SingleQuotedRawStringLiteral(s)
        | Value::DoubleQuotedRawStringLiteral(s)
        | Value::TripleSingleQuotedRawStringLiteral(s)
        | Value::TripleDoubleQuotedRawStringLiteral(s)
        | Value::DollarQuotedString(sqlparser::ast::DollarQuotedString { value: s, .. }) => {
            OwnedValue::Text(Arc::from(s.as_str()))
        }
        Value::SingleQuotedByteStringLiteral(s)
        | Value::DoubleQuotedByteStringLiteral(s)
        | Value::TripleSingleQuotedByteStringLiteral(s)
        | Value::TripleDoubleQuotedByteStringLiteral(s) => {
            OwnedValue::Blob(Arc::from(s.as_bytes()))
        }
        Value::HexStringLiteral(s) => OwnedValue::Blob(hex_string_to_bytes(s)?),
        other => {
            return Err(Error::UnsupportedSql(format!(
                "unsupported SQL literal: {other:?}"
            )));
        }
    }))
}

pub(crate) fn parse_numeric_value(input: &str) -> Result<OwnedValue> {
    if let Ok(v) = input.parse::<i64>() {
        return Ok(OwnedValue::Integer(v));
    }
    if let Ok(v) = input.parse::<f64>() {
        return Ok(OwnedValue::Real(v));
    }
    Err(Error::UnsupportedSql(format!(
        "invalid numeric literal: {input}"
    )))
}

pub(crate) fn hex_string_to_bytes(input: &str) -> Result<Arc<[u8]>> {
    if !input.len().is_multiple_of(2) {
        return Err(Error::UnsupportedSql(format!(
            "invalid hex string literal: {input}"
        )));
    }
    let mut out = Vec::with_capacity(input.len() / 2);
    for pair in input.as_bytes().chunks_exact(2) {
        let hi = hex_digit(pair[0])?;
        let lo = hex_digit(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(Arc::from(out))
}

pub(crate) fn hex_digit(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(Error::UnsupportedSql(format!(
            "invalid hex digit in blob literal: {}",
            byte as char
        ))),
    }
}

pub(crate) fn resolve_column_ordinal_name(
    column_lookup: &std::collections::HashMap<String, usize>,
    name: &str,
) -> Result<usize> {
    match column_lookup.get(&name.to_ascii_lowercase()).copied() {
        Some(v) => Ok(v),
        None => Err(Error::UnknownColumn(name.to_owned())),
    }
}

pub(crate) fn resolve_column_ordinal_in_table(
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    name: &str,
) -> Result<usize> {
    match table
        .columns
        .iter()
        .position(|column| column.folded.as_ref().eq_ignore_ascii_case(name))
    {
        Some(v) => Ok(v),
        None => Err(Error::UnknownColumn(name.to_owned())),
    }
}

pub(crate) fn resolve_column_ordinal_in_object_name(
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    name: &ObjectName,
) -> Result<usize> {
    match name.0.as_slice() {
        [part] => resolve_column_ordinal_in_table(table, &object_name_part_to_string(part)?),
        _ => Err(Error::UnsupportedSql(format!(
            "unsupported column name: {name}"
        ))),
    }
}

pub(crate) fn split_name(name: ObjectName) -> Result<(Option<DbName>, DbName)> {
    let display = name.to_string();
    let parts = name.0;
    match parts.as_slice() {
        [part] => Ok((None, DbName::new(object_name_part_to_string(part)?))),
        [schema, name] => Ok((
            Some(DbName::new(object_name_part_to_string(schema)?)),
            DbName::new(object_name_part_to_string(name)?),
        )),
        _ => Err(Error::UnsupportedSql(format!(
            "unsupported qualified name: {display}"
        ))),
    }
}

pub(crate) fn parse_qualified_name(name: ObjectName) -> Result<QualifiedName> {
    let (schema, name) = split_name(name)?;
    Ok(QualifiedName {
        schema: match schema {
            Some(s) => s,
            None => DbName::new("main"),
        },
        name,
    })
}
