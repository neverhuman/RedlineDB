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

    let name = func.name.to_string().to_ascii_lowercase();
    match name.as_str() {
        "length" => match values.first() {
            // SQLite: length(NULL) is NULL, not 0.
            Some(SqlValue::Null) | None => Ok(SqlValue::Null),
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
        _ => Err(Error::UnsupportedSql(format!(
            "unsupported function {name}"
        ))),
    }
}
