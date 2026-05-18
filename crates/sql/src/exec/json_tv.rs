//! Table-valued JSON walkers: `json_each` and `json_tree`. Both expose
//! SQLite's documented column shape `(key, value, type, atom, id, parent,
//! fullkey, path)`. `json_each` emits one row per immediate child of the
//! value at the (optional) path; `json_tree` recursively emits every node
//! in the subtree, depth-first, with `parent` linking each row back to
//! the containing row's `id`. Argument lowering (`TvArg`) and result
//! wrapping (`TvResult` -> `SelectSource::Cte`) reuse the C2-era
//! machinery; this module owns only the JSON walk.

use std::sync::Arc;

use redlinedb_kernel::catalog::SchemaSnapshot;
use serde_json::Value;

use crate::connection::Connection;
use crate::error::{Error, Result};
use crate::json::path::{JsonPath, PathSegment, resolve};
use crate::json::scalar::{json_to_sql, render_json};
use crate::value::SqlValue;

use super::table_valued::{TvArg, TvFunc, TvResult};

pub(super) fn registry() -> &'static [&'static dyn TvFunc] {
    &[&JsonEach, &JsonTree]
}

fn columns() -> Vec<String> {
    [
        "key", "value", "type", "atom", "id", "parent", "fullkey", "path",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// SQLite's `json_type` token for a `serde_json::Value`.
fn type_name(v: &Value) -> &'static str {
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

/// `value` column: scalars unwrap to SQL, objects/arrays stay as JSON text.
fn render_value(v: &Value) -> SqlValue {
    match v {
        Value::Array(_) | Value::Object(_) => render_json(v),
        _ => json_to_sql(v),
    }
}

/// `atom` column: NULL for containers; unwrapped scalar otherwise.
fn render_atom(v: &Value) -> SqlValue {
    match v {
        Value::Array(_) | Value::Object(_) => SqlValue::Null,
        _ => json_to_sql(v),
    }
}

/// Quote an object key for `fullkey`/`path`. SQLite uses bare idents when
/// the key matches `[A-Za-z_][A-Za-z0-9_]*`, otherwise emits a quoted form.
fn segment_key(key: &str) -> String {
    let bytes = key.as_bytes();
    let bare = !bytes.is_empty()
        && (bytes[0].is_ascii_alphabetic() || bytes[0] == b'_')
        && bytes[1..]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_');
    if bare {
        format!(".{key}")
    } else {
        format!(".\"{}\"", key.replace('"', "\\\""))
    }
}

/// Subdocument the walk should start at, the fullkey-prefix SQLite uses
/// there, and the `key` for the root row (NULL when path is `$`, else
/// the last path segment).
struct ResolvedArgs<'a> {
    target: &'a Value,
    fullkey: String,
    root_key: SqlValue,
}

fn parse_args<'a>(
    name: &'static str,
    doc: &'a Value,
    args: &[TvArg],
) -> Result<Option<ResolvedArgs<'a>>> {
    match args.len() {
        1 => Ok(Some(ResolvedArgs {
            target: doc,
            fullkey: "$".into(),
            root_key: SqlValue::Null,
        })),
        2 => {
            let path_text = match &args[1] {
                TvArg::Text(s) => s.as_ref(),
                TvArg::Null => return Ok(None),
                _ => {
                    return Err(Error::UnsupportedSql(format!(
                        "{name}: path argument must be TEXT"
                    )));
                }
            };
            let path = JsonPath::parse(path_text)
                .map_err(|e| Error::Parse(format!("JSON path error: {e}")))?;
            let referent = if path.is_root() {
                Some(doc)
            } else {
                resolve(doc, &path)
            };
            let Some(target) = referent else {
                return Ok(None);
            };
            let root_key = match path.segments.last() {
                None => SqlValue::Null,
                Some(PathSegment::Key(k)) => SqlValue::Text(Arc::from(k.as_str())),
                Some(PathSegment::Index(i)) => SqlValue::Integer(*i as i64),
                Some(PathSegment::FromEnd(_) | PathSegment::Append) => SqlValue::Null,
            };
            Ok(Some(ResolvedArgs {
                target,
                fullkey: path_text.to_owned(),
                root_key,
            }))
        }
        n => Err(Error::UnsupportedSql(format!(
            "{name} expects 1 or 2 arguments (got {n})"
        ))),
    }
}

fn parse_doc_arg(name: &'static str, args: &[TvArg]) -> Result<Option<Value>> {
    if args.is_empty() {
        return Err(Error::UnsupportedSql(format!(
            "{name} requires at least 1 argument"
        )));
    }
    match &args[0] {
        TvArg::Null => Ok(None),
        TvArg::Text(s) => serde_json::from_str(s.as_ref())
            .map(Some)
            .map_err(|e| Error::Parse(format!("malformed JSON: {e}"))),
        TvArg::Integer(n) => Ok(Some(Value::Number((*n).into()))),
    }
}

/// One materialised row in the documented column order.
fn make_row(
    key: SqlValue,
    value: &Value,
    id: i64,
    parent: Option<i64>,
    fullkey: &str,
    path: &str,
) -> Vec<SqlValue> {
    vec![
        key,
        render_value(value),
        SqlValue::Text(Arc::from(type_name(value))),
        render_atom(value),
        SqlValue::Integer(id),
        parent.map(SqlValue::Integer).unwrap_or(SqlValue::Null),
        SqlValue::Text(Arc::from(fullkey)),
        SqlValue::Text(Arc::from(path)),
    ]
}

/// Common driver shared by both `eval` implementations: parse args,
/// dispatch the body, return the materialised `TvResult`.
fn run_walk<F>(name: &'static str, args: &[TvArg], body: F) -> Result<TvResult>
where
    F: FnOnce(&Value, &str, SqlValue, &mut Vec<Vec<SqlValue>>),
{
    let cols = columns();
    let Some(doc) = parse_doc_arg(name, args)? else {
        return Ok(TvResult {
            columns: cols,
            rows: vec![],
        });
    };
    let Some(resolved) = parse_args(name, &doc, args)? else {
        return Ok(TvResult {
            columns: cols,
            rows: vec![],
        });
    };
    let mut rows = Vec::new();
    body(
        resolved.target,
        &resolved.fullkey,
        resolved.root_key,
        &mut rows,
    );
    Ok(TvResult {
        columns: cols,
        rows,
    })
}

struct JsonEach;
impl TvFunc for JsonEach {
    fn name(&self) -> &'static str {
        "json_each"
    }
    fn eval(
        &self,
        _conn: &Connection,
        _schema: &SchemaSnapshot,
        args: &[TvArg],
    ) -> Result<TvResult> {
        run_walk("json_each", args, |target, prefix, _root_key, rows| {
            let mut id: i64 = 0;
            emit_each(target, prefix, &mut id, rows);
        })
    }
}

fn emit_each(value: &Value, prefix: &str, id: &mut i64, rows: &mut Vec<Vec<SqlValue>>) {
    match value {
        Value::Array(arr) => {
            for (idx, child) in arr.iter().enumerate() {
                let fk = format!("{prefix}[{idx}]");
                rows.push(make_row(
                    SqlValue::Integer(idx as i64),
                    child,
                    *id,
                    None,
                    &fk,
                    prefix,
                ));
                *id += 1;
            }
        }
        Value::Object(map) => {
            for (k, child) in map {
                let fk = format!("{prefix}{}", segment_key(k));
                rows.push(make_row(
                    SqlValue::Text(Arc::from(k.as_str())),
                    child,
                    *id,
                    None,
                    &fk,
                    prefix,
                ));
                *id += 1;
            }
        }
        atom => {
            rows.push(make_row(SqlValue::Null, atom, *id, None, prefix, prefix));
            *id += 1;
        }
    }
}

struct JsonTree;
impl TvFunc for JsonTree {
    fn name(&self) -> &'static str {
        "json_tree"
    }
    fn eval(
        &self,
        _conn: &Connection,
        _schema: &SchemaSnapshot,
        args: &[TvArg],
    ) -> Result<TvResult> {
        run_walk("json_tree", args, |target, prefix, root_key, rows| {
            let mut id: i64 = 0;
            // Root row: key = last-segment-of-path (NULL at `$`), parent NULL.
            let root_id = id;
            rows.push(make_row(root_key, target, id, None, prefix, prefix));
            id += 1;
            emit_tree(target, prefix, root_id, &mut id, rows);
        })
    }
}

/// Recursive tree walk; emits one row per child node, depth-first.
fn emit_tree(
    value: &Value,
    parent_fullkey: &str,
    parent_id: i64,
    id: &mut i64,
    rows: &mut Vec<Vec<SqlValue>>,
) {
    match value {
        Value::Array(arr) => {
            for (idx, child) in arr.iter().enumerate() {
                let fk = format!("{parent_fullkey}[{idx}]");
                let my_id = *id;
                rows.push(make_row(
                    SqlValue::Integer(idx as i64),
                    child,
                    my_id,
                    Some(parent_id),
                    &fk,
                    parent_fullkey,
                ));
                *id += 1;
                emit_tree(child, &fk, my_id, id, rows);
            }
        }
        Value::Object(map) => {
            for (k, child) in map {
                let fk = format!("{parent_fullkey}{}", segment_key(k));
                let my_id = *id;
                rows.push(make_row(
                    SqlValue::Text(Arc::from(k.as_str())),
                    child,
                    my_id,
                    Some(parent_id),
                    &fk,
                    parent_fullkey,
                ));
                *id += 1;
                emit_tree(child, &fk, my_id, id, rows);
            }
        }
        _ => {}
    }
}
