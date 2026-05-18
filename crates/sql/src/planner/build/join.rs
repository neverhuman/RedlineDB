use super::*;

#[derive(Clone)]
struct JoinCandidate {
    plan: PhysicalPlan,
    rows: f64,
    cost: Cost,
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
        return match scans.into_iter().next().map(|(_, plan, _)| plan) {
            Some(plan) => plan,
            None => PhysicalPlan::leaf(PhysicalKind::Constant, Some("empty".to_owned())),
        };
    }

    if scans.len() <= optimizer.max_exact_join_tables
        && let Some(best) = plan_join_exact(&scans, selection, bindings, optimizer)
    {
        return best;
    }

    plan_join_greedy(&scans, selection, bindings, optimizer)
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
                join.access_predicates =
                    match selection.as_ref().map(|expr| vec![expr_to_string(expr)]) {
                        Some(v) => v,
                        None => Vec::new(),
                    };
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
        join.access_predicates = match selection.as_ref().map(|expr| vec![expr_to_string(expr)]) {
            Some(v) => v,
            None => Vec::new(),
        };
        join.relation = Some(table.table.name.to_string());
        current = join;
        current_rows = join_cost.rows;
    }
    current
}
