//! Partitioning, ordering, and peer-group assignment for window evaluation.

use std::cmp::Ordering;
use std::collections::HashMap;

use sqlparser::ast::{Expr, OrderByExpr};

use crate::error::Result;
use crate::value::{SqlValue, compare_values};

use super::super::SqlRow;
use super::super::eval_scalar;

pub(super) fn partition_rows(
    rows: &[SqlRow],
    partition_by: &[Expr],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<Vec<usize>>> {
    if partition_by.is_empty() {
        return Ok(vec![(0..rows.len()).collect()]);
    }
    let mut keys: Vec<Vec<SqlValue>> = Vec::with_capacity(rows.len());
    for row in rows {
        let mut key = Vec::with_capacity(partition_by.len());
        for expr in partition_by {
            key.push(eval_scalar(expr, &row.context(), bindings)?);
        }
        keys.push(key);
    }
    let mut group_by_key: HashMap<Vec<u8>, usize> = HashMap::with_capacity(keys.len());
    let mut groups: Vec<Vec<usize>> = Vec::new();
    // Phase 4.5: reuse a single scratch buffer across rows. The
    // HashMap key is borrowed for lookup (`get(scratch.as_slice())`)
    // so we only pay the clone allocation on cache miss — the hot
    // hit path is zero-allocation.
    let mut scratch: Vec<u8> = Vec::with_capacity(64);
    for (i, key) in keys.iter().enumerate() {
        crate::exec::vec::hash_agg::encode_group_key_bytes_into(key, &mut scratch)?;
        match group_by_key.get(scratch.as_slice()) {
            Some(&group_idx) => groups[group_idx].push(i),
            None => {
                group_by_key.insert(scratch.clone(), groups.len());
                groups.push(vec![i]);
            }
        }
    }
    Ok(groups)
}

pub(super) fn order_partition(
    partition: &[usize],
    rows: &[SqlRow],
    order_by: &[OrderByExpr],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<(usize, Vec<SqlValue>)>> {
    let mut items: Vec<(usize, Vec<SqlValue>)> = Vec::with_capacity(partition.len());
    for &idx in partition {
        let mut key = Vec::with_capacity(order_by.len());
        for ord in order_by {
            key.push(eval_scalar(&ord.expr, &rows[idx].context(), bindings)?);
        }
        items.push((idx, key));
    }
    if order_by.is_empty() {
        return Ok(items);
    }
    items.sort_by(|a, b| {
        for (i, ord) in order_by.iter().enumerate() {
            let ord_dir = ord.options.asc.unwrap_or(true);
            // SQLite default: NULLs FIRST for ASC, NULLs LAST for DESC.
            let nulls_first = ord.options.nulls_first.unwrap_or(ord_dir);
            let cmp = compare_with_nulls(&a.1[i], &b.1[i], nulls_first);
            let cmp = if ord_dir { cmp } else { cmp.reverse() };
            if cmp != Ordering::Equal {
                return cmp;
            }
        }
        a.0.cmp(&b.0)
    });
    Ok(items)
}

pub(super) fn assign_peer_ids(
    sorted: &[(usize, Vec<SqlValue>)],
    order_by: &[OrderByExpr],
) -> Vec<usize> {
    let mut ids = Vec::with_capacity(sorted.len());
    let mut current_id = 0usize;
    for i in 0..sorted.len() {
        if i == 0 {
            ids.push(current_id);
            continue;
        }
        let prev = &sorted[i - 1].1;
        let curr = &sorted[i].1;
        let mut equal = true;
        for (j, _) in order_by.iter().enumerate() {
            if compare_values(&prev[j], &curr[j]) != Ordering::Equal {
                equal = false;
                break;
            }
        }
        if !equal {
            current_id += 1;
        }
        ids.push(current_id);
    }
    ids
}

pub(super) fn peer_ranges(peer_ids: &[usize]) -> Vec<(usize, usize)> {
    if peer_ids.is_empty() {
        return Vec::new();
    }
    let peer_count = peer_ids[peer_ids.len() - 1].saturating_add(1);
    let mut ranges = vec![(0usize, 0usize); peer_count];
    for (index, &peer_id) in peer_ids.iter().enumerate() {
        if index == 0 || peer_ids[index - 1] != peer_id {
            ranges[peer_id].0 = index;
        }
        ranges[peer_id].1 = index;
    }
    ranges
}

fn compare_with_nulls(a: &SqlValue, b: &SqlValue, nulls_first: bool) -> Ordering {
    match (a, b) {
        (SqlValue::Null, SqlValue::Null) => Ordering::Equal,
        (SqlValue::Null, _) => {
            if nulls_first {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        }
        (_, SqlValue::Null) => {
            if nulls_first {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        }
        _ => compare_values(a, b),
    }
}
