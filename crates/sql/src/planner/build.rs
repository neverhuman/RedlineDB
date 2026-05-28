use super::*;

#[path = "build/join.rs"]
mod join;

pub(crate) use join::*;

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
            plan.table_hint.as_ref(),
        ),
        SelectSource::Tables(tables) => {
            build_join_plan(conn, tables, &plan.selection, bindings, &optimizer)
        }
        SelectSource::Joined(join) => {
            let mut tables = vec![join.base.clone()];
            tables.extend(join.joins.iter().map(|step| step.right.clone()));
            build_join_plan(conn, &tables, &None, bindings, &optimizer)
        }
        SelectSource::SqliteSchema | SelectSource::SqliteTempSchema => {
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
        SelectSource::Cte { name, rows, .. } => {
            let mut node = PhysicalPlan::leaf(PhysicalKind::Constant, Some(format!("cte {name}")));
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
        SelectSource::CompoundSet { branches, .. } => {
            // Reuse the UNION ALL shape for cost estimation until the
            // dedicated set-operation executor lands.
            let mut node = PhysicalPlan::leaf(
                PhysicalKind::Constant,
                Some("UNION/INTERSECT/EXCEPT".to_owned()),
            );
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
        base = wrap_limit_with_conn(Some(conn), base, plan);
    }

    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::{Database, DbOptions};
    use crate::planner::access_path::{planner_use_access_path, set_planner_use_access_path};
    use crate::statement::PreparedKind;

    fn select_plan(conn: &Arc<Connection>, sql: &str) -> SelectPlan {
        let stmt = conn.prepare(sql).expect("prepare");
        match &stmt.template().kind {
            PreparedKind::Select(plan) => plan.clone(),
            other => panic!("expected SELECT plan, got {other:?}"),
        }
    }

    fn index_child_limit(plan: &PhysicalPlan) -> Option<usize> {
        let [child] = plan.children.as_slice() else {
            panic!("expected LIMIT over one child, got {plan:?}");
        };
        let index = match child.kind {
            PhysicalKind::IndexScan => child,
            PhysicalKind::Project => {
                let [project_child] = child.children.as_slice() else {
                    panic!("expected PROJECT over one child, got {child:?}");
                };
                assert!(matches!(project_child.kind, PhysicalKind::IndexScan));
                project_child
            }
            _ => panic!("expected LIMIT over IndexScan or Project, got {child:?}"),
        };
        index.ordered_index_scan_limit
    }

    fn with_access_path_gate<T>(value: bool, f: impl FnOnce() -> T) -> T {
        let prev = planner_use_access_path();
        set_planner_use_access_path(value);
        let out = f();
        set_planner_use_access_path(prev);
        out
    }

    #[test]
    fn access_path_limit_pushdown_refuses_residual_predicate() {
        let conn = Database::create_in_memory(DbOptions::default())
            .expect("db")
            .connect();
        conn.execute("CREATE TABLE t(tenant INTEGER, k INTEGER, keep INTEGER)")
            .expect("create");
        conn.execute("CREATE INDEX t_tk ON t(tenant, k)")
            .expect("index");
        let select = select_plan(
            &conn,
            "SELECT k FROM t WHERE tenant = 1 AND keep = 1 ORDER BY k LIMIT 3",
        );
        with_access_path_gate(true, || {
            let physical = build_select_plan(&conn, &select, &[]);
            assert_eq!(index_child_limit(&physical), None);
        });
    }

    #[test]
    fn access_path_limit_pushdown_keeps_residual_free_ordered_scan() {
        let conn = Database::create_in_memory(DbOptions::default())
            .expect("db")
            .connect();
        conn.execute("CREATE TABLE t(tenant INTEGER, k INTEGER, v INTEGER)")
            .expect("create");
        conn.execute("CREATE INDEX t_tk ON t(tenant, k)")
            .expect("index");
        let select = select_plan(&conn, "SELECT k FROM t WHERE tenant = 1 ORDER BY k LIMIT 3");
        with_access_path_gate(true, || {
            let physical = build_select_plan(&conn, &select, &[]);
            assert_eq!(index_child_limit(&physical), Some(3));
        });
    }
}

pub(crate) fn build_table_scan_plan(
    conn: &Connection,
    table: &Arc<TableDef>,
    projection: &[SelectItem],
    selection: &Option<Expr>,
    order_by: &[OrderByExpr],
    bindings: &[Option<SqlValue>],
    optimizer: &OptimizerConfig,
    table_hint: Option<&crate::statement::TableAccessHint>,
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
        table_hint,
    );
    let is_covering = matches!(&access, AccessPath::CoveringIndexScan { .. });
    let ordering_satisfied = access_path_satisfies_ordering(
        conn, table, &access, selection, order_by, bindings, table_hint,
    );

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

fn access_path_satisfies_ordering(
    conn: &Connection,
    table: &Arc<TableDef>,
    access: &AccessPath,
    selection: &Option<Expr>,
    order_by: &[OrderByExpr],
    bindings: &[Option<SqlValue>],
    table_hint: Option<&crate::statement::TableAccessHint>,
) -> bool {
    if planner_use_access_path() {
        let ir = choose_access_path_ir(
            conn.engine(),
            table,
            selection,
            bindings,
            table_hint,
            order_by,
            None,
        );
        return ir.order_satisfies(order_by);
    }
    satisfies_ordering(table, access, order_by)
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
