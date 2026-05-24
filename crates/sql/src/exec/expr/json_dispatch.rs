//! Function-call dispatcher for scalar SQL functions.
//!
//! `eval_function` is the single entry point used by `eval_scalar` for
//! `Expr::Function`. The bulk of this file is a giant `match` on the
//! lower-cased function name; the JSON helpers delegate to
//! `crate::json::scalar`, the vector/datetime/string/numeric helpers live
//! in `super::scalar`, and any window-style call short-circuits via
//! `super::window::try_eval_window`.

use super::*;

thread_local! {
    static CURRENT_FTS_MATCH: std::cell::RefCell<Option<String>> = const {
        std::cell::RefCell::new(None)
    };
}

pub(crate) fn set_current_match_term(term: Option<String>) {
    CURRENT_FTS_MATCH.with(|cell| *cell.borrow_mut() = term);
}

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
    if name == "highlight" {
        return eval_highlight_function(func, row, bindings);
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

    eval_scalar_function_values(&name, values)
}

pub(crate) fn eval_scalar_function_values(
    name: &str,
    mut values: Vec<SqlValue>,
) -> Result<SqlValue> {
    match name {
        "last_insert_rowid" => {
            if !values.is_empty() {
                return Err(Error::UnsupportedSql(
                    "last_insert_rowid requires 0 args".to_owned(),
                ));
            }
            Ok(SqlValue::Integer(last_insert_rowid_value()))
        }
        "length" => match values.first() {
            // SQLite: length(NULL) is NULL, not 0. For TEXT, length returns
            // the count of Unicode characters (not bytes); for BLOB, byte
            // count. See https://sqlite.org/lang_corefunc.html#length.
            Some(SqlValue::Null) | None => Ok(SqlValue::Null),
            Some(SqlValue::Blob(value)) => Ok(SqlValue::Integer(value.len() as i64)),
            Some(SqlValue::Text(value)) => {
                Ok(SqlValue::Integer(value.chars().count() as i64))
            }
            Some(other) => Ok(SqlValue::Integer(value_to_string(other).chars().count() as i64)),
        },
        // SQLite octet_length(X): byte length regardless of type. TEXT in its
        // UTF-8 byte form, BLOB in its raw byte form, others coerced to TEXT
        // then byte-counted. NULL propagates.
        "octet_length" => match values.first() {
            Some(SqlValue::Null) | None => Ok(SqlValue::Null),
            Some(SqlValue::Blob(value)) => Ok(SqlValue::Integer(value.len() as i64)),
            Some(SqlValue::Text(value)) => Ok(SqlValue::Integer(value.as_bytes().len() as i64)),
            Some(other) => Ok(SqlValue::Integer(value_to_string(other).as_bytes().len() as i64)),
        },
        // SQLite concat(X, ...) — concatenates non-NULL operands (NULLs treated
        // as empty strings). Always returns TEXT.
        "concat" => {
            let mut out = String::new();
            for v in &values {
                if !matches!(v, SqlValue::Null) {
                    out.push_str(&value_to_string(v));
                }
            }
            Ok(SqlValue::Text(Arc::from(out)))
        }
        // SQLite concat_ws(SEP, X, ...) — like concat but inserts SEP between
        // non-NULL operands. NULL separator → NULL result; NULL operands
        // skipped.
        "concat_ws" => {
            if values.is_empty() || matches!(values[0], SqlValue::Null) {
                return Ok(SqlValue::Null);
            }
            let sep = value_to_string(&values[0]);
            let mut first = true;
            let mut out = String::new();
            for v in &values[1..] {
                if matches!(v, SqlValue::Null) {
                    continue;
                }
                if !first {
                    out.push_str(&sep);
                }
                first = false;
                out.push_str(&value_to_string(v));
            }
            Ok(SqlValue::Text(Arc::from(out)))
        }
        // soundex(X) is gated behind SQLITE_SOUNDEX in the reference build
        // and *not* compiled into sqlite3 v3.53.1 (`PRAGMA compile_options`
        // confirms it). Surface the same "no such function" error so parity
        // tests that expect rejection don't see a phantom success.
        "soundex" => Err(Error::UnsupportedSql("no such function: soundex".to_owned())),
        // SQLite unhex(X[, ignore]) — decode a hex string into a blob. If any
        // non-hex / non-ignore character appears, return NULL. Whitespace is
        // not implicit; only chars in `ignore` are skipped.
        "unhex" => {
            if values.is_empty() || matches!(values[0], SqlValue::Null) {
                return Ok(SqlValue::Null);
            }
            if values.len() > 1 && matches!(values[1], SqlValue::Null) {
                return Ok(SqlValue::Null);
            }
            let s = value_to_string(&values[0]);
            let ignore = values.get(1).map(value_to_string).unwrap_or_default();
            match sqlite_unhex(&s, &ignore) {
                Some(bytes) => Ok(SqlValue::Blob(Arc::from(bytes.as_slice()))),
                None => Ok(SqlValue::Null),
            }
        }
        // SQLite-style two-arg `like(PATTERN, VALUE)` / `like(PATTERN, VALUE, ESC)`
        // — function form (note argument order vs. the LIKE operator).
        "like" => {
            if values.len() < 2 {
                return Err(Error::UnsupportedSql(
                    "like requires at least 2 args".to_owned(),
                ));
            }
            let pattern = values[0].clone();
            let value = values[1].clone();
            let escape_char = values.get(2).and_then(|v| match v {
                SqlValue::Text(s) if s.chars().count() == 1 => {
                    Some(sqlparser::ast::Value::SingleQuotedString(s.to_string()))
                }
                _ => None,
            });
            let case_insensitive =
                crate::exec::current_connection().is_none_or(|conn| !conn.case_sensitive_like());
            like_result(value, pattern, false, escape_char, case_insensitive)
        }
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
        "min" | "max" => eval_scalar_min_max(&values, name == "min"),
        "round" => round_function(&values),
        // SQLite math1 unary functions. Each returns NULL for non-finite
        // / out-of-domain inputs (sqlite's math1 semantics) via `math1_unary`.
        "sin" => math1_unary(&values, f64::sin),
        "cos" => math1_unary(&values, f64::cos),
        "tan" => math1_unary(&values, f64::tan),
        "asin" => math1_unary(&values, f64::asin),
        "acos" => math1_unary(&values, f64::acos),
        "atan" => math1_unary(&values, f64::atan),
        "sinh" => math1_unary(&values, f64::sinh),
        "cosh" => math1_unary(&values, f64::cosh),
        "tanh" => math1_unary(&values, f64::tanh),
        "asinh" => math1_unary(&values, f64::asinh),
        "acosh" => math1_unary(&values, f64::acosh),
        "atanh" => math1_unary(&values, f64::atanh),
        "sqrt" => math1_unary(&values, f64::sqrt),
        "exp" => math1_unary(&values, f64::exp),
        "ln" => math1_unary(&values, f64::ln),
        "log10" => math1_unary(&values, f64::log10),
        "log2" => math1_unary(&values, f64::log2),
        // SQLite log(): 1-arg = natural log, 2-arg = log_b(x).
        "log" => math_log(&values),
        "atan2" => math1_binary(&values, f64::atan2),
        "degrees" => math_degrees(&values),
        "radians" => math_radians(&values),
        "trunc" => math_trunc(&values),
        "pi" => {
            if !values.is_empty() {
                return Err(Error::UnsupportedSql("pi takes 0 args".to_owned()));
            }
            Ok(math_pi())
        }
        "mod" => math_mod(&values),
        "ceil" | "ceiling" => math1_unary(&values, f64::ceil),
        "floor" => math1_unary(&values, f64::floor),
        "pow" | "power" => math1_binary(&values, f64::powf),
        "timediff" => timediff_function(&values),
        // SQLite hex(X) returns an *empty TEXT*, not NULL, when X is NULL —
        // see https://sqlite.org/lang_corefunc.html#hex and `func.c`. We
        // also default to empty TEXT when called with no args so error
        // surfaces stay consistent with sqlite.
        "hex" => match values.first() {
            None => Ok(SqlValue::Text(Arc::from(""))),
            Some(SqlValue::Null) => Ok(SqlValue::Text(Arc::from(""))),
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
            glob_result(values[1].clone(), values[0].clone(), false)
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
        "jsonb" => crate::json::scalar::json_func(&values),
        "to_jsonb" => crate::json::scalar::json_quote(&values),
        "jsonb_pretty" => crate::json::jsonb::jsonb_pretty(&values),
        "jsonb_strip_nulls" => crate::json::jsonb::jsonb_strip_nulls(&values),
        "jsonb_set" => crate::json::jsonb::jsonb_set(&values),
        "jsonb_insert" => crate::json::jsonb::jsonb_insert(&values),
        "jsonb_path_exists" => crate::json::jsonb::jsonb_path_exists(&values),
        "jsonb_path_match" => crate::json::jsonb::jsonb_path_match(&values),
        "jsonb_path_query_first" => crate::json::jsonb::jsonb_path_query_first(&values),
        "jsonb_contains" => crate::json::jsonb::jsonb_contains(&values),
        "jsonb_contained" => crate::json::jsonb::jsonb_contained(&values),
        "jsonb_exists" => crate::json::jsonb::jsonb_exists(&values),
        "jsonb_exists_any" => crate::json::jsonb::jsonb_exists_any(&values),
        "jsonb_exists_all" => crate::json::jsonb::jsonb_exists_all(&values),
        "jsonb_concat" => crate::json::jsonb::jsonb_concat(&values),
        "jsonb_delete" => crate::json::jsonb::jsonb_delete(&values),
        "jsonb_delete_path" => crate::json::jsonb::jsonb_delete_path(&values),
        "jsonb_typeof" => crate::json::jsonb::jsonb_typeof(&values),
        "jsonb_array_length" => crate::json::jsonb::jsonb_array_length(&values),
        "jsonb_build_object" => crate::json::jsonb::jsonb_build_object(&values),
        "jsonb_build_array" => crate::json::jsonb::jsonb_build_array(&values),
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
        // Track H — beyond-SQLite (Postgres) parity functions.
        "date_trunc" => crate::exec::expr::scalar::value::pg_date_trunc(&values),
        "gen_random_uuid" => crate::exec::expr::scalar::value::pg_gen_random_uuid(&values),
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

fn timediff_function(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() != 2 || values.iter().any(|v| matches!(v, SqlValue::Null)) {
        return Ok(SqlValue::Null);
    }
    let lhs = value_to_string(&values[0]);
    let rhs = value_to_string(&values[1]);
    let Some(lhs) = parse_ymd(&lhs) else {
        return Ok(SqlValue::Null);
    };
    let Some(rhs) = parse_ymd(&rhs) else {
        return Ok(SqlValue::Null);
    };
    let days = days_from_civil(lhs.0, lhs.1, lhs.2) - days_from_civil(rhs.0, rhs.1, rhs.2);
    let sign = if days < 0 { '-' } else { '+' };
    Ok(SqlValue::Text(Arc::from(format!(
        "{sign}0000-00-{:02} 00:00:00.000",
        days.abs()
    ))))
}

fn parse_ymd(value: &str) -> Option<(i64, i64, i64)> {
    let date = value.get(0..10)?;
    let mut parts = date.split('-');
    Some((
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ))
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = y - (m <= 2) as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn eval_highlight_function(
    func: &sqlparser::ast::Function,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    let FunctionArguments::List(list) = &func.args else {
        return Err(Error::UnsupportedSql(
            "highlight requires arguments".to_owned(),
        ));
    };
    if list.args.len() != 4 {
        return Err(Error::UnsupportedSql(
            "highlight requires 4 args".to_owned(),
        ));
    }
    let col_idx = match list.args.get(1) {
        Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))) => {
            numeric_value(&eval_scalar(expr, row, bindings)?)? as usize
        }
        _ => 0,
    };
    let start = match list.args.get(2) {
        Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))) => {
            value_to_string(&eval_scalar(expr, row, bindings)?)
        }
        _ => String::new(),
    };
    let end = match list.args.get(3) {
        Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))) => {
            value_to_string(&eval_scalar(expr, row, bindings)?)
        }
        _ => String::new(),
    };
    let values = row.to_owned_row().values()?;
    let text = values.get(col_idx).map(value_to_string).unwrap_or_default();
    let Some(needle) = current_match_term() else {
        return Ok(SqlValue::Text(Arc::from(text)));
    };
    if needle.is_empty() {
        return Ok(SqlValue::Text(Arc::from(text)));
    }
    Ok(SqlValue::Text(Arc::from(
        text.replace(&needle, &format!("{start}{needle}{end}")),
    )))
}

fn current_match_term() -> Option<String> {
    CURRENT_FTS_MATCH.with(|cell| cell.borrow().clone())
}

fn eval_scalar_min_max(values: &[SqlValue], is_min: bool) -> Result<SqlValue> {
    if values.is_empty() || values.iter().any(|value| matches!(value, SqlValue::Null)) {
        return Ok(SqlValue::Null);
    }
    let mut best = values[0].clone();
    for value in &values[1..] {
        let ord = compare_values(value, &best);
        let replace = if is_min {
            ord == Ordering::Less
        } else {
            ord == Ordering::Greater
        };
        if replace {
            best = value.clone();
        }
    }
    Ok(best)
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
