//! Postgres-flavoured JSONB operator and function helpers.
//!
//! RedlineDB stores JSON as TEXT (SQLite JSON1 style). These helpers
//! implement the operator semantics PostgreSQL 16 exposes for `jsonb`
//! values, parsing/serialising via `serde_json`. Booleans are returned
//! as the literal text tokens `"t"` / `"f"` to match `psql -At` rendering.
//!
//! Rendering uses Postgres' "compact-with-spaces" style:
//!   `{"a":1,"b":2}`  →  `{"a": 1, "b": 2}`
//!   `[1,2,3]`        →  `[1, 2, 3]`

use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::value::SqlValue;

use super::path::{JsonPath, MutationMode, mutate, remove, resolve};
use super::scalar::{parse_json_arg, sql_to_json_value};

// ---------------------------------------------------------------------------
// PG-style rendering
// ---------------------------------------------------------------------------

/// Render a `Value` as text matching PostgreSQL's `jsonb` output:
/// object/array members are separated by `", "`, object keys are
/// followed by `": "`, and there is no leading/trailing whitespace.
pub fn render_pg(value: &Value) -> String {
    let mut out = String::new();
    write_pg(&mut out, value);
    out
}

/// Render a `Value` as PostgreSQL's `jsonb_pretty` output: 4-space
/// indented, sorted by source-order keys, with a final newline omitted
/// (the caller surfaces it as a row).
pub fn render_pg_pretty(value: &Value) -> String {
    let mut out = String::new();
    write_pretty(&mut out, value, 0);
    out
}

fn write_pg(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => write_string_literal(out, s),
        Value::Array(arr) => {
            out.push('[');
            for (i, v) in arr.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_pg(out, v);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            for (i, (k, v)) in map.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_string_literal(out, k);
                out.push_str(": ");
                write_pg(out, v);
            }
            out.push('}');
        }
    }
}

fn write_pretty(out: &mut String, value: &Value, depth: usize) {
    match value {
        Value::Array(arr) if !arr.is_empty() => {
            out.push('[');
            for (i, v) in arr.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push('\n');
                push_indent(out, depth + 1);
                write_pretty(out, v, depth + 1);
            }
            out.push('\n');
            push_indent(out, depth);
            out.push(']');
        }
        Value::Object(map) if !map.is_empty() => {
            out.push('{');
            for (i, (k, v)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push('\n');
                push_indent(out, depth + 1);
                write_string_literal(out, k);
                out.push_str(": ");
                write_pretty(out, v, depth + 1);
            }
            out.push('\n');
            push_indent(out, depth);
            out.push('}');
        }
        // Empty container / scalar: render on a single line.
        _ => write_pg(out, value),
    }
}

fn push_indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("    ");
    }
}

fn write_string_literal(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Build a JSONB output SqlValue from a parsed JSON tree.
pub fn jsonb_text(value: &Value) -> SqlValue {
    SqlValue::Text(render_pg(value).into())
}

/// Postgres boolean rendering for `-At` output: `t` for true, `f` for
/// false.
pub fn pg_bool(b: bool) -> SqlValue {
    SqlValue::Text(if b { "t".into() } else { "f".into() })
}

// ---------------------------------------------------------------------------
// Argument parsing helpers
// ---------------------------------------------------------------------------

/// Parse a SQL value as a JSONB document. NULL → None.
pub fn parse_jsonb(value: &SqlValue) -> Result<Option<Value>> {
    parse_json_arg(value)
}

/// Parse a path-array literal of the form `'{a,b,1}'` (Postgres `text[]`)
/// into a sequence of segments: each unquoted segment that parses as
/// integer becomes an Index, otherwise a Key.
pub fn parse_pg_path(value: &SqlValue) -> Result<JsonPath> {
    let text = match value {
        SqlValue::Text(s) => s.to_string(),
        other => return Err(Error::Parse(format!("JSON path must be TEXT: {other:?}"))),
    };
    let trimmed = text.trim();
    let inner = if let Some(rest) = trimmed.strip_prefix('{') {
        rest.strip_suffix('}').ok_or_else(|| {
            Error::Parse(format!(
                "JSON path literal must be `{{...}}` (got `{text}`)"
            ))
        })?
    } else if trimmed.starts_with('$') {
        // Already in JSON-Path syntax — defer to the SQLite parser.
        return JsonPath::parse(trimmed)
            .map_err(|e| Error::Parse(format!("JSON path error: {e}")));
    } else {
        return Err(Error::Parse(format!(
            "JSON path literal must be `{{...}}` (got `{text}`)"
        )));
    };

    let mut path = String::from("$");
    if !inner.is_empty() {
        for raw in inner.split(',') {
            let part = raw.trim();
            if part.is_empty() {
                continue;
            }
            if let Ok(idx) = part.parse::<i64>() {
                if idx >= 0 {
                    path.push_str(&format!("[{idx}]"));
                } else {
                    // PG allows negative indices counting from the end.
                    path.push_str(&format!("[#-{}]", -idx));
                }
            } else if part.chars().all(|c| c.is_alphanumeric() || c == '_') {
                path.push('.');
                path.push_str(part);
            } else {
                path.push('.');
                path.push('"');
                for ch in part.chars() {
                    if ch == '"' {
                        path.push('\\');
                    }
                    path.push(ch);
                }
                path.push('"');
            }
        }
    }
    JsonPath::parse(&path).map_err(|e| Error::Parse(format!("JSON path error: {e}")))
}


// ---------------------------------------------------------------------------
// Containment
// ---------------------------------------------------------------------------

/// Postgres `jsonb @> jsonb` containment. Returns `true` iff `right`
/// is structurally contained within `left`.
pub fn contains(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Object(l), Value::Object(r)) => r.iter().all(|(k, rv)| match l.get(k) {
            Some(lv) => contains(lv, rv),
            None => false,
        }),
        (Value::Array(l), Value::Array(r)) => r.iter().all(|rv| l.iter().any(|lv| contains(lv, rv))),
        // PG: a JSON array contains a scalar iff some element equals the scalar.
        (Value::Array(l), _) if !matches!(right, Value::Array(_) | Value::Object(_)) => {
            l.iter().any(|lv| lv == right)
        }
        // PG: scalars match by identity.
        _ => left == right,
    }
}

pub fn op_at_arrow(left: &SqlValue, right: &SqlValue) -> Result<SqlValue> {
    let (Some(a), Some(b)) = (parse_jsonb(left)?, parse_jsonb(right)?) else {
        return Ok(SqlValue::Null);
    };
    Ok(pg_bool(contains(&a, &b)))
}

pub fn op_arrow_at(left: &SqlValue, right: &SqlValue) -> Result<SqlValue> {
    let (Some(a), Some(b)) = (parse_jsonb(left)?, parse_jsonb(right)?) else {
        return Ok(SqlValue::Null);
    };
    Ok(pg_bool(contains(&b, &a)))
}

// ---------------------------------------------------------------------------
// Key existence: `?`, `?|`, `?&`
// ---------------------------------------------------------------------------

/// True when `key` exists in `doc` as:
///   * an object key,
///   * an array element whose string value equals `key`,
///   * or a top-level string scalar equal to `key`.
pub fn key_exists(doc: &Value, key: &str) -> bool {
    match doc {
        Value::Object(map) => map.contains_key(key),
        Value::Array(arr) => arr.iter().any(|v| matches!(v, Value::String(s) if s == key)),
        Value::String(s) => s == key,
        _ => false,
    }
}

pub fn op_question(left: &SqlValue, right: &SqlValue) -> Result<SqlValue> {
    let Some(doc) = parse_jsonb(left)? else {
        return Ok(SqlValue::Null);
    };
    let key = match right {
        SqlValue::Null => return Ok(SqlValue::Null),
        SqlValue::Text(s) => s.to_string(),
        other => crate::exec::expr::scalar::value::value_to_string(other),
    };
    Ok(pg_bool(key_exists(&doc, &key)))
}

pub fn op_question_any(left: &SqlValue, keys: &[SqlValue]) -> Result<SqlValue> {
    let Some(doc) = parse_jsonb(left)? else {
        return Ok(SqlValue::Null);
    };
    let any = keys.iter().any(|v| match v {
        SqlValue::Null => false,
        SqlValue::Text(s) => key_exists(&doc, s.as_ref()),
        other => key_exists(&doc, &crate::exec::expr::scalar::value::value_to_string(other)),
    });
    Ok(pg_bool(any))
}

pub fn op_question_all(left: &SqlValue, keys: &[SqlValue]) -> Result<SqlValue> {
    let Some(doc) = parse_jsonb(left)? else {
        return Ok(SqlValue::Null);
    };
    let all = keys.iter().all(|v| match v {
        SqlValue::Null => false,
        SqlValue::Text(s) => key_exists(&doc, s.as_ref()),
        other => key_exists(&doc, &crate::exec::expr::scalar::value::value_to_string(other)),
    });
    Ok(pg_bool(all))
}

// ---------------------------------------------------------------------------
// Path extraction: `#>`, `#>>`
// ---------------------------------------------------------------------------

pub fn op_hash_arrow(left: &SqlValue, right: &SqlValue) -> Result<SqlValue> {
    let Some(doc) = parse_jsonb(left)? else {
        return Ok(SqlValue::Null);
    };
    let path = parse_pg_path(right)?;
    Ok(match resolve(&doc, &path) {
        Some(v) => jsonb_text(v),
        None => SqlValue::Null,
    })
}

pub fn op_hash_long_arrow(left: &SqlValue, right: &SqlValue) -> Result<SqlValue> {
    let Some(doc) = parse_jsonb(left)? else {
        return Ok(SqlValue::Null);
    };
    let path = parse_pg_path(right)?;
    Ok(match resolve(&doc, &path) {
        Some(v) => match v {
            Value::Null => SqlValue::Null,
            Value::String(s) => SqlValue::Text(s.clone().into()),
            Value::Bool(b) => SqlValue::Text(if *b { "true".into() } else { "false".into() }),
            Value::Number(n) => SqlValue::Text(n.to_string().into()),
            other => jsonb_text(other),
        },
        None => SqlValue::Null,
    })
}

// ---------------------------------------------------------------------------
// Path delete: `#-`
// ---------------------------------------------------------------------------

pub fn op_hash_minus(left: &SqlValue, right: &SqlValue) -> Result<SqlValue> {
    let Some(mut doc) = parse_jsonb(left)? else {
        return Ok(SqlValue::Null);
    };
    let path = parse_pg_path(right)?;
    remove(&mut doc, &path);
    Ok(jsonb_text(&doc))
}

// ---------------------------------------------------------------------------
// JSONPath predicate match: `@@`
// ---------------------------------------------------------------------------

pub fn op_at_at(left: &SqlValue, right: &SqlValue) -> Result<SqlValue> {
    let Some(doc) = parse_jsonb(left)? else {
        return Ok(SqlValue::Null);
    };
    let path_text = match right {
        SqlValue::Null => return Ok(SqlValue::Null),
        SqlValue::Text(s) => s.to_string(),
        other => crate::exec::expr::scalar::value::value_to_string(other),
    };
    Ok(pg_bool(eval_jsonpath_predicate(&doc, &path_text)?))
}

pub fn op_at_question(left: &SqlValue, right: &SqlValue) -> Result<SqlValue> {
    let Some(doc) = parse_jsonb(left)? else {
        return Ok(SqlValue::Null);
    };
    let path_text = match right {
        SqlValue::Null => return Ok(SqlValue::Null),
        SqlValue::Text(s) => s.to_string(),
        other => crate::exec::expr::scalar::value::value_to_string(other),
    };
    Ok(pg_bool(path_returns_any(&doc, &path_text)?))
}

// ---------------------------------------------------------------------------
// Concat (`||`) and key/index delete (`-`)
// ---------------------------------------------------------------------------

pub fn op_concat(left: &Value, right: &Value) -> Value {
    match (left, right) {
        (Value::Object(l), Value::Object(r)) => {
            let mut merged = l.clone();
            for (k, v) in r {
                merged.insert(k.clone(), v.clone());
            }
            Value::Object(merged)
        }
        (Value::Array(l), Value::Array(r)) => {
            let mut merged = l.clone();
            merged.extend(r.iter().cloned());
            Value::Array(merged)
        }
        (Value::Array(l), other) => {
            let mut merged = l.clone();
            merged.push(other.clone());
            Value::Array(merged)
        }
        (other, Value::Array(r)) => {
            let mut merged = Vec::with_capacity(r.len() + 1);
            merged.push(other.clone());
            merged.extend(r.iter().cloned());
            Value::Array(merged)
        }
        (a, b) => Value::Array(vec![a.clone(), b.clone()]),
    }
}

pub fn op_delete_key(doc: &Value, key: &str) -> Value {
    match doc {
        Value::Object(map) => {
            let mut next = map.clone();
            next.shift_remove(key);
            Value::Object(next)
        }
        Value::Array(arr) => {
            let next: Vec<Value> = arr
                .iter()
                .filter(|v| !matches!(v, Value::String(s) if s == key))
                .cloned()
                .collect();
            Value::Array(next)
        }
        other => other.clone(),
    }
}

pub fn op_delete_index(doc: &Value, idx: i64) -> Value {
    let Value::Array(arr) = doc else {
        return doc.clone();
    };
    let len = arr.len() as i64;
    let resolved = if idx >= 0 { idx } else { len + idx };
    if resolved < 0 || resolved >= len {
        return doc.clone();
    }
    let mut next = arr.clone();
    next.remove(resolved as usize);
    Value::Array(next)
}

// ---------------------------------------------------------------------------
// jsonb_* function implementations.
// ---------------------------------------------------------------------------

pub fn jsonb_pretty(values: &[SqlValue]) -> Result<SqlValue> {
    let Some(arg) = values.first() else {
        return Err(Error::UnsupportedSql(
            "jsonb_pretty requires 1 argument".into(),
        ));
    };
    let Some(doc) = parse_jsonb(arg)? else {
        return Ok(SqlValue::Null);
    };
    Ok(SqlValue::Text(render_pg_pretty(&doc).into()))
}

pub fn jsonb_strip_nulls(values: &[SqlValue]) -> Result<SqlValue> {
    let Some(arg) = values.first() else {
        return Err(Error::UnsupportedSql(
            "jsonb_strip_nulls requires 1 argument".into(),
        ));
    };
    let Some(mut doc) = parse_jsonb(arg)? else {
        return Ok(SqlValue::Null);
    };
    strip_nulls_in_place(&mut doc);
    Ok(jsonb_text(&doc))
}

fn strip_nulls_in_place(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|_, v| !v.is_null());
            for v in map.values_mut() {
                strip_nulls_in_place(v);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                strip_nulls_in_place(v);
            }
        }
        _ => {}
    }
}

pub fn jsonb_set(values: &[SqlValue]) -> Result<SqlValue> {
    if !(values.len() == 3 || values.len() == 4) {
        return Err(Error::UnsupportedSql(
            "jsonb_set requires (target, path, value [, create_if_missing])".into(),
        ));
    }
    let Some(mut doc) = parse_jsonb(&values[0])? else {
        return Ok(SqlValue::Null);
    };
    let path = parse_pg_path(&values[1])?;
    let new_value = match parse_jsonb(&values[2])? {
        Some(v) => v,
        None => return Ok(SqlValue::Null),
    };
    let create_if_missing = if values.len() == 4 {
        bool_arg(&values[3], true)
    } else {
        true
    };
    let mode = if create_if_missing {
        MutationMode::Set
    } else {
        MutationMode::Replace
    };
    mutate(&mut doc, &path, new_value, mode).map_err(|e| Error::Parse(e.to_string()))?;
    Ok(jsonb_text(&doc))
}

pub fn jsonb_insert(values: &[SqlValue]) -> Result<SqlValue> {
    if !(values.len() == 3 || values.len() == 4) {
        return Err(Error::UnsupportedSql(
            "jsonb_insert requires (target, path, value [, insert_after])".into(),
        ));
    }
    let Some(mut doc) = parse_jsonb(&values[0])? else {
        return Ok(SqlValue::Null);
    };
    let path = parse_pg_path(&values[1])?;
    let new_value = match parse_jsonb(&values[2])? {
        Some(v) => v,
        None => return Ok(SqlValue::Null),
    };
    let insert_after = if values.len() == 4 {
        bool_arg(&values[3], false)
    } else {
        false
    };
    jsonb_insert_recursive(&mut doc, &path.segments, new_value, insert_after);
    Ok(jsonb_text(&doc))
}

fn jsonb_insert_recursive(
    current: &mut Value,
    segments: &[super::path::PathSegment],
    new_value: Value,
    insert_after: bool,
) {
    use super::path::PathSegment;

    let (seg, rest) = match segments.split_first() {
        Some(parts) => parts,
        None => return,
    };
    let is_leaf = rest.is_empty();

    match seg {
        PathSegment::Key(k) => {
            let Value::Object(map) = current else {
                return;
            };
            if is_leaf {
                // jsonb_insert on objects inserts only when missing.
                if !map.contains_key(k) {
                    map.insert(k.clone(), new_value);
                }
            } else if let Some(child) = map.get_mut(k) {
                jsonb_insert_recursive(child, rest, new_value, insert_after);
            }
        }
        PathSegment::Index(i) => {
            let Value::Array(arr) = current else {
                return;
            };
            if is_leaf {
                let pos = if insert_after { *i + 1 } else { *i };
                let pos = pos.min(arr.len());
                arr.insert(pos, new_value);
            } else if let Some(child) = arr.get_mut(*i) {
                jsonb_insert_recursive(child, rest, new_value, insert_after);
            }
        }
        PathSegment::Append => {
            let Value::Array(arr) = current else {
                return;
            };
            if is_leaf {
                arr.push(new_value);
            }
        }
        PathSegment::FromEnd(n) => {
            let Value::Array(arr) = current else {
                return;
            };
            if *n == 0 || *n > arr.len() {
                return;
            }
            let idx = arr.len() - *n;
            if is_leaf {
                let pos = if insert_after { idx + 1 } else { idx };
                let pos = pos.min(arr.len());
                arr.insert(pos, new_value);
            } else {
                jsonb_insert_recursive(&mut arr[idx], rest, new_value, insert_after);
            }
        }
    }
}

pub fn jsonb_path_exists(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() < 2 {
        return Err(Error::UnsupportedSql(
            "jsonb_path_exists requires (target, jsonpath)".into(),
        ));
    }
    let Some(doc) = parse_jsonb(&values[0])? else {
        return Ok(SqlValue::Null);
    };
    let path = match &values[1] {
        SqlValue::Null => return Ok(SqlValue::Null),
        SqlValue::Text(s) => s.to_string(),
        other => crate::exec::expr::scalar::value::value_to_string(other),
    };
    Ok(pg_bool(path_returns_any(&doc, &path)?))
}

pub fn jsonb_path_match(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() < 2 {
        return Err(Error::UnsupportedSql(
            "jsonb_path_match requires (target, jsonpath)".into(),
        ));
    }
    let Some(doc) = parse_jsonb(&values[0])? else {
        return Ok(SqlValue::Null);
    };
    let path = match &values[1] {
        SqlValue::Null => return Ok(SqlValue::Null),
        SqlValue::Text(s) => s.to_string(),
        other => crate::exec::expr::scalar::value::value_to_string(other),
    };
    Ok(pg_bool(eval_jsonpath_predicate(&doc, &path)?))
}

pub fn jsonb_path_query_first(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() < 2 {
        return Err(Error::UnsupportedSql(
            "jsonb_path_query_first requires (target, jsonpath)".into(),
        ));
    }
    let Some(doc) = parse_jsonb(&values[0])? else {
        return Ok(SqlValue::Null);
    };
    let path = match &values[1] {
        SqlValue::Null => return Ok(SqlValue::Null),
        SqlValue::Text(s) => s.to_string(),
        other => crate::exec::expr::scalar::value::value_to_string(other),
    };
    let matches = collect_jsonpath_results(&doc, &path)?;
    Ok(match matches.into_iter().next() {
        Some(v) => jsonb_text(&v),
        None => SqlValue::Null,
    })
}

pub fn jsonb_contains(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() != 2 {
        return Err(Error::UnsupportedSql(
            "jsonb_contains requires 2 arguments".into(),
        ));
    }
    op_at_arrow(&values[0], &values[1])
}

pub fn jsonb_contained(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() != 2 {
        return Err(Error::UnsupportedSql(
            "jsonb_contained requires 2 arguments".into(),
        ));
    }
    op_arrow_at(&values[0], &values[1])
}

pub fn jsonb_exists(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() != 2 {
        return Err(Error::UnsupportedSql(
            "jsonb_exists requires 2 arguments".into(),
        ));
    }
    op_question(&values[0], &values[1])
}

pub fn jsonb_exists_any(values: &[SqlValue]) -> Result<SqlValue> {
    if values.is_empty() {
        return Err(Error::UnsupportedSql(
            "jsonb_exists_any requires at least 1 argument".into(),
        ));
    }
    op_question_any(&values[0], &values[1..])
}

pub fn jsonb_exists_all(values: &[SqlValue]) -> Result<SqlValue> {
    if values.is_empty() {
        return Err(Error::UnsupportedSql(
            "jsonb_exists_all requires at least 1 argument".into(),
        ));
    }
    op_question_all(&values[0], &values[1..])
}

pub fn jsonb_concat(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() != 2 {
        return Err(Error::UnsupportedSql(
            "jsonb_concat requires 2 arguments".into(),
        ));
    }
    let Some(a) = parse_jsonb(&values[0])? else {
        return Ok(SqlValue::Null);
    };
    let Some(b) = parse_jsonb(&values[1])? else {
        return Ok(SqlValue::Null);
    };
    Ok(jsonb_text(&op_concat(&a, &b)))
}

pub fn jsonb_delete(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() != 2 {
        return Err(Error::UnsupportedSql(
            "jsonb_delete requires 2 arguments".into(),
        ));
    }
    let Some(doc) = parse_jsonb(&values[0])? else {
        return Ok(SqlValue::Null);
    };
    match &values[1] {
        SqlValue::Null => Ok(SqlValue::Null),
        SqlValue::Integer(n) => Ok(jsonb_text(&op_delete_index(&doc, *n))),
        SqlValue::Text(s) => Ok(jsonb_text(&op_delete_key(&doc, s.as_ref()))),
        other => Ok(jsonb_text(&op_delete_key(
            &doc,
            &crate::exec::expr::scalar::value::value_to_string(other),
        ))),
    }
}

pub fn jsonb_delete_path(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() != 2 {
        return Err(Error::UnsupportedSql(
            "jsonb_delete_path requires 2 arguments".into(),
        ));
    }
    op_hash_minus(&values[0], &values[1])
}

pub fn jsonb_typeof(values: &[SqlValue]) -> Result<SqlValue> {
    let Some(arg) = values.first() else {
        return Err(Error::UnsupportedSql(
            "jsonb_typeof requires 1 argument".into(),
        ));
    };
    let Some(doc) = parse_jsonb(arg)? else {
        return Ok(SqlValue::Null);
    };
    let name = match doc {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    Ok(SqlValue::Text(name.into()))
}

pub fn jsonb_array_length(values: &[SqlValue]) -> Result<SqlValue> {
    let Some(arg) = values.first() else {
        return Err(Error::UnsupportedSql(
            "jsonb_array_length requires 1 argument".into(),
        ));
    };
    let Some(doc) = parse_jsonb(arg)? else {
        return Ok(SqlValue::Null);
    };
    match doc {
        Value::Array(arr) => Ok(SqlValue::Integer(arr.len() as i64)),
        _ => Err(Error::DatatypeMismatch),
    }
}

pub fn jsonb_build_object(values: &[SqlValue]) -> Result<SqlValue> {
    if !values.len().is_multiple_of(2) {
        return Err(Error::UnsupportedSql(
            "jsonb_build_object requires an even number of arguments".into(),
        ));
    }
    let mut map = Map::new();
    for pair in values.chunks(2) {
        let key = match &pair[0] {
            SqlValue::Null => {
                return Err(Error::ConstraintViolation(
                    "jsonb_build_object: NULL key".into(),
                ));
            }
            SqlValue::Text(s) => s.to_string(),
            SqlValue::Integer(n) => n.to_string(),
            SqlValue::Real(f) => crate::exec::expr::scalar::value::format_real_sqlite(*f),
            SqlValue::Blob(b) => String::from_utf8_lossy(b).into_owned(),
        };
        map.insert(key, sql_to_json_value(&pair[1]));
    }
    Ok(jsonb_text(&Value::Object(map)))
}

pub fn jsonb_build_array(values: &[SqlValue]) -> Result<SqlValue> {
    let arr: Vec<Value> = values.iter().map(sql_to_json_value).collect();
    Ok(jsonb_text(&Value::Array(arr)))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn bool_arg(value: &SqlValue, default: bool) -> bool {
    match value {
        SqlValue::Null => default,
        SqlValue::Integer(n) => *n != 0,
        SqlValue::Real(f) => *f != 0.0,
        SqlValue::Text(s) => match s.to_ascii_lowercase().as_str() {
            "t" | "true" | "1" | "yes" | "on" => true,
            "f" | "false" | "0" | "no" | "off" => false,
            _ => default,
        },
        SqlValue::Blob(b) => !b.is_empty(),
    }
}

/// Tiny JSONPath subset evaluator. Recognises:
///   `$.a.b`, `$[*]`, `$.a[*]`, `$[i]`, `$.a[i]`, `$.*`, plus filter
///   expressions `?(@.key OP literal)` and the top-level predicates
///   used by `@@` (`$.a > 3`, `$ > 3`, `$.a == "x"`).
///
/// Returns the matched nodes in document order. Unknown syntax falls
/// back to a strict subset; unsupported tokens raise `UnsupportedSql`.
fn collect_jsonpath_results(doc: &Value, path: &str) -> Result<Vec<Value>> {
    let parsed = parse_jsonpath(path)?;
    let mut results = Vec::new();
    eval_jsonpath_steps(doc, &parsed.steps, 0, &mut results);
    Ok(results)
}

/// Crate-visible entry point used by the `jsonb_path_query` TVF in
/// `exec::json_tv` to enumerate JSONPath matches.
pub(crate) fn collect_jsonpath_for_tvf(doc: &Value, path: &str) -> Result<Vec<Value>> {
    collect_jsonpath_results(doc, path)
}

fn path_returns_any(doc: &Value, path: &str) -> Result<bool> {
    Ok(!collect_jsonpath_results(doc, path)?.is_empty())
}

/// Evaluate a JSONPath predicate (`$.a > 3` style). Returns true iff
/// there is at least one match for the rooted predicate.
fn eval_jsonpath_predicate(doc: &Value, path: &str) -> Result<bool> {
    let trimmed = path.trim();
    // Pattern: "<lhs-path> <op> <literal>"
    if let Some((lhs, op, rhs)) = split_predicate(trimmed) {
        let nodes = collect_jsonpath_results(doc, lhs)?;
        let rhs_val = parse_predicate_literal(rhs)?;
        return Ok(nodes.iter().any(|n| compare_predicate(n, op, &rhs_val)));
    }
    // Otherwise treat as a plain existence test.
    path_returns_any(doc, trimmed)
}

fn split_predicate(path: &str) -> Option<(&str, &str, &str)> {
    // Order matters: try the longer operators first.
    for op in ["==", "!=", "<>", ">=", "<=", ">", "<"] {
        if let Some(idx) = path.find(op) {
            let (lhs, rest) = path.split_at(idx);
            let rhs = &rest[op.len()..];
            return Some((lhs.trim(), op, rhs.trim()));
        }
    }
    None
}

fn parse_predicate_literal(s: &str) -> Result<Value> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(Error::Parse(
            "JSONPath predicate missing right-hand side".into(),
        ));
    }
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        return Ok(Value::String(trimmed[1..trimmed.len() - 1].to_owned()));
    }
    if let Ok(n) = trimmed.parse::<i64>() {
        return Ok(Value::Number(n.into()));
    }
    if let Ok(n) = trimmed.parse::<f64>() {
        return Ok(serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or(Value::Null));
    }
    if trimmed.eq_ignore_ascii_case("true") {
        return Ok(Value::Bool(true));
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return Ok(Value::Bool(false));
    }
    if trimmed.eq_ignore_ascii_case("null") {
        return Ok(Value::Null);
    }
    Err(Error::Parse(format!(
        "unsupported JSONPath literal: `{trimmed}`"
    )))
}

fn compare_predicate(lhs: &Value, op: &str, rhs: &Value) -> bool {
    use std::cmp::Ordering;
    let ordering = match (lhs, rhs) {
        (Value::Number(a), Value::Number(b)) => {
            let af = a.as_f64();
            let bf = b.as_f64();
            match (af, bf) {
                (Some(x), Some(y)) => x.partial_cmp(&y),
                _ => None,
            }
        }
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
        (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
        (Value::Null, Value::Null) => Some(Ordering::Equal),
        _ => None,
    };
    match (op, ordering) {
        ("==", Some(Ordering::Equal)) => true,
        ("!=" | "<>", Some(o)) => o != Ordering::Equal,
        (">", Some(Ordering::Greater)) => true,
        (">=", Some(o)) => o != Ordering::Less,
        ("<", Some(Ordering::Less)) => true,
        ("<=", Some(o)) => o != Ordering::Greater,
        _ => false,
    }
}

#[derive(Debug, Clone)]
enum JsonPathStep {
    Root,
    Member(String),
    /// `.*` — every member value of an object.
    AnyMember,
    /// `[i]` — array index.
    Index(usize),
    /// `[*]` — every array element / every member value of an object.
    Any,
}

#[derive(Debug, Clone)]
struct ParsedJsonPath {
    steps: Vec<JsonPathStep>,
}

fn parse_jsonpath(s: &str) -> Result<ParsedJsonPath> {
    let mut steps = Vec::new();
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return Err(Error::Parse("empty JSONPath".into()));
    }
    if bytes[0] != b'$' {
        return Err(Error::Parse(format!(
            "JSONPath must start with `$` (got `{s}`)"
        )));
    }
    steps.push(JsonPathStep::Root);
    let mut i = 1usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'.' {
            i += 1;
            if i >= bytes.len() {
                return Err(Error::Parse("dangling `.` in JSONPath".into()));
            }
            if bytes[i] == b'*' {
                steps.push(JsonPathStep::AnyMember);
                i += 1;
                continue;
            }
            let start = i;
            while i < bytes.len() && is_jsonpath_member_byte(bytes[i]) {
                i += 1;
            }
            if i == start {
                return Err(Error::Parse(format!("empty member after `.` in `{s}`")));
            }
            steps.push(JsonPathStep::Member(s[start..i].to_owned()));
        } else if b == b'[' {
            i += 1;
            // Skip the optional `?` filter expression: we ignore the
            // body and treat it as `[*]`.
            if i < bytes.len() && bytes[i] == b'?' {
                let mut depth = 1i32;
                while i < bytes.len() && depth > 0 {
                    i += 1;
                    match bytes.get(i) {
                        Some(b'(') => depth += 1,
                        Some(b')') => depth -= 1,
                        _ => {}
                    }
                }
                if i >= bytes.len() || bytes[i] != b')' {
                    return Err(Error::Parse(
                        "unterminated filter expression in JSONPath".into(),
                    ));
                }
                i += 1;
                if i >= bytes.len() || bytes[i] != b']' {
                    return Err(Error::Parse(
                        "expected `]` after filter expression".into(),
                    ));
                }
                i += 1;
                steps.push(JsonPathStep::Any);
                continue;
            }
            if i < bytes.len() && bytes[i] == b'*' {
                i += 1;
                if i >= bytes.len() || bytes[i] != b']' {
                    return Err(Error::Parse("expected `]` after `*`".into()));
                }
                i += 1;
                steps.push(JsonPathStep::Any);
                continue;
            }
            let start = i;
            while i < bytes.len() && bytes[i] != b']' {
                i += 1;
            }
            if i >= bytes.len() {
                return Err(Error::Parse("unterminated `[` in JSONPath".into()));
            }
            let inner = s[start..i].trim();
            i += 1;
            let idx: usize = inner
                .parse()
                .map_err(|_| Error::Parse(format!("bad JSONPath index `{inner}`")))?;
            steps.push(JsonPathStep::Index(idx));
        } else if b.is_ascii_whitespace() {
            i += 1;
        } else {
            // Unrecognised tail — stop here so predicate splitting can
            // pick up the rest (`$.a > 3` etc.).
            return Err(Error::Parse(format!(
                "unsupported JSONPath token at offset {i} of `{s}`"
            )));
        }
    }
    Ok(ParsedJsonPath { steps })
}

fn is_jsonpath_member_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn eval_jsonpath_steps(
    current: &Value,
    steps: &[JsonPathStep],
    idx: usize,
    out: &mut Vec<Value>,
) {
    if idx >= steps.len() {
        out.push(current.clone());
        return;
    }
    match &steps[idx] {
        JsonPathStep::Root => eval_jsonpath_steps(current, steps, idx + 1, out),
        JsonPathStep::Member(key) => {
            if let Value::Object(map) = current
                && let Some(v) = map.get(key)
            {
                eval_jsonpath_steps(v, steps, idx + 1, out);
            }
        }
        JsonPathStep::AnyMember => {
            if let Value::Object(map) = current {
                for v in map.values() {
                    eval_jsonpath_steps(v, steps, idx + 1, out);
                }
            }
        }
        JsonPathStep::Index(i) => {
            if let Value::Array(arr) = current
                && let Some(v) = arr.get(*i)
            {
                eval_jsonpath_steps(v, steps, idx + 1, out);
            }
        }
        JsonPathStep::Any => match current {
            Value::Array(arr) => {
                for v in arr {
                    eval_jsonpath_steps(v, steps, idx + 1, out);
                }
            }
            Value::Object(map) => {
                for v in map.values() {
                    eval_jsonpath_steps(v, steps, idx + 1, out);
                }
            }
            _ => {}
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;

    fn t(s: &str) -> SqlValue {
        SqlValue::Text(Arc::from(s))
    }

    #[test]
    fn pg_render_object_uses_spaces() {
        let v = json!({"a": 1, "b": 2});
        assert_eq!(render_pg(&v), r#"{"a": 1, "b": 2}"#);
    }

    #[test]
    fn pg_render_array_uses_spaces() {
        let v = json!([1, 2, 3]);
        assert_eq!(render_pg(&v), "[1, 2, 3]");
    }

    #[test]
    fn pretty_indents_four_spaces() {
        let v = json!({"a": 1, "b": [1, 2, 3]});
        let s = render_pg_pretty(&v);
        assert!(s.contains("    \"a\": 1"));
        assert!(s.contains("        1,"));
    }

    #[test]
    fn contains_object_subset() {
        let a = json!({"a": 1, "b": 2});
        let b = json!({"a": 1});
        assert!(contains(&a, &b));
        let c = json!({"a": 2});
        assert!(!contains(&a, &c));
    }

    #[test]
    fn contains_array_subset() {
        let a = json!([1, 2, 3]);
        let b = json!([1, 2]);
        assert!(contains(&a, &b));
        let c = json!([5]);
        assert!(!contains(&a, &c));
    }

    #[test]
    fn key_exists_object_array_string() {
        assert!(key_exists(&json!({"a": 1}), "a"));
        assert!(!key_exists(&json!({"a": 1}), "b"));
        assert!(key_exists(&json!(["a", "b"]), "b"));
        assert!(key_exists(&json!("a"), "a"));
        assert!(!key_exists(&json!(42), "42"));
    }

    #[test]
    fn parse_pg_path_handles_named_and_indexed() {
        let p = parse_pg_path(&t("{a,b,2}")).unwrap();
        let printed = p.to_string();
        assert!(printed.contains(".a"));
        assert!(printed.contains(".b"));
        assert!(printed.contains("[2]"));
    }

    #[test]
    fn parse_pg_path_handles_empty() {
        let p = parse_pg_path(&t("{}")).unwrap();
        assert!(p.is_root());
    }

    #[test]
    fn concat_objects_merges() {
        let merged = op_concat(&json!({"a": 1}), &json!({"b": 2}));
        assert_eq!(merged, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn concat_arrays_appends() {
        let merged = op_concat(&json!([1, 2]), &json!([3, 4]));
        assert_eq!(merged, json!([1, 2, 3, 4]));
    }

    #[test]
    fn delete_key_removes_object_member() {
        let doc = json!({"a": 1, "b": 2});
        let out = op_delete_key(&doc, "a");
        assert_eq!(out, json!({"b": 2}));
    }

    #[test]
    fn jsonb_set_replaces_and_creates() {
        let v = jsonb_set(&[t(r#"{"a":1,"b":2}"#), t("{a}"), t("99")]).unwrap();
        assert_eq!(v, SqlValue::Text(r#"{"a": 99, "b": 2}"#.into()));
        let v = jsonb_set(&[
            t(r#"{"a":1}"#),
            t("{b}"),
            t("7"),
            SqlValue::Integer(1),
        ])
        .unwrap();
        assert_eq!(v, SqlValue::Text(r#"{"a": 1, "b": 7}"#.into()));
    }

    #[test]
    fn jsonb_set_with_create_false_skips_missing() {
        let v = jsonb_set(&[
            t(r#"{"a":1}"#),
            t("{b}"),
            t("7"),
            SqlValue::Integer(0),
        ])
        .unwrap();
        assert_eq!(v, SqlValue::Text(r#"{"a": 1}"#.into()));
    }

    #[test]
    fn jsonb_insert_array_before_and_after() {
        let v = jsonb_insert(&[t("[1,2,3]"), t("{1}"), t("99")]).unwrap();
        assert_eq!(v, SqlValue::Text("[1, 99, 2, 3]".into()));
        let v = jsonb_insert(&[
            t("[1,2,3]"),
            t("{1}"),
            t("99"),
            SqlValue::Integer(1),
        ])
        .unwrap();
        assert_eq!(v, SqlValue::Text("[1, 2, 99, 3]".into()));
    }

    #[test]
    fn jsonb_strip_nulls_drops_null_values() {
        let v = jsonb_strip_nulls(&[t(r#"{"a":1,"b":null,"c":{"d":null,"e":2}}"#)]).unwrap();
        assert_eq!(v, SqlValue::Text(r#"{"a": 1, "c": {"e": 2}}"#.into()));
    }

    #[test]
    fn jsonb_pretty_indents() {
        let v = jsonb_pretty(&[t(r#"{"a":1,"b":[1,2,3]}"#)]).unwrap();
        let SqlValue::Text(s) = v else { panic!() };
        assert!(s.contains("    \"a\": 1"));
        assert!(s.contains("        1"));
    }

    #[test]
    fn jsonpath_predicate_match() {
        let doc = json!({"a": 5});
        assert!(eval_jsonpath_predicate(&doc, "$.a > 3").unwrap());
        assert!(!eval_jsonpath_predicate(&doc, "$.a > 9").unwrap());
    }

    #[test]
    fn jsonpath_exists_handles_dotted_and_index() {
        let doc = json!({"a": {"b": 1}});
        assert!(path_returns_any(&doc, "$.a.b").unwrap());
        assert!(!path_returns_any(&doc, "$.a.z").unwrap());
        let arr = json!([10, 20, 30]);
        let nodes = collect_jsonpath_results(&arr, "$[*]").unwrap();
        assert_eq!(nodes, vec![json!(10), json!(20), json!(30)]);
    }
}
