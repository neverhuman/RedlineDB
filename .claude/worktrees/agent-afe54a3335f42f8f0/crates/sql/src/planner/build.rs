use super::*;

pub(crate) fn build_select_plan(
    conn: &Connection,
    plan: &SelectPlan,
    bindings: &[Option<SqlValue>],
) -> PhysicalPlan {
    let optimizer = conn.optimizer_config().clone();
    let mut base = match &plan.source {
        SelectSource::Table(table) => build_table_scan_plan(
            conn,
            table,
            &plan.projection,
            &plan.selection,
            &plan.order_by,
            bindings,
            &optimizer,
        ),
        SelectSource::Tables(tables) => {
            build_join_plan(conn, tables, &plan.selection, bindings, &optimizer)
        }
        SelectSource::Joined(join) => {
            let mut tables = vec![join.base.clone()];
            tables.extend(join.joins.iter().map(|step| step.right.clone()));
            build_join_plan(conn, &tables, &None, bindings, &optimizer)
        }
        SelectSource::SqliteSchema => {
            let mut node =
                PhysicalPlan::leaf(PhysicalKind::TableScan, Some("sqlite_schema".to_owned()));
            node.estimated_rows = conn.engine().sqlite_schema().len() as f64;
            node.cost = estimate_scan_cost(node.estimated_rows, estimate_width_for_schema());
            node.projected_columns = vec![
                "type".into(),
                "name".into(),
                "tbl_name".into(),
                "rootpage".into(),
                "sql".into(),
            ];
            node
        }
        SelectSource::StaticRows { rows } => {
            let mut node =
                PhysicalPlan::leaf(PhysicalKind::Constant, Some("static rows".to_owned()));
            node.estimated_rows = rows.len() as f64;
            node.cost = Cost::zero();
            node
        }
        SelectSource::CompoundAll(branches) => {
            let mut node = PhysicalPlan::leaf(PhysicalKind::Constant, Some("UNION ALL".to_owned()));
            node.children = branches
                .iter()
                .map(|branch| build_select_plan(conn, branch, bindings))
                .collect();
            node.estimated_rows = node.children.iter().map(|child| child.estimated_rows).sum();
            node.cost = node.children.iter().fold(Cost::zero(), |mut acc, child| {
                acc.startup += child.cost.startup;
                acc.total += child.cost.total;
                acc.rows += child.cost.rows;
                acc.width = acc.width.max(child.cost.width);
                acc.memory_bytes = acc.memory_bytes.max(child.cost.memory_bytes);
                acc.spill_bytes = acc.spill_bytes.max(child.cost.spill_bytes);
                acc
            });
            node
        }
        SelectSource::Empty => {
            let mut node =
                PhysicalPlan::leaf(PhysicalKind::Constant, Some("constant row".to_owned()));
            node.estimated_rows = 1.0;
            node.cost = Cost::zero();
            node
        }
    };

    if requires_aggregate(plan) {
        base = wrap_aggregate(base, plan);
    }

    if !plan.order_by.is_empty() && !output_order_satisfies(&base.output_order, &plan.order_by) {
        base = wrap_ordering(base, plan, bindings, &optimizer);
    }

    if has_projection_exprs(&plan.projection) {
        base = wrap_project(base, plan);
    }

    if plan.limit.is_some() || plan.offset.is_some() {
        base = wrap_limit(base, plan);
    }

    base
}

pub(crate) fn build_table_scan_plan(
    conn: &Connection,
    table: &Arc<TableDef>,
    projection: &[SelectItem],
    selection: &Option<Expr>,
    order_by: &[OrderByExpr],
    bindings: &[Option<SqlValue>],
    optimizer: &OptimizerConfig,
) -> PhysicalPlan {
    let stats = conn.stats_snapshot();
    let table_stats = stats.tables.get(&table.table_id);
    let row_estimate = estimate_table_rows(table_stats);
    let width = estimate_table_width(table_stats, table);
    let rowid = selection_rowid_eq(table, selection, bindings)
        .ok()
        .flatten();
    let access = choose_access_path(
        conn,
        table,
        projection,
        selection,
        order_by,
        rowid,
        bindings,
        table_stats,
        optimizer,
    );
    let is_covering = matches!(&access, AccessPath::CoveringIndexScan { .. });
    let ordering_satisfied = satisfies_ordering(table, &access, order_by);

    let mut node = match access {
        AccessPath::TableScan => {
            let mut node =
                PhysicalPlan::leaf(PhysicalKind::TableScan, Some(table.name.to_string()));
            node.estimated_rows = row_estimate;
            node.cost = estimate_scan_cost(row_estimate, width);
            node
        }
        AccessPath::RowIdGet { rowid } => {
            let mut node = PhysicalPlan::leaf(PhysicalKind::RowIdGet, Some(table.name.to_string()));
            node.estimated_rows = 1.0;
            node.cost = Cost {
                startup: INDEX_PROBE_STARTUP,
                total: INDEX_PROBE_STARTUP + CPU_OPERATOR_COST + CPU_TUPLE_COST,
                rows: 1.0,
                width,
                memory_bytes: 0,
                spill_bytes: 0,
            };
            node.access_predicates = vec![format!("rowid = {}", rowid.0)];
            node
        }
        AccessPath::IndexPointLookup { index, predicates } => {
            let mut node =
                PhysicalPlan::leaf(PhysicalKind::IndexScan, Some(table.name.to_string()));
            node.index = Some(index.name.to_string());
            node.index_probe_kind = Some("PointLookup");
            node.estimated_rows = estimate_eq_rows(table_stats, &index, &predicates);
            node.cost = estimate_index_cost(node.estimated_rows, width, true);
            node.access_predicates = predicates;
            node
        }
        AccessPath::IndexRangeScan { index, predicates } => {
            let mut node =
                PhysicalPlan::leaf(PhysicalKind::IndexScan, Some(table.name.to_string()));
            node.index = Some(index.name.to_string());
            node.index_probe_kind = Some("RangeScan");
            node.estimated_rows = estimate_range_rows(table_stats, &index, &predicates);
            node.cost = estimate_index_cost(node.estimated_rows, width, false);
            node.access_predicates = predicates;
            node
        }
        AccessPath::CoveringIndexScan {
            index,
            predicates,
            projected_columns,
        } => {
            let mut node =
                PhysicalPlan::leaf(PhysicalKind::IndexScan, Some(table.name.to_string()));
            node.index = Some(index.name.to_string());
            node.estimated_rows = estimate_index_rows(table_stats, &index);
            node.cost = estimate_index_cost(node.estimated_rows, width, true);
            node.access_predicates = predicates;
            node.projected_columns = projected_columns;
            node
        }
        AccessPath::MultiIndexOr { inputs } | AccessPath::MultiIndexAnd { inputs } => {
            let mut node =
                PhysicalPlan::leaf(PhysicalKind::MultiIndexScan, Some(table.name.to_string()));
            node.children = inputs
                .into_iter()
                .map(|input| access_plan_to_node(table, input, width))
                .collect();
            node.estimated_rows = node.children.iter().map(|child| child.estimated_rows).sum();
            node.cost = Cost {
                startup: INDEX_PROBE_STARTUP,
                total: node
                    .children
                    .iter()
                    .map(|child| child.cost.total)
                    .sum::<f64>()
                    + CPU_OPERATOR_COST,
                rows: node.estimated_rows,
                width,
                memory_bytes: 0,
                spill_bytes: 0,
            };
            node
        }
    };

    if selection.is_none() {
        node.residual_predicates.clear();
    } else if !node.access_predicates.is_empty() {
        node.residual_predicates
            .push(expr_to_string(selection.as_ref().unwrap()));
    }

    if node.projected_columns.is_empty() {
        node.projected_columns = projected_columns_from_select(table, projection);
    }
    if is_covering {
        node.kind = PhysicalKind::IndexScan;
    }

    if ordering_satisfied {
        node.output_order = order_by
            .iter()
            .map(|item| expr_to_string(&item.expr))
            .collect();
    }

    node
}

pub(crate) fn access_plan_to_node(
    table: &Arc<TableDef>,
    access: AccessPath,
    width: f64,
) -> PhysicalPlan {
    match access {
        AccessPath::TableScan => {
            let mut node =
                PhysicalPlan::leaf(PhysicalKind::TableScan, Some(table.name.to_string()));
            node.cost = estimate_scan_cost(estimate_table_rows(None), width);
            node
        }
        AccessPath::RowIdGet { rowid } => {
            let mut node = PhysicalPlan::leaf(PhysicalKind::RowIdGet, Some(table.name.to_string()));
            node.access_predicates = vec![format!("rowid = {}", rowid.0)];
            node
        }
        AccessPath::IndexPointLookup { index, predicates }
        | AccessPath::IndexRangeScan { index, predicates } => {
            let mut node =
                PhysicalPlan::leaf(PhysicalKind::IndexScan, Some(table.name.to_string()));
            node.index = Some(index.name.to_string());
            node.access_predicates = predicates;
            node
        }
        AccessPath::CoveringIndexScan {
            index,
            predicates,
            projected_columns,
        } => {
            let mut node =
                PhysicalPlan::leaf(PhysicalKind::IndexScan, Some(table.name.to_string()));
            node.index = Some(index.name.to_string());
            node.access_predicates = predicates;
            node.projected_columns = projected_columns;
            node
        }
        AccessPath::MultiIndexOr { inputs } | AccessPath::MultiIndexAnd { inputs } => {
            let mut node =
                PhysicalPlan::leaf(PhysicalKind::MultiIndexScan, Some(table.name.to_string()));
            node.children = inputs
                .into_iter()
                .map(|input| access_plan_to_node(table, input, width))
                .collect();
            node
        }
    }
}

pub(crate) fn build_join_plan(
    conn: &Connection,
    tables: &[BoundTable],
    selection: &Option<Expr>,
    bindings: &[Option<SqlValue>],
    optimizer: &OptimizerConfig,
) -> PhysicalPlan {
    let stats = conn.stats_snapshot();
    let scans: Vec<(BoundTable, PhysicalPlan, f64)> = tables
        .iter()
        .map(|table| {
            let scan =
                build_table_scan_plan(conn, &table.table, &[], &None, &[], bindings, optimizer);
            let rows = estimate_table_rows(stats.tables.get(&table.table.table_id));
            (table.clone(), scan, rows)
        })
        .collect();

    if scans.len() <= 1 {
        return scans
            .into_iter()
            .next()
            .map(|(_, plan, _)| plan)
            .unwrap_or_else(|| {
                PhysicalPlan::leaf(PhysicalKind::Constant, Some("empty".to_owned()))
            });
    }

    if scans.len() <= optimizer.max_exact_join_tables
        && let Some(best) = plan_join_exact(&scans, selection, bindings, optimizer)
    {
        return best;
    }

    plan_join_greedy(&scans, selection, bindings, optimizer)
}

#[derive(Clone)]
struct JoinCandidate {
    plan: PhysicalPlan,
    rows: f64,
    cost: Cost,
}

pub(crate) fn plan_join_exact(
    scans: &[(BoundTable, PhysicalPlan, f64)],
    selection: &Option<Expr>,
    bindings: &[Option<SqlValue>],
    optimizer: &OptimizerConfig,
) -> Option<PhysicalPlan> {
    let n = scans.len();
    let max_alternatives = optimizer.max_join_alternatives.max(1);
    let mut dp: Vec<Vec<JoinCandidate>> = vec![Vec::new(); 1 << n];

    for (idx, (_, plan, rows)) in scans.iter().enumerate() {
        dp[1 << idx].push(JoinCandidate {
            plan: plan.clone(),
            rows: *rows,
            cost: plan.cost,
        });
    }

    for mask in 1usize..(1usize << n) {
        if mask.count_ones() <= 1 {
            continue;
        }
        let mut candidates = Vec::new();
        let mut remaining = mask;
        while remaining != 0 {
            let bit = remaining & (!remaining + 1);
            remaining ^= bit;
            let idx = bit.trailing_zeros() as usize;
            let prev_mask = mask ^ bit;
            for prev in &dp[prev_mask] {
                let table = &scans[idx].0.table;
                let next_scan = scans[idx].1.clone();
                let right_rows = scans[idx].2;
                let join_kind = choose_join_kind(prev.rows, right_rows, table, selection, bindings);
                let join_cost = join_cost(join_kind, prev.rows, right_rows);
                let mut join = PhysicalPlan::new(match join_kind {
                    JoinKind::Hash => PhysicalKind::HashJoin,
                    JoinKind::IndexNestedLoop => PhysicalKind::IndexNestedLoopJoin,
                    JoinKind::NestedLoop | JoinKind::Cross => PhysicalKind::NestedLoopJoin,
                });
                let join_rows = join_cost.rows;
                let join_plan_cost = Cost {
                    startup: prev.cost.startup + join_cost.startup,
                    total: prev.cost.total + join_cost.total,
                    rows: join_rows,
                    width: prev.cost.width + scans[idx].1.cost.width,
                    memory_bytes: prev
                        .cost
                        .memory_bytes
                        .saturating_add(join_cost.memory_bytes),
                    spill_bytes: prev.cost.spill_bytes.saturating_add(join_cost.spill_bytes),
                };
                join.children = vec![prev.plan.clone(), next_scan];
                join.estimated_rows = join_rows;
                join.access_predicates = selection
                    .as_ref()
                    .map(|expr| vec![expr_to_string(expr)])
                    .unwrap_or_default();
                join.relation = Some(table.name.to_string());
                join.cost = join_plan_cost;
                candidates.push(JoinCandidate {
                    plan: join,
                    rows: join_rows,
                    cost: join_plan_cost,
                });
            }
        }
        candidates.sort_by(|left, right| {
            left.cost
                .total
                .partial_cmp(&right.cost.total)
                .unwrap_or(Ordering::Equal)
                .then_with(|| {
                    left.rows
                        .partial_cmp(&right.rows)
                        .unwrap_or(Ordering::Equal)
                })
                .then_with(|| {
                    left.plan
                        .relation
                        .as_deref()
                        .cmp(&right.plan.relation.as_deref())
                })
        });
        candidates.truncate(max_alternatives);
        if !candidates.is_empty() {
            dp[mask] = candidates;
        }
    }

    dp[(1usize << n) - 1]
        .first()
        .map(|candidate| candidate.plan.clone())
}

pub(crate) fn plan_join_greedy(
    scans: &[(BoundTable, PhysicalPlan, f64)],
    selection: &Option<Expr>,
    bindings: &[Option<SqlValue>],
    _optimizer: &OptimizerConfig,
) -> PhysicalPlan {
    let mut ordered = scans.to_vec();
    ordered.sort_by(|left, right| left.2.partial_cmp(&right.2).unwrap_or(Ordering::Equal));
    let mut iter = ordered.into_iter();
    let (_, mut current, mut current_rows) = iter.next().unwrap();
    for (table, next_scan, rows) in iter {
        let join_kind = choose_join_kind(current_rows, rows, &table.table, selection, bindings);
        let join_cost = join_cost(join_kind, current_rows, rows);
        let current_width = current.cost.width;
        let next_width = next_scan.cost.width;
        let current_plan = current;
        let mut join = PhysicalPlan::new(match join_kind {
            JoinKind::Hash => PhysicalKind::HashJoin,
            JoinKind::IndexNestedLoop => PhysicalKind::IndexNestedLoopJoin,
            JoinKind::NestedLoop | JoinKind::Cross => PhysicalKind::NestedLoopJoin,
        });
        join.children = vec![current_plan, next_scan];
        join.estimated_rows = join_cost.rows;
        join.cost = Cost {
            startup: join_cost.startup,
            total: join_cost.total + join.children[0].cost.total,
            rows: join_cost.rows,
            width: current_width + next_width,
            memory_bytes: join_cost.memory_bytes,
            spill_bytes: join_cost.spill_bytes,
        };
        join.access_predicates = selection
            .as_ref()
            .map(|expr| vec![expr_to_string(expr)])
            .unwrap_or_default();
        join.relation = Some(table.table.name.to_string());
        current = join;
        current_rows = join_cost.rows;
    }
    current
}
