//! Compound-SELECT set operations: `UNION`, `INTERSECT`, `EXCEPT`.
//!
//! `UNION ALL` keeps its existing fast path in [`SelectSource::CompoundAll`];
//! the three deduplicating operations here materialise both branches into
//! row vectors, then combine according to [`CompoundSetOp`] with hash dedup.
//!
//! Column count compatibility is checked at execution time; type-class
//! compatibility uses SQLite-compatible "any-type-fits-any-cell" semantics
//! since storage classes are dynamic.

use std::collections::HashSet;

use crate::connection::Connection;
use crate::error::{Error, Result};
use crate::statement::{CompoundSetOp, SelectPlan};
use crate::value::SqlValue;

use super::materialize_select_plan_rows;

/// A stable, hash-friendly key derived from a row value.
fn row_key(row: &[SqlValue]) -> String {
    let mut s = String::with_capacity(row.len() * 16);
    for v in row {
        match v {
            SqlValue::Null => s.push_str("N|"),
            SqlValue::Integer(i) => {
                s.push('I');
                s.push_str(&i.to_string());
                s.push('|');
            }
            SqlValue::Real(r) => {
                s.push('R');
                // bit-equality so NaN/-0 dedup like SQLite (close enough).
                s.push_str(&format!("{:?}", r.to_bits()));
                s.push('|');
            }
            SqlValue::Text(t) => {
                s.push('T');
                s.push_str(t);
                s.push('|');
            }
            SqlValue::Blob(b) => {
                s.push('B');
                s.push_str(&format!("{:?}", b.as_ref()));
                s.push('|');
            }
        }
    }
    s
}

fn dedup_rows(rows: Vec<Vec<SqlValue>>) -> Vec<Vec<SqlValue>> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let key = row_key(&row);
        if seen.insert(key) {
            out.push(row);
        }
    }
    out
}

fn check_arity(left: &[Vec<SqlValue>], right: &[Vec<SqlValue>]) -> Result<()> {
    let lw = left.first().map(|r| r.len());
    let rw = right.first().map(|r| r.len());
    match (lw, rw) {
        (Some(a), Some(b)) if a != b => Err(Error::UnsupportedSql(format!(
            "SELECTs to the left and right of a set operator do not have the same number of result columns ({a} vs {b})"
        ))),
        _ => Ok(()),
    }
}

pub(crate) fn collect_compound_set_rows(
    conn: &Connection,
    op: CompoundSetOp,
    branches: &[SelectPlan],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<Vec<SqlValue>>> {
    if branches.is_empty() {
        return Ok(Vec::new());
    }
    // Materialise the first branch and then fold the rest in left-to-right.
    let mut accum = materialize_select_plan_rows(conn, &branches[0], bindings)?;
    for branch in &branches[1..] {
        let next = materialize_select_plan_rows(conn, branch, bindings)?;
        check_arity(&accum, &next)?;
        accum = combine_two(op, accum, next);
    }
    Ok(dedup_rows(accum))
}

fn combine_two(
    op: CompoundSetOp,
    left: Vec<Vec<SqlValue>>,
    right: Vec<Vec<SqlValue>>,
) -> Vec<Vec<SqlValue>> {
    match op {
        CompoundSetOp::UnionDistinct => {
            let mut out = left;
            out.extend(right);
            out
        }
        CompoundSetOp::Intersect => {
            let r_keys: HashSet<String> = right.iter().map(|r| row_key(r)).collect();
            let l_dedup = dedup_rows(left);
            l_dedup
                .into_iter()
                .filter(|row| r_keys.contains(&row_key(row)))
                .collect()
        }
        CompoundSetOp::Except => {
            let r_keys: HashSet<String> = right.iter().map(|r| row_key(r)).collect();
            let l_dedup = dedup_rows(left);
            l_dedup
                .into_iter()
                .filter(|row| !r_keys.contains(&row_key(row)))
                .collect()
        }
    }
}
