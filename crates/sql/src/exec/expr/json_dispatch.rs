//! Function-call dispatcher for scalar SQL functions.
//!
//! `eval_function` is the single entry point used by `eval_scalar` for
//! `Expr::Function`. The bulk of this file is a giant `match` on the
//! lower-cased function name; the JSON helpers delegate to
//! `crate::json::scalar`, the vector/datetime/string/numeric helpers live
//! in `super::scalar`, and any window-style call short-circuits via
//! `super::window::try_eval_window`.

use super::*;

pub(super) fn eval_function(
    func: &sqlparser::ast::Function,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    if let Some(result) = window::try_eval_window(func) {
        return result;
    }
    let name = func.name.to_string().to_ascii_lowercase();
    if name == "raise" {
        return eval_raise_function(func);
    }
    let mut values = Vec::new();
    if let FunctionArguments::List(list) = &func.args {
        for arg in &list.args {
            match arg {
                FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => {
                    values.push(eval_scalar(expr, row, bindings)?)
                }
                _ => {
                    return Err(Error::UnsupportedSql(
                        "unsupported function argument".to_owned(),
                    ));
                }
            }
        }
    } else if !matches!(func.args, FunctionArguments::None) {
        return Err(Error::UnsupportedSql(
            "unsupported function call form".to_owned(),
        ));
    }

    match name.as_str() {
        "last_insert_rowid" => {
            if !values.is_empty() {
                return Err(Error::UnsupportedSql(
                    "last_insert_rowid requires 0 args".to_owned(),
                ));
            }
            Ok(SqlValue::Integer(last_insert_rowid_value()))
        }
        "length" => match values.first() {
            // SQLite: length(NULL) is NULL, not 0.
            Some(SqlValue::Null) | None => Ok(SqlValue::Null),
            Some(SqlValue::Blob(value)) => Ok(SqlValue::Integer(value.len() as i64)),
            Some(other) => Ok(SqlValue::Integer(value_to_string(other).len() as i64)),
        },
        "lower" => match values.first() {
            Some(SqlValue::Null) | None => Ok(SqlValue::Null),
            Some(other) => Ok(SqlValue::Text(Arc::from(
                value_to_string(other).to_ascii_lowercase(),
            ))),
        },
        "upper" => match values.first() {
            Some(SqlValue::Null) | None => Ok(SqlValue::Null),
            Some(other) => Ok(SqlValue::Text(Arc::from(
                value_to_string(other).to_ascii_uppercase(),
            ))),
        },
        "abs" => match values.first() {
            // SQLite: abs(NULL) is NULL, not an error.
            Some(SqlValue::Null) | None => Ok(SqlValue::Null),
            Some(SqlValue::Integer(v)) => Ok(SqlValue::Integer(v.wrapping_abs())),
            Some(SqlValue::Real(v)) => Ok(SqlValue::Real(v.abs())),
            // Coerce text / blob to numeric then abs (SQLite implicit-numeric).
            Some(SqlValue::Text(_)) | Some(SqlValue::Blob(_)) => {
                match numeric_value(values.first().unwrap()) {
                    Ok(v) => Ok(SqlValue::Real(v.abs())),
                    Err(_) => Ok(SqlValue::Real(0.0)),
                }
            }
        },
        "coalesce" | "ifnull" => {
            for value in values {
                if !matches!(value, SqlValue::Null) {
                    return Ok(value);
                }
            }
            Ok(SqlValue::Null)
        }
        "nullif" => {
            if values.len() != 2 {
                return Err(Error::UnsupportedSql("nullif requires 2 args".to_owned()));
            }
            if compare_values(&values[0], &values[1]) == Ordering::Equal {
                Ok(SqlValue::Null)
            } else {
                Ok(values.remove(0))
            }
        }
        "round" => round_function(&values),
        "hex" => match values.first() {
            Some(SqlValue::Null) | None => Ok(SqlValue::Null),
            Some(other) => Ok(SqlValue::Text(Arc::from(hex_value(other)))),
        },
        "quote" => Ok(SqlValue::Text(Arc::from(quote_value(
            values.first().unwrap_or(&SqlValue::Null),
        )))),
        "random" => Ok(SqlValue::Integer(random_i64())),
        "likely" | "unlikely" => Ok(values.into_iter().next().unwrap_or(SqlValue::Null)),
        "likelihood" => Ok(values.into_iter().next().unwrap_or(SqlValue::Null)),
        // SQLite substr(X, Y) / substr(X, Y, Z) — 1-based, negative Y counts
        // from the end, negative Z is an error in SQLite but we clamp to 0.
        "substr" | "substring" => sqlite_substr_function(&values),
        // SQLite instr(X, Y) — 1-based position of first occurrence of Y in X,
        // 0 if not found, NULL if either arg is NULL.
        "instr" => {
            if values.len() < 2 {
                return Ok(SqlValue::Null);
            }
            if matches!(values[0], SqlValue::Null) || matches!(values[1], SqlValue::Null) {
                return Ok(SqlValue::Null);
            }
            let haystack = value_to_string(&values[0]);
            let needle = value_to_string(&values[1]);
            if needle.is_empty() {
                return Ok(SqlValue::Integer(1));
            }
            let pos = haystack
                .char_indices()
                .enumerate()
                .find(|(_, (byte_pos, _))| haystack[*byte_pos..].starts_with(&needle))
                .map(|(char_pos, _)| char_pos as i64 + 1)
                .unwrap_or(0);
            Ok(SqlValue::Integer(pos))
        }
        // SQLite trim / ltrim / rtrim — strip specified chars (or whitespace).
        "trim" => sqlite_trim_function(values.first().unwrap_or(&SqlValue::Null), values.get(1)),
        "ltrim" => sqlite_ltrim_function(values.first().unwrap_or(&SqlValue::Null), values.get(1)),
        "rtrim" => sqlite_rtrim_function(values.first().unwrap_or(&SqlValue::Null), values.get(1)),
        // SQLite replace(X, Y, Z) — replace all occurrences of Y in X with Z.
        "replace" => {
            if values.len() < 3 {
                return Ok(SqlValue::Null);
            }
            if values.iter().take(3).any(|v| matches!(v, SqlValue::Null)) {
                return Ok(SqlValue::Null);
            }
            let s = value_to_string(&values[0]);
            let from = value_to_string(&values[1]);
            let to = value_to_string(&values[2]);
            Ok(SqlValue::Text(Arc::from(
                s.replace(from.as_str(), to.as_str()),
            )))
        }
        // SQLite printf/format — basic sprintf-style formatting.
        // We support %s %d %i %f %e %g %x %X %o %% placeholders.
        "printf" | "format" => {
            if values.is_empty() || matches!(values[0], SqlValue::Null) {
                return Ok(SqlValue::Null);
            }
            let fmt = value_to_string(&values[0]);
            let result = sqlite_printf(&fmt, &values[1..]);
            Ok(SqlValue::Text(Arc::from(result)))
        }
        // SQLite iif(C, T, F) — equivalent to CASE WHEN C THEN T ELSE F END.
        "iif" => {
            if values.len() < 3 {
                return Ok(SqlValue::Null);
            }
            if is_truthy(&values[0]) {
                Ok(values.remove(1))
            } else {
                Ok(values.remove(2))
            }
        }
        // SQLite sign(X) — returns -1, 0, 1, or NULL.
        "sign" => match values.first() {
            None | Some(SqlValue::Null) => Ok(SqlValue::Null),
            Some(SqlValue::Integer(v)) => Ok(SqlValue::Integer(v.signum())),
            Some(SqlValue::Real(v)) => Ok(SqlValue::Integer(if *v < 0.0 {
                -1
            } else if *v > 0.0 {
                1
            } else {
                0
            })),
            Some(other) => match value_to_string(other).trim().parse::<f64>() {
                Ok(v) => Ok(SqlValue::Integer(if v < 0.0 {
                    -1
                } else if v > 0.0 {
                    1
                } else {
                    0
                })),
                Err(_) => Ok(SqlValue::Integer(0)),
            },
        },
        // SQLite char(X1, X2, ...) — returns string of Unicode code points.
        "char" => {
            let mut out = String::with_capacity(values.len() * 3);
            for v in &values {
                let cp = match v {
                    SqlValue::Integer(n) => *n,
                    SqlValue::Real(r) => *r as i64,
                    SqlValue::Null => return Ok(SqlValue::Null),
                    other => value_to_string(other).trim().parse::<i64>().unwrap_or(0),
                };
                let ch = char::from_u32(cp as u32).unwrap_or(char::REPLACEMENT_CHARACTER);
                out.push(ch);
            }
            Ok(SqlValue::Text(Arc::from(out)))
        }
        // SQLite unicode(X) — returns Unicode code point of first char of X.
        "unicode" => match values.first() {
            None | Some(SqlValue::Null) => Ok(SqlValue::Null),
            Some(v) => {
                let s = value_to_string(v);
                match s.chars().next() {
                    None => Ok(SqlValue::Null),
                    Some(ch) => Ok(SqlValue::Integer(ch as i64)),
                }
            }
        },
        // SQLite zeroblob(N) — returns a BLOB of N zero bytes.
        "zeroblob" => match values.first() {
            None | Some(SqlValue::Null) => Ok(SqlValue::Null),
            Some(v) => {
                let n = match v {
                    SqlValue::Integer(n) => *n,
                    SqlValue::Real(r) => *r as i64,
                    other => value_to_string(other).trim().parse::<i64>().unwrap_or(0),
                };
                let n = n.max(0) as usize;
                Ok(SqlValue::Blob(Arc::from(vec![0u8; n].as_slice())))
            }
        },
        // SQLite randomblob(N) — returns N random bytes as BLOB.
        "randomblob" => match values.first() {
            None | Some(SqlValue::Null) => Ok(SqlValue::Null),
            Some(v) => {
                let n = match v {
                    SqlValue::Integer(n) => *n,
                    SqlValue::Real(r) => *r as i64,
                    other => value_to_string(other).trim().parse::<i64>().unwrap_or(0),
                };
                let n = n.max(0) as usize;
                let bytes: Vec<u8> = (0..n).map(|_| (random_i64() & 0xFF) as u8).collect();
                Ok(SqlValue::Blob(Arc::from(bytes.as_slice())))
            }
        },
        "glob" => {
            if values.len() < 2 {
                return Err(Error::UnsupportedSql("glob requires 2 args".to_owned()));
            }
            glob_result(values[0].clone(), values[1].clone(), false)
        }
        "typeof" => Ok(SqlValue::Text(Arc::from(match values.first() {
            Some(SqlValue::Null) | None => "null",
            Some(SqlValue::Integer(_)) => "integer",
            Some(SqlValue::Real(_)) => "real",
            Some(SqlValue::Text(_)) => "text",
            Some(SqlValue::Blob(_)) => "blob",
        }))),
        "json" => crate::json::scalar::json_func(&values),
        "json_array" => crate::json::scalar::json_array(&values),
        "json_array_length" => crate::json::scalar::json_array_length(&values),
        "json_object" => crate::json::scalar::json_object(&values),
        "json_extract" => crate::json::scalar::json_extract(&values),
        "json_set" => crate::json::scalar::json_set(&values),
        "json_insert" => crate::json::scalar::json_insert(&values),
        "json_replace" => crate::json::scalar::json_replace(&values),
        "json_remove" => crate::json::scalar::json_remove(&values),
        "json_patch" => crate::json::scalar::json_patch(&values),
        "json_type" => crate::json::scalar::json_type(&values),
        "json_valid" => crate::json::scalar::json_valid(&values),
        "json_quote" => crate::json::scalar::json_quote(&values),
        "json_minify" => crate::json::scalar::json_minify(&values),
        "vector" | "vector_blob" | "vector_from_json" => {
            let arg = values.first().unwrap_or(&SqlValue::Null);
            vector_construct_from_value(arg)
        }
        "vector_dims" => {
            let arg = values.first().unwrap_or(&SqlValue::Null);
            vector_dims_value(arg)
        }
        "vector_distance_l2" => vector_pair_distance(&values, VectorOpMetric::L2),
        "vector_distance_cosine" => vector_pair_distance(&values, VectorOpMetric::Cosine),
        "vector_distance_ip" => vector_pair_distance(&values, VectorOpMetric::InnerProduct),
        "date" => datetime_function(&values, DateTimeKind::Date),
        "time" => datetime_function(&values, DateTimeKind::Time),
        "datetime" => datetime_function(&values, DateTimeKind::Datetime),
        "julianday" => datetime_function(&values, DateTimeKind::JulianDay),
        "unixepoch" => datetime_function(&values, DateTimeKind::Unix),
        "strftime" => strftime_function(&values),
        "regexp" => {
            if values.len() != 2 {
                return Err(Error::UnsupportedSql("regexp requires 2 args".to_owned()));
            }
            crate::exec::expr::regexp_result(values[1].clone(), values[0].clone(), false)
        }
        _ => {
            let db = crate::udf::current_db();
            match crate::udf::call_registered_scalar(db, &name, &values) {
                Some(Ok(v)) => Ok(v),
                Some(Err(msg)) => Err(Error::UnsupportedSql(msg)),
                None => Err(Error::UnsupportedSql(format!(
                    "unsupported function {name}"
                ))),
            }
        }
    }
}

fn eval_raise_function(func: &sqlparser::ast::Function) -> Result<SqlValue> {
    let FunctionArguments::List(list) = &func.args else {
        return Err(Error::UnsupportedSql("RAISE requires arguments".to_owned()));
    };
    let action = raise_action(list.args.first())?;
    if action == "ignore" {
        return Err(Error::TriggerIgnore);
    }
    if !matches!(action.as_str(), "abort" | "fail" | "rollback") {
        return Err(Error::UnsupportedSql(format!(
            "unsupported RAISE action: {action}"
        )));
    }
    if list.args.len() != 2 {
        return Err(Error::UnsupportedSql(format!(
            "RAISE({}) requires a message",
            action.to_ascii_uppercase()
        )));
    }
    let message = raise_message(list.args.get(1))?;
    Err(Error::ConstraintViolation(message))
}

fn raise_action(arg: Option<&FunctionArg>) -> Result<String> {
    match arg {
        Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(Expr::Identifier(ident)))) => {
            Ok(ident.value.to_ascii_lowercase())
        }
        Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(Expr::Value(value)))) => {
            match &value.value {
                Value::SingleQuotedString(value) | Value::DoubleQuotedString(value) => {
                    Ok(value.to_ascii_lowercase())
                }
                _ => Err(Error::UnsupportedSql(
                    "RAISE action must be ABORT, FAIL, ROLLBACK, or IGNORE".to_owned(),
                )),
            }
        }
        _ => Err(Error::UnsupportedSql(
            "RAISE action must be ABORT, FAIL, ROLLBACK, or IGNORE".to_owned(),
        )),
    }
}

fn raise_message(arg: Option<&FunctionArg>) -> Result<String> {
    match arg {
        Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(Expr::Value(value)))) => match &value.value
        {
            Value::SingleQuotedString(value)
            | Value::DoubleQuotedString(value)
            | Value::EscapedStringLiteral(value)
            | Value::TripleSingleQuotedString(value)
            | Value::TripleDoubleQuotedString(value)
            | Value::UnicodeStringLiteral(value)
            | Value::SingleQuotedRawStringLiteral(value)
            | Value::DoubleQuotedRawStringLiteral(value)
            | Value::TripleSingleQuotedRawStringLiteral(value)
            | Value::TripleDoubleQuotedRawStringLiteral(value) => Ok(value.clone()),
            Value::DollarQuotedString(value) => Ok(value.value.clone()),
            _ => Err(Error::UnsupportedSql(
                "RAISE message must be a string literal".to_owned(),
            )),
        },
        _ => Err(Error::UnsupportedSql(
            "RAISE message must be a string literal".to_owned(),
        )),
    }
}

fn last_insert_rowid_value() -> i64 {
    if let Some(ptr) = crate::exec::current_session_ptr() {
        // SAFETY: installed by `with_write_tx` for the duration of the
        // synchronous statement/trigger execution scope.
        let session: &crate::session::SessionState = unsafe { &*ptr };
        return session.last_insert_rowid.unwrap_or(0);
    }
    current_connection()
        .and_then(|conn| conn.last_insert_rowid())
        .unwrap_or(0)
}
