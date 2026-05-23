use super::super::agg_eval::{eval_group_scalar_with_ctx, project_group_row};
use super::super::*;
use super::order::{eval_group_key, sort_groups_by_order_by, sort_projected_rows_by_order_by};

pub(crate) fn execute_grouped_select(
    plan: &crate::statement::SelectPlan,
    rows: Vec<SqlRow>,
    bindings: &[Option<SqlValue>],
    limit: usize,
    offset: usize,
    memory: &mut QueryMemoryBroker,
) -> Result<Vec<Vec<SqlValue>>> {
    let mut filtered = Vec::with_capacity(rows.len());
    for row in rows {
        if selection_passes(&plan.selection, &row, bindings)? {
            filtered.push(row);
        }
    }

    if plan.distinct && plan.group_by.is_empty() {
        let mut out = Vec::with_capacity(filtered.len());
        let memory_bytes = filtered.iter().try_fold(0usize, |acc, row| {
            row.values().map(|values| acc + row_width(&values))
        })?;
        memory.request(memory_bytes)?;
        for row in filtered {
            let first_context = Some(row.context());
            if let Some(having) = &plan.having
                && !is_truthy(&eval_group_scalar_with_ctx(
                    having,
                    std::slice::from_ref(&row),
                    first_context.as_ref(),
                    bindings,
                )?)
            {
                continue;
            }
            out.push(project_row(&plan.projection, &row, bindings)?);
        }
        out.sort_by(|left, right| compare_rows(left, right));
        out.dedup_by(|left, right| compare_rows(left, right) == Ordering::Equal);
        if !plan.order_by.is_empty() {
            sort_projected_rows_by_order_by(&mut out, &plan.projection, &plan.order_by, bindings)?;
        }
        return Ok(out.into_iter().skip(offset).take(limit).collect());
    }

    let groups = if plan.group_by.is_empty() {
        vec![filtered]
    } else {
        use std::collections::HashMap;
        let mut index_by_key: HashMap<Vec<u8>, usize> = HashMap::with_capacity(filtered.len());
        let mut groups: Vec<Vec<SqlRow>> = Vec::with_capacity(filtered.len());
        for row in filtered {
            let key = eval_group_key(&plan.group_by, &row, bindings)?;
            let key_bytes = vec::hash_agg::encode_group_key_bytes(&key)?;
            match index_by_key.get(&key_bytes) {
                Some(&idx) => groups[idx].push(row),
                None => {
                    index_by_key.insert(key_bytes, groups.len());
                    groups.push(vec![row]);
                }
            }
        }
        groups
    };

    let memory_bytes = groups.iter().try_fold(0usize, |acc, group| {
        let group_bytes = group.iter().try_fold(0usize, |group_acc, row| {
            row.values().map(|values| group_acc + row_width(&values))
        })?;
        Ok::<usize, Error>(acc + group_bytes)
    })?;
    memory.request(memory_bytes)?;

    let mut out = Vec::with_capacity(groups.len());
    if plan.distinct {
        for group in groups {
            let first_context = group.first().map(|row| row.context());
            if group.is_empty()
                && !plan
                    .projection
                    .iter()
                    .any(super::select::select_item_contains_aggregate)
            {
                continue;
            }
            if let Some(having) = &plan.having
                && !is_truthy(&eval_group_scalar_with_ctx(
                    having,
                    &group,
                    first_context.as_ref(),
                    bindings,
                )?)
            {
                continue;
            }
            out.push(project_group_row(&plan.projection, &group, bindings)?);
        }
        out.sort_by(|left, right| compare_rows(left, right));
        out.dedup_by(|left, right| compare_rows(left, right) == Ordering::Equal);
        if !plan.order_by.is_empty() {
            sort_projected_rows_by_order_by(&mut out, &plan.projection, &plan.order_by, bindings)?;
        }
        return Ok(out.into_iter().skip(offset).take(limit).collect());
    }

    let mut surviving_groups: Vec<&[SqlRow]> = Vec::with_capacity(groups.len());
    for group in &groups {
        let first_context = group.first().map(|row| row.context());
        if group.is_empty()
            && !plan
                .projection
                .iter()
                .any(super::select::select_item_contains_aggregate)
        {
            continue;
        }
        if let Some(having) = &plan.having
            && !is_truthy(&eval_group_scalar_with_ctx(
                having,
                group,
                first_context.as_ref(),
                bindings,
            )?)
        {
            continue;
        }
        out.push(project_group_row(&plan.projection, group, bindings)?);
        surviving_groups.push(group.as_slice());
    }

    if !plan.order_by.is_empty() {
        sort_groups_by_order_by(
            &mut out,
            &surviving_groups,
            &plan.projection,
            &plan.order_by,
            bindings,
        )?;
    }

    Ok(out.into_iter().skip(offset).take(limit).collect())
}
