//! SQLite JSON1 scalar function implementations.
//!
//! Each function maps from `&[SqlValue]` -> `Result<SqlValue, Error>` and
//! follows SQLite's documented behaviour as closely as practical.
//!
//! Type coercion: SQL TEXT values are parsed as JSON; INTEGER/REAL become
//! the obvious JSON scalars; NULL propagates to NULL except where the
//! operation specifically defines another rule (e.g. `json_object` keys
//! must not be NULL).

use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::value::SqlValue;

use super::path::{JsonPath, MutationMode, PathError, mutate, remove, resolve};

// ---------------------------------------------------------------------------
// Type adapters
// ---------------------------------------------------------------------------

/// Convert a SQL value into the JSON value it would represent inside a
/// JSON-building function (json_array, json_object, json_set, ...).
///
/// Per SQLite docs: text arguments are taken as **JSON values** if they
/// parse as valid JSON, otherwise as quoted JSON strings. INTEGER/REAL go
/// straight to numbers, NULL → null, BLOB → quoted base16 string (we use
/// a straight string repr for simplicity, matching SQLite's behaviour for
/// text-affinity).
///
/// `as_json_argument`: text is *always* a JSON string (used by the input
/// arg of json/json_extract, where the whole text is the JSON document).
pub(crate) fn sql_to_json_value(value: &SqlValue) -> Value {
    match value {
        SqlValue::Null => Value::Null,
        SqlValue::Integer(n) => Value::Number((*n).into()),
        SqlValue::Real(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        SqlValue::Text(s) => {
            // Per SQLite: text args to json_array/json_object are stored
            // as JSON strings unless they were produced by another JSON
            // function. We don't track provenance, so we always treat
            // text as a string literal (the safer default; users can call
            // json(text) to splice raw JSON).
            Value::String(s.to_string())
        }
        SqlValue::Blob(b) => Value::String(String::from_utf8_lossy(b).into_owned()),
    }
}

/// Parse a SQL TEXT argument as a JSON document. NULL propagates as None.
pub(crate) fn parse_json_arg(value: &SqlValue) -> Result<Option<Value>> {
    match value {
        SqlValue::Null => Ok(None),
        SqlValue::Integer(n) => Ok(Some(Value::Number((*n).into()))),
        SqlValue::Real(f) => Ok(Some(
            serde_json::Number::from_f64(*f)
                .map(Value::Number)
                .unwrap_or(Value::Null),
        )),
        SqlValue::Text(s) => match serde_json::from_str(s.as_ref()) {
            Ok(v) => Ok(Some(v)),
            Err(e) => Err(Error::Parse(format!("malformed JSON: {e}"))),
        },
        SqlValue::Blob(b) => {
            let s = std::str::from_utf8(b)
                .map_err(|e| Error::Parse(format!("invalid UTF-8 in JSON blob: {e}")))?;
            match serde_json::from_str(s) {
                Ok(v) => Ok(Some(v)),
                Err(e) => Err(Error::Parse(format!("malformed JSON: {e}"))),
            }
        }
    }
}

/// Render a JSON value as TEXT (compact form, no extra whitespace).
pub(crate) fn render_json(value: &Value) -> SqlValue {
    SqlValue::Text(serde_json::to_string(value).unwrap_or_default().into())
}

/// Project a JSON value into a SQL value (used by `->>` and `json_extract`
/// when called with a single path: scalars unwrap, objects/arrays render
/// as JSON text).
pub(crate) fn json_to_sql(value: &Value) -> SqlValue {
    match value {
        Value::Null => SqlValue::Null,
        Value::Bool(b) => SqlValue::Integer(if *b { 1 } else { 0 }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                SqlValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                SqlValue::Real(f)
            } else {
                SqlValue::Null
            }
        }
        Value::String(s) => SqlValue::Text(s.clone().into()),
        Value::Array(_) | Value::Object(_) => render_json(value),
    }
}

fn parse_path(value: &SqlValue) -> Result<JsonPath> {
    let SqlValue::Text(s) = value else {
        return Err(Error::Parse("JSON path must be TEXT".into()));
    };
    JsonPath::parse(s.as_ref()).map_err(path_error)
}

fn path_error(e: PathError) -> Error {
    Error::Parse(format!("JSON path error: {e}"))
}

// ---------------------------------------------------------------------------
// Scalar functions
// ---------------------------------------------------------------------------

pub fn json_func(values: &[SqlValue]) -> Result<SqlValue> {
    let arg = values
        .first()
        .ok_or_else(|| Error::UnsupportedSql("json() requires 1 argument".into()))?;
    match parse_json_arg(arg)? {
        Some(v) => Ok(render_json(&v)),
        None => Ok(SqlValue::Null),
    }
}

pub fn json_array(values: &[SqlValue]) -> Result<SqlValue> {
    let arr: Vec<Value> = values.iter().map(sql_to_json_value).collect();
    Ok(render_json(&Value::Array(arr)))
}

pub fn json_array_length(values: &[SqlValue]) -> Result<SqlValue> {
    if values.is_empty() {
        return Err(Error::UnsupportedSql(
            "json_array_length requires at least one argument".into(),
        ));
    }
    let Some(json) = parse_json_arg(&values[0])? else {
        return Ok(SqlValue::Null);
    };
    let target = if values.len() >= 2 {
        let path = parse_path(&values[1])?;
        match resolve(&json, &path) {
            Some(v) => v,
            None => return Ok(SqlValue::Null),
        }
    } else {
        &json
    };
    Ok(match target {
        Value::Array(a) => SqlValue::Integer(a.len() as i64),
        _ => SqlValue::Integer(0),
    })
}

pub fn json_object(values: &[SqlValue]) -> Result<SqlValue> {
    if !values.len().is_multiple_of(2) {
        return Err(Error::UnsupportedSql(
            "json_object requires an even number of arguments".into(),
        ));
    }
    let mut map = Map::new();
    for pair in values.chunks(2) {
        let key = match &pair[0] {
            SqlValue::Null => {
                return Err(Error::ConstraintViolation("json_object: NULL key".into()));
            }
            SqlValue::Text(s) => s.to_string(),
            SqlValue::Integer(n) => n.to_string(),
            SqlValue::Real(f) => f.to_string(),
            SqlValue::Blob(b) => String::from_utf8_lossy(b).into_owned(),
        };
        map.insert(key, sql_to_json_value(&pair[1]));
    }
    Ok(render_json(&Value::Object(map)))
}

pub fn json_extract(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() < 2 {
        return Err(Error::UnsupportedSql(
            "json_extract requires at least 2 arguments".into(),
        ));
    }
    let Some(doc) = parse_json_arg(&values[0])? else {
        return Ok(SqlValue::Null);
    };

    if values.len() == 2 {
        let path = parse_path(&values[1])?;
        return Ok(match resolve(&doc, &path) {
            Some(v) => json_to_sql(v),
            None => SqlValue::Null,
        });
    }

    // Multiple paths: produce a JSON array of the resolved values (NULL
    // for missing). Per SQLite, scalars are kept as-is in the array.
    let mut out = Vec::with_capacity(values.len() - 1);
    for path_arg in &values[1..] {
        let path = parse_path(path_arg)?;
        out.push(match resolve(&doc, &path) {
            Some(v) => v.clone(),
            None => Value::Null,
        });
    }
    Ok(render_json(&Value::Array(out)))
}

pub fn json_set(values: &[SqlValue]) -> Result<SqlValue> {
    apply_mutation(values, MutationMode::Set, "json_set")
}

pub fn json_insert(values: &[SqlValue]) -> Result<SqlValue> {
    apply_mutation(values, MutationMode::Insert, "json_insert")
}

pub fn json_replace(values: &[SqlValue]) -> Result<SqlValue> {
    apply_mutation(values, MutationMode::Replace, "json_replace")
}

fn apply_mutation(values: &[SqlValue], mode: MutationMode, name: &str) -> Result<SqlValue> {
    if values.is_empty() || !(values.len() - 1).is_multiple_of(2) {
        return Err(Error::UnsupportedSql(format!(
            "{name} requires (json, path, value, ...)"
        )));
    }
    let Some(mut doc) = parse_json_arg(&values[0])? else {
        return Ok(SqlValue::Null);
    };
    let mut i = 1usize;
    while i + 1 < values.len() {
        let path = parse_path(&values[i])?;
        let value = sql_to_json_value(&values[i + 1]);
        mutate(&mut doc, &path, value, mode).map_err(path_error)?;
        i += 2;
    }
    Ok(render_json(&doc))
}

pub fn json_remove(values: &[SqlValue]) -> Result<SqlValue> {
    if values.is_empty() {
        return Err(Error::UnsupportedSql(
            "json_remove requires at least 1 argument".into(),
        ));
    }
    let Some(mut doc) = parse_json_arg(&values[0])? else {
        return Ok(SqlValue::Null);
    };
    for path_arg in &values[1..] {
        let path = parse_path(path_arg)?;
        remove(&mut doc, &path);
    }
    Ok(render_json(&doc))
}

pub fn json_patch(values: &[SqlValue]) -> Result<SqlValue> {
    if values.len() != 2 {
        return Err(Error::UnsupportedSql(
            "json_patch requires (target, patch)".into(),
        ));
    }
    let Some(mut target) = parse_json_arg(&values[0])? else {
        return Ok(SqlValue::Null);
    };
    let Some(patch) = parse_json_arg(&values[1])? else {
        return Ok(SqlValue::Null);
    };
    apply_merge_patch(&mut target, patch);
    Ok(render_json(&target))
}

/// RFC 7396 JSON Merge Patch.
fn apply_merge_patch(target: &mut Value, patch: Value) {
    match patch {
        Value::Object(patch_map) => {
            if !target.is_object() {
                *target = Value::Object(Map::new());
            }
            let target_map = target.as_object_mut().unwrap();
            for (k, v) in patch_map {
                if v.is_null() {
                    target_map.remove(&k);
                } else if let Some(existing) = target_map.get_mut(&k) {
                    apply_merge_patch(existing, v);
                } else {
                    let mut sub = Value::Object(Map::new());
                    apply_merge_patch(&mut sub, v);
                    target_map.insert(k, sub);
                }
            }
        }
        other => *target = other,
    }
}

pub fn json_type(values: &[SqlValue]) -> Result<SqlValue> {
    if values.is_empty() {
        return Err(Error::UnsupportedSql(
            "json_type requires at least 1 argument".into(),
        ));
    }
    let Some(doc) = parse_json_arg(&values[0])? else {
        return Ok(SqlValue::Null);
    };
    let target = if values.len() >= 2 {
        let path = parse_path(&values[1])?;
        match resolve(&doc, &path) {
            Some(v) => v,
            None => return Ok(SqlValue::Null),
        }
    } else {
        &doc
    };
    Ok(SqlValue::Text(json_type_name(target).into()))
}

fn json_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(true) => "true",
        Value::Bool(false) => "false",
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "integer"
            } else {
                "real"
            }
        }
        Value::String(_) => "text",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

pub fn json_valid(values: &[SqlValue]) -> Result<SqlValue> {
    let arg = values
        .first()
        .ok_or_else(|| Error::UnsupportedSql("json_valid requires 1 argument".into()))?;
    let s = match arg {
        SqlValue::Null => return Ok(SqlValue::Null),
        SqlValue::Text(s) => s.to_string(),
        SqlValue::Blob(b) => match std::str::from_utf8(b) {
            Ok(s) => s.to_owned(),
            Err(_) => return Ok(SqlValue::Integer(0)),
        },
        // Per SQLite: numerics are valid JSON.
        SqlValue::Integer(_) | SqlValue::Real(_) => return Ok(SqlValue::Integer(1)),
    };
    let ok = serde_json::from_str::<Value>(&s).is_ok();
    Ok(SqlValue::Integer(if ok { 1 } else { 0 }))
}

pub fn json_quote(values: &[SqlValue]) -> Result<SqlValue> {
    let arg = values
        .first()
        .ok_or_else(|| Error::UnsupportedSql("json_quote requires 1 argument".into()))?;
    Ok(render_json(&sql_to_json_value(arg)))
}

/// Re-export `json_func` under the alias `json_minify` for symmetry with
/// the documented function set; SQLite uses `json()` as the canonical
/// minifier, but some tooling expects this name.
pub fn json_minify(values: &[SqlValue]) -> Result<SqlValue> {
    json_func(values)
}

// ---------------------------------------------------------------------------
// `->` and `->>` operator helpers (called from the binary-op dispatch).
// ---------------------------------------------------------------------------

/// `json -> path`: returns a JSON-typed value (objects/arrays keep their
/// structure as TEXT, scalars are still rendered as JSON).
pub fn arrow_json(left: &SqlValue, right: &SqlValue) -> Result<SqlValue> {
    let Some(doc) = parse_json_arg(left)? else {
        return Ok(SqlValue::Null);
    };
    let path = arrow_path(right)?;
    Ok(match resolve(&doc, &path) {
        Some(v) => render_json(v),
        None => SqlValue::Null,
    })
}

/// `json ->> path`: returns the SQL value (scalars unwrap, structures
/// render as JSON TEXT).
pub fn arrow_sql(left: &SqlValue, right: &SqlValue) -> Result<SqlValue> {
    let Some(doc) = parse_json_arg(left)? else {
        return Ok(SqlValue::Null);
    };
    let path = arrow_path(right)?;
    Ok(match resolve(&doc, &path) {
        Some(v) => json_to_sql(v),
        None => SqlValue::Null,
    })
}

/// SQLite accepts shorthand path syntax for the `->` operators: a bare
/// integer N becomes `$[N]`, a bare identifier `key` becomes `$.key`.
fn arrow_path(value: &SqlValue) -> Result<JsonPath> {
    match value {
        SqlValue::Text(s) => {
            let s = s.as_ref();
            if s.starts_with('$') {
                JsonPath::parse(s).map_err(path_error)
            } else {
                JsonPath::parse(&format!("$.{s}")).map_err(path_error)
            }
        }
        SqlValue::Integer(n) => JsonPath::parse(&format!("$[{n}]")).map_err(path_error),
        _ => Err(Error::Parse("arrow path must be TEXT or INTEGER".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn t(s: &str) -> SqlValue {
        SqlValue::Text(Arc::from(s))
    }

    #[test]
    fn json_validates_and_minifies() {
        let v = json_func(&[t("  { \"a\" : 1 }  ")]).unwrap();
        assert_eq!(v, t(r#"{"a":1}"#));
    }

    #[test]
    fn json_propagates_null() {
        let v = json_func(&[SqlValue::Null]).unwrap();
        assert_eq!(v, SqlValue::Null);
    }

    #[test]
    fn json_array_builds_array() {
        let v = json_array(&[SqlValue::Integer(1), SqlValue::Integer(2), SqlValue::Null]).unwrap();
        assert_eq!(v, t("[1,2,null]"));
    }

    #[test]
    fn json_object_rejects_null_key() {
        let err = json_object(&[SqlValue::Null, SqlValue::Integer(1)]).unwrap_err();
        assert!(matches!(err, Error::ConstraintViolation(_)));
    }

    #[test]
    fn json_extract_single_path() {
        let v = json_extract(&[t(r#"{"a":[1,2,3]}"#), t("$.a[1]")]).unwrap();
        assert_eq!(v, SqlValue::Integer(2));
    }

    #[test]
    fn json_extract_returns_null_for_missing() {
        let v = json_extract(&[t(r#"{"a":1}"#), t("$.missing")]).unwrap();
        assert_eq!(v, SqlValue::Null);
    }

    #[test]
    fn json_set_overwrites() {
        let v = json_set(&[t(r#"{"a":1}"#), t("$.a"), SqlValue::Integer(2)]).unwrap();
        assert_eq!(v, t(r#"{"a":2}"#));
    }

    #[test]
    fn json_insert_skips_existing() {
        let v = json_insert(&[t(r#"{"a":1}"#), t("$.a"), SqlValue::Integer(99)]).unwrap();
        assert_eq!(v, t(r#"{"a":1}"#));
    }

    #[test]
    fn json_replace_skips_missing() {
        let v = json_replace(&[t(r#"{"a":1}"#), t("$.b"), SqlValue::Integer(99)]).unwrap();
        assert_eq!(v, t(r#"{"a":1}"#));
    }

    #[test]
    fn json_remove_drops_member() {
        let v = json_remove(&[t(r#"{"a":1,"b":2}"#), t("$.a")]).unwrap();
        assert_eq!(v, t(r#"{"b":2}"#));
    }

    #[test]
    fn json_patch_merges() {
        let v = json_patch(&[t(r#"{"a":1,"b":2}"#), t(r#"{"b":null,"c":3}"#)]).unwrap();
        assert_eq!(v, t(r#"{"a":1,"c":3}"#));
    }

    #[test]
    fn json_type_returns_kind() {
        assert_eq!(
            json_type(&[t(r#"{"a":[1,2]}"#), t("$.a")]).unwrap(),
            t("array")
        );
        assert_eq!(json_type(&[t("null")]).unwrap(), t("null"));
        assert_eq!(json_type(&[t("3.14")]).unwrap(), t("real"));
    }

    #[test]
    fn json_valid_returns_one_or_zero() {
        assert_eq!(json_valid(&[t("[1,2]")]).unwrap(), SqlValue::Integer(1));
        assert_eq!(json_valid(&[t("not json")]).unwrap(), SqlValue::Integer(0));
    }

    #[test]
    fn json_quote_quotes_text() {
        assert_eq!(json_quote(&[t("hi")]).unwrap(), t("\"hi\""));
    }

    #[test]
    fn arrow_json_returns_json_text() {
        let v = arrow_json(&t(r#"{"a":[1,2]}"#), &t("$.a")).unwrap();
        assert_eq!(v, t("[1,2]"));
    }

    #[test]
    fn arrow_sql_unwraps_scalar() {
        let v = arrow_sql(&t(r#"{"a":42}"#), &t("$.a")).unwrap();
        assert_eq!(v, SqlValue::Integer(42));
    }

    #[test]
    fn arrow_path_shorthand() {
        let v = arrow_sql(&t(r#"{"a":1}"#), &t("a")).unwrap();
        assert_eq!(v, SqlValue::Integer(1));
        let v = arrow_sql(&t("[10,20]"), &SqlValue::Integer(1)).unwrap();
        assert_eq!(v, SqlValue::Integer(20));
    }
}
