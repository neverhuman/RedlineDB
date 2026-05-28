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

/// Stack-buffer capacity for lowercased function names. Every known
/// scalar/aggregate/window function fits comfortably (longest is
/// `json_group_object` at 17 bytes; we round up to 48 for safety).
pub(crate) const FN_NAME_STACK: usize = 48;

/// Borrow the function name as a single unquoted identifier, lowercased
/// into the caller-provided stack buffer. Returns `None` for qualified
/// names (`schema.fn`), quoted identifiers, or names longer than
/// `FN_NAME_STACK` — those callers fall through to the
/// `to_string().to_ascii_lowercase()` slow path, which still pays the
/// allocation cost but is reached for <1% of function calls in
/// practice.
pub(crate) fn simple_function_name_lower<'b>(
    func: &sqlparser::ast::Function,
    scratch: &'b mut [u8; FN_NAME_STACK],
) -> Option<&'b str> {
    let parts = &func.name.0;
    if parts.len() != 1 {
        return None;
    }
    let ident = match &parts[0] {
        sqlparser::ast::ObjectNamePart::Identifier(ident) if ident.quote_style.is_none() => ident,
        _ => return None,
    };
    let raw = ident.value.as_bytes();
    if raw.len() > scratch.len() {
        return None;
    }
    for (i, &b) in raw.iter().enumerate() {
        scratch[i] = b.to_ascii_lowercase();
    }
    let s = &scratch[..raw.len()];
    // SAFETY: `raw` was a valid UTF-8 &str (from Ident::value), and
    // ASCII-folding preserves UTF-8 validity for the subset of bytes
    // <0x80. For bytes >=0x80 the value is unchanged by
    // to_ascii_lowercase, so the resulting buffer is byte-identical
    // valid UTF-8.
    std::str::from_utf8(s).ok()
}

pub(super) fn eval_function(
    func: &sqlparser::ast::Function,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    if let Some(result) = window::try_eval_window(func) {
        return result;
    }

    let mut scratch = [0u8; FN_NAME_STACK];
    let borrowed = simple_function_name_lower(func, &mut scratch);
    let owned;
    let name: &str = match borrowed {
        Some(s) => s,
        None => {
            owned = func.name.to_string().to_ascii_lowercase();
            owned.as_str()
        }
    };

    if name == "raise" {
        return eval_raise_function(func);
    }
    if name == "highlight" {
        return eval_highlight_function(func, row, bindings);
    }
    // Phase 4.3: hint capacity for the args buffer. Called per scalar
    // function call per row in projection / aggregate filter paths.
    let mut values = Vec::with_capacity(match &func.args {
        FunctionArguments::List(list) => list.args.len(),
        _ => 0,
    });
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

    eval_scalar_function_values(name, values)
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
                // Phase 2.1 ASCII fast path: for pure-ASCII strings,
                // character count equals byte length. `str::is_ascii`
                // is SIMD-vectorized on x86_64 in Rust 1.95.
                let len = if value.is_ascii() {
                    value.len() as i64
                } else {
                    value.chars().count() as i64
                };
                Ok(SqlValue::Integer(len))
            }
            Some(other) => {
                let s = value_to_string(other);
                let len = if s.is_ascii() {
                    s.len() as i64
                } else {
                    s.chars().count() as i64
                };
                Ok(SqlValue::Integer(len))
            }
        },
        // SQLite octet_length(X): byte length regardless of type. TEXT in its
        // UTF-8 byte form, BLOB in its raw byte form, others coerced to TEXT
        // then byte-counted. NULL propagates.
        "octet_length" => match values.first() {
            Some(SqlValue::Null) | None => Ok(SqlValue::Null),
            Some(SqlValue::Blob(value)) => Ok(SqlValue::Integer(value.len() as i64)),
            Some(SqlValue::Text(value)) => Ok(SqlValue::Integer(value.as_bytes().len() as i64)),
            Some(other) => Ok(SqlValue::Integer(
                value_to_string(other).as_bytes().len() as i64
            )),
        },
        // SQLite concat(X, ...) — concatenates non-NULL operands (NULLs treated
        // as empty strings). Always returns TEXT.
        // Phase 2.3: value_as_str returns Cow<'_, str>; SqlValue::Text
        // borrows from its Arc<str> without allocation.
        "concat" => {
            let mut out = String::new();
            for v in &values {
                if !matches!(v, SqlValue::Null) {
                    out.push_str(value_as_str(v).as_ref());
                }
            }
            Ok(SqlValue::Text(Arc::from(out)))
        }
        "concat_ws" => {
            if values.is_empty() || matches!(values[0], SqlValue::Null) {
                return Ok(SqlValue::Null);
            }
            let sep = value_as_str(&values[0]);
            let mut first = true;
            let mut out = String::new();
            for v in &values[1..] {
                if matches!(v, SqlValue::Null) {
                    continue;
                }
                if !first {
                    out.push_str(sep.as_ref());
                }
                first = false;
                out.push_str(value_as_str(v).as_ref());
            }
            Ok(SqlValue::Text(Arc::from(out)))
        }
        // soundex(X) is gated behind SQLITE_SOUNDEX in the reference build
        // and *not* compiled into sqlite3 v3.53.1 (`PRAGMA compile_options`
        // confirms it). Surface the same "no such function" error so parity
        // tests that expect rejection don't see a phantom success.
        "soundex" => Err(Error::UnsupportedSql(
            "no such function: soundex".to_owned(),
        )),
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
            // A13: pass by reference — the previous `clone()`s are pure
            // waste now that `like_result` takes `&SqlValue`.
            let pattern = &values[0];
            let value = &values[1];
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
        // SQLite's `lower`/`upper` are documented as ASCII-only, but in
        // practice the reference build links against ICU and folds the
        // full Unicode range. Postgres with a UTF-8 libc locale (e.g.
        // en_US.UTF-8) does Unicode-aware case folding too — but its
        // libc `wctoupper`/`wctolower` only do 1-to-1 mappings, NOT
        // Unicode's full SpecialCasing table. That means `straße` →
        // `STRAßE` (not `STRASSE`) and `İ` → `İ` (not `I` + combining
        // dot above). Mirror that by running `char::to_uppercase`/
        // `to_lowercase` per character and falling back to the original
        // when the iterator yields more than one char (a SpecialCasing
        // expansion). NULL propagates.
        "lower" => match values.first() {
            Some(SqlValue::Null) | None => Ok(SqlValue::Null),
            Some(other) => Ok(SqlValue::Text(Arc::from(libc_lower(
                value_as_str(other).as_ref(),
            )))),
        },
        "upper" => match values.first() {
            Some(SqlValue::Null) | None => Ok(SqlValue::Null),
            Some(other) => Ok(SqlValue::Text(Arc::from(libc_upper(
                value_as_str(other).as_ref(),
            )))),
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
        "sin" => math1_unary(&values, libm::sin),
        "cos" => math1_unary(&values, libm::cos),
        "tan" => math1_unary(&values, libm::tan),
        "asin" => math1_unary(&values, libm::asin),
        "acos" => math1_unary(&values, libm::acos),
        "atan" => math1_unary(&values, libm::atan),
        "sinh" => math1_unary(&values, libm::sinh),
        "cosh" => math1_unary(&values, libm::cosh),
        "tanh" => math1_unary(&values, libm::tanh),
        "asinh" => math1_unary(&values, libm::asinh),
        "acosh" => math1_unary(&values, libm::acosh),
        "atanh" => math1_unary(&values, libm::atanh),
        "sqrt" => math1_unary(&values, libm::sqrt),
        "exp" => math1_unary(&values, libm::exp),
        "ln" => math1_unary(&values, libm::log),
        "log10" => math1_unary(&values, libm::log10),
        "log2" => math1_unary(&values, libm::log2),
        // SQLite log(): 1-arg = natural log, 2-arg = log_b(x).
        "log" => math_log(&values),
        "atan2" => math1_binary(&values, libm::atan2),
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
            // Phase 2.3: borrow when possible.
            let haystack = value_as_str(&values[0]);
            let needle = value_as_str(&values[1]);
            if needle.is_empty() {
                return Ok(SqlValue::Integer(1));
            }
            // Phase 2.2: ASCII fast path. When both sides are ASCII,
            // byte offset == char offset, so memmem (SIMD-accelerated
            // for >=2-byte needles via memchr) gives us O(n) substring
            // search without the per-char `starts_with` allocation
            // cascade.
            let pos = if haystack.is_ascii() && needle.is_ascii() {
                match memchr::memmem::find(haystack.as_bytes(), needle.as_bytes()) {
                    Some(byte_pos) => byte_pos as i64 + 1,
                    None => 0,
                }
            } else {
                let hay: &str = haystack.as_ref();
                let need: &str = needle.as_ref();
                hay.char_indices()
                    .enumerate()
                    .find(|(_, (byte_pos, _))| hay[*byte_pos..].starts_with(need))
                    .map(|(char_pos, _)| char_pos as i64 + 1)
                    .unwrap_or(0)
            };
            Ok(SqlValue::Integer(pos))
        }
        // SQLite trim / ltrim / rtrim — strip specified chars (or whitespace).
        "trim" => sqlite_trim_function(values.first().unwrap_or(&SqlValue::Null), values.get(1)),
        "ltrim" => sqlite_ltrim_function(values.first().unwrap_or(&SqlValue::Null), values.get(1)),
        "rtrim" => sqlite_rtrim_function(values.first().unwrap_or(&SqlValue::Null), values.get(1)),
        // SQLite replace(X, Y, Z) — replace all occurrences of Y in X with Z.
        // Phase 2.3 + 2.5: value_as_str borrows from Arc<str> when the
        // argument is already a Text value (the common case for
        // REPLACE on column data); avoids three String allocations
        // per call.
        "replace" => {
            if values.len() < 3 {
                return Ok(SqlValue::Null);
            }
            if values.iter().take(3).any(|v| matches!(v, SqlValue::Null)) {
                return Ok(SqlValue::Null);
            }
            let s = value_as_str(&values[0]);
            let from = value_as_str(&values[1]);
            let to = value_as_str(&values[2]);
            Ok(SqlValue::Text(Arc::from(
                s.replace(from.as_ref(), to.as_ref()),
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
            // A14: pass by reference — `glob_result` now takes `&SqlValue`.
            glob_result(&values[1], &values[0], false)
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
        "pg_array_contains" => crate::exec::expr::scalar::value::pg_array_contains(&values),
        "pg_array_contained" => crate::exec::expr::scalar::value::pg_array_contained(&values),
        "pg_array_overlap" => crate::exec::expr::scalar::value::pg_array_overlap(&values),
        // Track J — Postgres sequence helpers operate on session-level
        // sequence state recorded by CREATE SEQUENCE.
        "nextval" => pg_sequence_nextval(&values),
        "currval" => pg_sequence_currval(&values),
        "setval" => pg_sequence_setval(&values),
        "current_schema" => Ok(SqlValue::Text(std::sync::Arc::from("public"))),
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

/// Track J — Postgres `nextval(seq)`. Reads the named sequence from
/// session state, advances it by `increment`, and returns the new value.
/// The first call returns the configured `start`; subsequent calls add
/// `increment`. Unknown sequences raise an UnsupportedSql error
/// mirroring `relation "<name>" does not exist`.
fn pg_sequence_nextval(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() != 1 {
        return Err(Error::UnsupportedSql(
            "nextval expects one argument".to_owned(),
        ));
    }
    let name = pg_sequence_name(&values[0])?;
    let conn = crate::exec::current_connection().ok_or_else(|| {
        Error::UnsupportedSql("nextval requires an active connection context".to_owned())
    })?;
    let result = conn.with_session(|session| {
        let entry = session
            .pg_sequences
            .get_mut(&name)
            .ok_or_else(|| Error::UnsupportedSql(format!("relation \"{name}\" does not exist")))?;
        let next = match entry.last_value {
            Some(v) => v + entry.increment,
            None => entry.start,
        };
        entry.last_value = Some(next);
        Ok(next)
    })?;
    Ok(SqlValue::Integer(result))
}

/// Track J — Postgres `currval(seq)`. Returns the most recent value
/// produced by `nextval`. Errors if `nextval` has never been called on
/// the sequence in this session, mirroring Postgres' standard surface.
fn pg_sequence_currval(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() != 1 {
        return Err(Error::UnsupportedSql(
            "currval expects one argument".to_owned(),
        ));
    }
    let name = pg_sequence_name(&values[0])?;
    let conn = crate::exec::current_connection().ok_or_else(|| {
        Error::UnsupportedSql("currval requires an active connection context".to_owned())
    })?;
    let result = conn.with_session(|session| {
        let entry = session
            .pg_sequences
            .get(&name)
            .ok_or_else(|| Error::UnsupportedSql(format!("relation \"{name}\" does not exist")))?;
        match entry.last_value {
            Some(v) => Ok(v),
            None => Err(Error::UnsupportedSql(format!(
                "currval of sequence \"{name}\" is not yet defined in this session"
            ))),
        }
    })?;
    Ok(SqlValue::Integer(result))
}

/// Track J — Postgres `setval(seq, value [, is_called])`. Sets the
/// sequence's last_value to the given integer. If `is_called` is false,
/// the next `nextval` returns `value` rather than `value + increment`
/// (Postgres semantics). When omitted, `is_called` defaults to true.
fn pg_sequence_setval(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() < 2 || values.len() > 3 {
        return Err(Error::UnsupportedSql(
            "setval expects 2 or 3 arguments".to_owned(),
        ));
    }
    let name = pg_sequence_name(&values[0])?;
    let value = match &values[1] {
        SqlValue::Integer(v) => *v,
        SqlValue::Real(v) => *v as i64,
        SqlValue::Text(t) => t
            .parse::<i64>()
            .map_err(|_| Error::UnsupportedSql(format!("setval value must be integer: {t}")))?,
        _ => {
            return Err(Error::UnsupportedSql(
                "setval second argument must be integer".to_owned(),
            ));
        }
    };
    let is_called = if values.len() == 3 {
        match &values[2] {
            SqlValue::Integer(v) => *v != 0,
            _ => true,
        }
    } else {
        true
    };
    let conn = crate::exec::current_connection().ok_or_else(|| {
        Error::UnsupportedSql("setval requires an active connection context".to_owned())
    })?;
    conn.with_session(|session| {
        let entry = session
            .pg_sequences
            .get_mut(&name)
            .ok_or_else(|| Error::UnsupportedSql(format!("relation \"{name}\" does not exist")))?;
        if is_called {
            entry.last_value = Some(value);
        } else {
            entry.last_value = Some(value - entry.increment);
        }
        Ok(())
    })?;
    Ok(SqlValue::Integer(value))
}

fn pg_sequence_name(value: &SqlValue) -> Result<String> {
    match value {
        SqlValue::Text(s) => {
            // Track J — strip schema qualifier (`sch.s` → `s`). SQLite has
            // no schema layer; sequences live in a flat session map.
            let folded = s.to_ascii_lowercase();
            let stripped = folded
                .rsplit_once('.')
                .map(|(_schema, name)| name.to_owned())
                .unwrap_or(folded);
            Ok(stripped)
        }
        _ => Err(Error::UnsupportedSql(
            "sequence name must be a string".to_owned(),
        )),
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

/// libc-style Unicode lowercasing: per-char `to_lowercase`, but if the
/// canonical mapping yields more than one char (a Unicode SpecialCasing
/// expansion — e.g. Turkish dotted `İ` → `i`+combining-dot-above) keep
/// the original char instead. This matches Postgres' `lower()` with a
/// UTF-8 libc locale, whose underlying `wctolower` only emits 1-to-1
/// mappings.
///
/// Phase 2.1: ASCII fast path. `make_ascii_lowercase` is a single
/// SIMD-vectorized byte sweep when the input is pure ASCII (the
/// dominant case for SCALAR_STRING and the SCALAR_ARITH cases). For
/// any byte >= 0x80 we fall through to the per-char Unicode path.
fn libc_lower(input: &str) -> String {
    if input.is_ascii() {
        let mut bytes = input.as_bytes().to_vec();
        bytes.make_ascii_lowercase();
        return String::from_utf8(bytes).expect("ascii bytes are valid utf-8");
    }
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        let mut iter = ch.to_lowercase();
        match (iter.next(), iter.next()) {
            (Some(first), None) => out.push(first),
            (Some(_), Some(_)) | (None, _) => out.push(ch),
        }
    }
    out
}

/// libc-style Unicode uppercasing: per-char `to_uppercase`, but if the
/// canonical mapping yields more than one char (e.g. `ß` → `SS`) keep
/// the original char. Matches Postgres' `upper()` with a UTF-8 libc
/// locale — `upper('straße')` → `STRAßE`, `upper('σς')` → `ΣΣ`.
fn libc_upper(input: &str) -> String {
    if input.is_ascii() {
        let mut bytes = input.as_bytes().to_vec();
        bytes.make_ascii_uppercase();
        return String::from_utf8(bytes).expect("ascii bytes are valid utf-8");
    }
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        let mut iter = ch.to_uppercase();
        match (iter.next(), iter.next()) {
            (Some(first), None) => out.push(first),
            (Some(_), Some(_)) | (None, _) => out.push(ch),
        }
    }
    out
}
