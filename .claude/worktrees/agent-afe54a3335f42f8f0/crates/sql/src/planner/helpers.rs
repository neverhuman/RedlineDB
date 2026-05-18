use super::*;

pub(crate) fn estimate_scan_cost(rows: f64, width: f64) -> Cost {
    let pages = (rows / 64.0).max(1.0);
    Cost {
        startup: SEQ_PAGE_COST,
        total: pages * SEQ_PAGE_COST + rows * (CPU_TUPLE_COST + CPU_OPERATOR_COST),
        rows,
        width,
        memory_bytes: 0,
        spill_bytes: 0,
    }
}

pub(crate) fn estimate_index_cost(rows: f64, width: f64, point_lookup: bool) -> Cost {
    Cost {
        startup: INDEX_PROBE_STARTUP + if point_lookup { 0.0 } else { RANDOM_PAGE_COST },
        total: INDEX_PROBE_STARTUP
            + rows * (CPU_TUPLE_COST + CPU_OPERATOR_COST)
            + if point_lookup {
                0.0
            } else {
                rows * RANDOM_PAGE_COST / 64.0
            },
        rows,
        width,
        memory_bytes: 0,
        spill_bytes: 0,
    }
}

pub(crate) fn join_cost(kind: JoinKind, left_rows: f64, right_rows: f64) -> Cost {
    let rows = match kind {
        JoinKind::Hash | JoinKind::IndexNestedLoop | JoinKind::NestedLoop | JoinKind::Cross => {
            (left_rows * right_rows * UNKNOWN_PREDICATE_SELECTIVITY).max(1.0)
        }
    };
    Cost {
        startup: match kind {
            JoinKind::Hash => CPU_OPERATOR_COST * 10.0,
            JoinKind::IndexNestedLoop => INDEX_PROBE_STARTUP,
            JoinKind::NestedLoop | JoinKind::Cross => 0.0,
        },
        total: left_rows * CPU_TUPLE_COST + right_rows * CPU_TUPLE_COST + rows * CPU_OPERATOR_COST,
        rows,
        width: 0.0,
        memory_bytes: if kind == JoinKind::Hash {
            ((left_rows + right_rows) * 32.0) as usize
        } else {
            0
        },
        spill_bytes: 0,
    }
}

pub(crate) fn estimate_table_rows(stats: Option<&TableStats>) -> f64 {
    stats
        .map(|stats| stats.live_row_count.max(stats.row_count) as f64)
        .filter(|rows| *rows > 0.0)
        .unwrap_or(1024.0)
}

pub(crate) fn estimate_table_width(stats: Option<&TableStats>, table: &Arc<TableDef>) -> f64 {
    stats
        .map(|stats| stats.avg_row_bytes.max(1.0))
        .unwrap_or_else(|| (table.columns.len().max(1) * 16) as f64)
}

pub(crate) fn estimate_width_for_schema() -> f64 {
    64.0
}

pub(crate) fn estimate_eq_rows(
    table_stats: Option<&TableStats>,
    index: &Arc<IndexDef>,
    predicates: &[String],
) -> f64 {
    let base = estimate_table_rows(table_stats);
    let ndv = if predicates.is_empty() {
        UNKNOWN_EQ_SELECTIVITY.recip()
    } else {
        table_stats
            .map(|stats| (stats.row_count.max(1) as f64 / index.keys.len().max(1) as f64).max(1.0))
            .unwrap_or(10.0)
    };
    (base / ndv.max(1.0)).max(1.0)
}

pub(crate) fn estimate_range_rows(
    table_stats: Option<&TableStats>,
    _index: &Arc<IndexDef>,
    _predicates: &[String],
) -> f64 {
    (estimate_table_rows(table_stats) * UNKNOWN_RANGE_SELECTIVITY).max(1.0)
}

pub(crate) fn estimate_index_rows(table_stats: Option<&TableStats>, _index: &Arc<IndexDef>) -> f64 {
    estimate_table_rows(table_stats)
}

pub(crate) fn order_satisfied_by_index(
    table: &TableDef,
    index: &Arc<IndexDef>,
    order_by: &[OrderByExpr],
) -> bool {
    if order_by.is_empty() || order_by.len() > index.keys.len() {
        return order_by.is_empty();
    }
    for (order, key) in order_by.iter().zip(index.keys.iter()) {
        let Some(order_ordinal) = order_expr_column_ordinal(&order.expr, table) else {
            return false;
        };
        if order_ordinal != key.ordinal as usize {
            return false;
        }
        let ascending = order.options.asc.unwrap_or(true);
        if ascending && key.sort_dir != redlinedb_kernel::catalog::SortDir::Asc {
            return false;
        }
        if !ascending && key.sort_dir != redlinedb_kernel::catalog::SortDir::Desc {
            return false;
        }
    }
    true
}

pub(crate) fn satisfies_ordering(
    table: &Arc<TableDef>,
    access: &AccessPath,
    order_by: &[OrderByExpr],
) -> bool {
    match access {
        AccessPath::RowIdGet { .. } => order_by.is_empty(),
        AccessPath::IndexPointLookup { index, .. }
        | AccessPath::IndexRangeScan { index, .. }
        | AccessPath::CoveringIndexScan { index, .. } => {
            order_satisfied_by_index(table, index, order_by)
        }
        _ => false,
    }
}

pub(crate) fn projection_expr_covered(
    table: &Arc<TableDef>,
    covered_ordinals: &std::collections::BTreeSet<usize>,
    expr: &Expr,
) -> bool {
    match expr {
        Expr::Identifier(ident) => projection_name_covered(table, covered_ordinals, &ident.value),
        Expr::CompoundIdentifier(parts) => parts
            .last()
            .is_some_and(|ident| projection_name_covered(table, covered_ordinals, &ident.value)),
        _ => false,
    }
}

pub(crate) fn projection_name_covered(
    table: &Arc<TableDef>,
    covered_ordinals: &std::collections::BTreeSet<usize>,
    name: &str,
) -> bool {
    if is_rowid_name(table, name) {
        return true;
    }
    column_ordinal_for_table(name, table)
        .map(|ordinal| covered_ordinals.contains(&ordinal))
        .unwrap_or(false)
}

pub(crate) fn is_rowid_name(table: &Arc<TableDef>, name: &str) -> bool {
    name.eq_ignore_ascii_case("rowid")
        || name.eq_ignore_ascii_case("_rowid_")
        || name.eq_ignore_ascii_case("oid")
        || table
            .rowid_alias_column
            .and_then(|alias| table.columns.get(alias as usize))
            .is_some_and(|col| col.folded.as_ref().eq_ignore_ascii_case(name))
}

pub(crate) fn order_expr_column_ordinal(expr: &Expr, table: &TableDef) -> Option<usize> {
    match expr {
        Expr::Identifier(ident) => column_ordinal_for_table(&ident.value, table),
        Expr::CompoundIdentifier(parts) => parts
            .last()
            .and_then(|ident| column_ordinal_for_table(&ident.value, table)),
        _ => None,
    }
}

pub(crate) fn column_ordinal_for_table(name: &str, table: &TableDef) -> Option<usize> {
    table
        .columns
        .iter()
        .position(|column| column.folded.as_ref().eq_ignore_ascii_case(name))
}

pub(crate) fn eval_constant(expr: &Expr, bindings: &[Option<SqlValue>]) -> Option<SqlValue> {
    match expr {
        Expr::Value(v) => Some(match &v.value {
            sqlparser::ast::Value::Null => SqlValue::Null,
            sqlparser::ast::Value::Boolean(v) => SqlValue::Integer(if *v { 1 } else { 0 }),
            sqlparser::ast::Value::Number(n, _) => n
                .parse()
                .ok()
                .map(SqlValue::Integer)
                .unwrap_or(SqlValue::Null),
            sqlparser::ast::Value::SingleQuotedString(s) => SqlValue::Text(Arc::from(s.as_str())),
            sqlparser::ast::Value::DoubleQuotedString(s) => SqlValue::Text(Arc::from(s.as_str())),
            sqlparser::ast::Value::Placeholder(name) => {
                let slot = name
                    .strip_prefix('?')
                    .and_then(|slot| slot.parse::<usize>().ok())?;
                bindings
                    .get(slot)
                    .cloned()
                    .flatten()
                    .unwrap_or(SqlValue::Null)
            }
            _ => return None,
        }),
        Expr::Nested(expr) => eval_constant(expr, bindings),
        Expr::UnaryOp { op, expr } => {
            let value = eval_constant(expr, bindings)?;
            match op {
                sqlparser::ast::UnaryOperator::Minus => match value {
                    SqlValue::Integer(v) => Some(SqlValue::Integer(-v)),
                    SqlValue::Real(v) => Some(SqlValue::Real(-v)),
                    _ => None,
                },
                sqlparser::ast::UnaryOperator::Plus => Some(value),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(crate) fn expr_to_string(expr: &Expr) -> String {
    expr.to_string()
}

pub(crate) fn flatten_query_plan(root: &PhysicalPlan) -> Vec<FlattenedNode> {
    let mut out = Vec::new();
    flatten_node(root, None, &mut out);
    out
}

pub(crate) fn flatten_node(
    node: &PhysicalPlan,
    parent: Option<usize>,
    out: &mut Vec<FlattenedNode>,
) {
    let id = out.len();
    out.push(FlattenedNode {
        id,
        parent,
        detail: render_detail(node),
    });
    for child in &node.children {
        flatten_node(child, Some(id), out);
    }
}

pub(crate) fn render_text(root: &PhysicalPlan) -> String {
    let mut out = String::new();
    render_text_node(root, 0, &mut out);
    out
}

pub(crate) fn render_text_node(node: &PhysicalPlan, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let _ = writeln!(out, "{indent}{}", render_detail(node));
    for child in &node.children {
        render_text_node(child, depth + 1, out);
    }
}

pub(crate) fn render_json(root: &PhysicalPlan) -> String {
    let mut out = String::new();
    render_json_node(root, &mut out);
    out
}

pub(crate) fn render_json_node(node: &PhysicalPlan, out: &mut String) {
    out.push('{');
    push_json_kv(out, "kind", &format!("{:?}", node.kind));
    out.push(',');
    push_json_kv(out, "relation", node.relation.as_deref().unwrap_or(""));
    out.push(',');
    push_json_kv(out, "index", node.index.as_deref().unwrap_or(""));
    out.push(',');
    push_json_kv(out, "index_probe_kind", node.index_probe_kind.unwrap_or(""));
    out.push(',');
    push_json_num(out, "estimated_rows", node.estimated_rows);
    out.push(',');
    push_json_num(out, "startup_cost", node.cost.startup);
    out.push(',');
    push_json_num(out, "total_cost", node.cost.total);
    out.push(',');
    push_json_num(out, "width", node.cost.width);
    out.push(',');
    push_json_usize(out, "memory_bytes", node.cost.memory_bytes);
    out.push(',');
    push_json_usize(out, "spill_bytes", node.cost.spill_bytes);
    out.push(',');
    push_json_opt_usize(out, "actual_rows", node.actual_rows);
    out.push(',');
    push_json_opt_usize(out, "loops", node.loops);
    out.push(',');
    push_json_opt_num(out, "elapsed_ms", node.elapsed_ms);
    out.push(',');
    push_json_opt_usize(out, "peak_memory_bytes", node.peak_memory_bytes);
    out.push(',');
    push_json_opt_usize(out, "spill_bytes_actual", node.spill_bytes);
    out.push(',');
    push_json_array(out, "access_predicates", &node.access_predicates);
    out.push(',');
    push_json_array(out, "residual_predicates", &node.residual_predicates);
    out.push(',');
    push_json_array(out, "output_order", &node.output_order);
    out.push(',');
    push_json_array(out, "projected_columns", &node.projected_columns);
    out.push(',');
    push_json_usize(out, "memory_budget", node.memory_budget);
    out.push(',');
    out.push_str("\"children\":[");
    for (idx, child) in node.children.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        render_json_node(child, out);
    }
    out.push_str("]}");
}

pub(crate) fn render_detail(node: &PhysicalPlan) -> String {
    let mut out = String::new();
    match node.kind {
        PhysicalKind::TableScan => {
            let _ = write!(
                out,
                "SCAN TABLE {}",
                node.relation.as_deref().unwrap_or("?")
            );
        }
        PhysicalKind::RowIdGet => {
            let _ = write!(
                out,
                "SEARCH TABLE {} USING INTEGER PRIMARY KEY",
                node.relation.as_deref().unwrap_or("?")
            );
        }
        PhysicalKind::IndexScan => {
            let relation = node.relation.as_deref().unwrap_or("?");
            // Render the probe kind (PointLookup vs RangeScan) so
            // EXPLAIN consumers can distinguish equality probes from
            // range scans even though both reuse `IndexScan` as the
            // physical kind. The COVERING render is reserved for the
            // covering-index access path (which today is gated off);
            // an `index_probe_kind` of "PointLookup" or "RangeScan"
            // is the signal that we picked one of the IndexPointLookup
            // / IndexRangeScan paths and should NOT be rendered as
            // covering even if the projection happens to be a subset
            // of the index keys.
            if let Some(index) = &node.index {
                let probe = node.index_probe_kind.unwrap_or("");
                if !probe.is_empty() {
                    let _ = write!(out, "SEARCH TABLE {relation} USING INDEX {index}: {probe}");
                } else if !node.projected_columns.is_empty() {
                    let _ = write!(out, "SEARCH TABLE {relation} USING COVERING INDEX {index}");
                } else {
                    let _ = write!(out, "SEARCH TABLE {relation} USING INDEX {index}");
                }
            } else {
                let _ = write!(out, "SEARCH TABLE {relation} USING INDEX");
            }
        }
        PhysicalKind::MultiIndexScan => {
            let _ = write!(
                out,
                "MULTI-INDEX SCAN {}",
                node.relation.as_deref().unwrap_or("?")
            );
        }
        PhysicalKind::Filter => {
            let _ = write!(out, "FILTER");
        }
        PhysicalKind::Project => {
            let _ = write!(out, "PROJECT");
        }
        PhysicalKind::NestedLoopJoin => {
            let _ = write!(out, "NESTED LOOP JOIN");
        }
        PhysicalKind::IndexNestedLoopJoin => {
            let _ = write!(out, "INDEX NESTED LOOP JOIN");
        }
        PhysicalKind::HashJoin => {
            let _ = write!(out, "HASH JOIN");
        }
        PhysicalKind::StreamingAggregate => {
            let _ = write!(out, "STREAMING AGGREGATE");
        }
        PhysicalKind::HashAggregate => {
            let _ = write!(out, "HASH AGGREGATE");
        }
        PhysicalKind::Sort => {
            let _ = write!(out, "SORT");
        }
        PhysicalKind::TopN => {
            let _ = write!(out, "TOP-N");
        }
        PhysicalKind::Limit => {
            let _ = write!(out, "LIMIT");
        }
        PhysicalKind::Explain => {
            let _ = write!(out, "EXPLAIN");
        }
        PhysicalKind::Constant => {
            let _ = write!(
                out,
                "{}",
                node.relation.as_deref().unwrap_or("CONSTANT ROW")
            );
        }
    }
    if !node.access_predicates.is_empty() {
        let _ = write!(out, " access=[{}]", node.access_predicates.join(", "));
    }
    if !node.residual_predicates.is_empty() {
        let _ = write!(out, " residual=[{}]", node.residual_predicates.join(", "));
    }
    if !node.output_order.is_empty() {
        let _ = write!(out, " order=[{}]", node.output_order.join(", "));
    }
    if !node.projected_columns.is_empty() {
        let _ = write!(out, " columns=[{}]", node.projected_columns.join(", "));
    }
    let _ = write!(
        out,
        " rows={:.3} total_cost={:.3}",
        node.estimated_rows, node.cost.total
    );
    if let Some(actual_rows) = node.actual_rows {
        let _ = write!(out, " actual_rows={actual_rows}");
    }
    if let Some(loops) = node.loops {
        let _ = write!(out, " loops={loops}");
    }
    if let Some(elapsed) = node.elapsed_ms {
        let _ = write!(out, " elapsed_ms={elapsed:.3}");
    }
    if let Some(peak) = node.peak_memory_bytes {
        let _ = write!(out, " peak_memory_bytes={peak}");
    }
    if let Some(spill) = node.spill_bytes {
        let _ = write!(out, " spill_bytes={spill}");
    }
    out
}

pub(crate) fn push_json_kv(out: &mut String, key: &str, value: &str) {
    let _ = write!(out, "\"{}\":\"{}\"", escape_json(key), escape_json(value));
}

pub(crate) fn push_json_num(out: &mut String, key: &str, value: f64) {
    let _ = write!(out, "\"{}\":{value}", escape_json(key));
}

pub(crate) fn push_json_usize(out: &mut String, key: &str, value: usize) {
    let _ = write!(out, "\"{}\":{value}", escape_json(key));
}

pub(crate) fn push_json_opt_usize(out: &mut String, key: &str, value: Option<usize>) {
    let _ = write!(out, "\"{}\":", escape_json(key));
    match value {
        Some(v) => {
            let _ = write!(out, "{v}");
        }
        None => out.push_str("null"),
    }
}

pub(crate) fn push_json_opt_num(out: &mut String, key: &str, value: Option<f64>) {
    let _ = write!(out, "\"{}\":", escape_json(key));
    match value {
        Some(v) => {
            let _ = write!(out, "{v}");
        }
        None => out.push_str("null"),
    }
}

pub(crate) fn push_json_array(out: &mut String, key: &str, values: &[String]) {
    let _ = write!(out, "\"{}\":[", escape_json(key));
    for (idx, value) in values.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        let _ = write!(out, "\"{}\"", escape_json(value));
    }
    out.push(']');
}

pub(crate) fn escape_json(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}

pub(crate) fn selection_rowid_eq(
    table: &Arc<TableDef>,
    selection: &Option<Expr>,
    bindings: &[Option<SqlValue>],
) -> Result<Option<RowId>> {
    let Some(expr) = selection else {
        return Ok(None);
    };
    let rowid_col = |name: &str| {
        name.eq_ignore_ascii_case("rowid")
            || name.eq_ignore_ascii_case("_rowid_")
            || name.eq_ignore_ascii_case("oid")
            || table
                .rowid_alias_column
                .and_then(|alias| table.columns.get(alias as usize))
                .is_some_and(|col| col.folded.as_ref().eq_ignore_ascii_case(name))
    };
    let Expr::BinaryOp { left, op, right } = expr else {
        return Ok(None);
    };
    if !matches!(op, BinaryOperator::Eq) {
        return Ok(None);
    }
    let expr_rowid = if let Some(value) = rowid_eq_side(table, left, right, bindings, &rowid_col)? {
        value
    } else if let Some(value) = rowid_eq_side(table, right, left, bindings, &rowid_col)? {
        value
    } else {
        return Ok(None);
    };
    Ok(Some(expr_rowid))
}

pub(crate) fn rowid_eq_side(
    _table: &Arc<TableDef>,
    ident_side: &Expr,
    value_side: &Expr,
    bindings: &[Option<SqlValue>],
    rowid_col: &impl Fn(&str) -> bool,
) -> Result<Option<RowId>> {
    let name = match ident_side {
        Expr::Identifier(ident) if rowid_col(&ident.value) => Some(ident.value.as_str()),
        Expr::CompoundIdentifier(parts) => parts.last().and_then(|ident| {
            if rowid_col(&ident.value) {
                Some(ident.value.as_str())
            } else {
                None
            }
        }),
        _ => None,
    };
    if name.is_none() {
        return Ok(None);
    }
    let value = eval_constant(value_side, bindings).unwrap_or(SqlValue::Null);
    match value {
        SqlValue::Integer(v) if v >= 0 => Ok(Some(RowId::new(v as u64))),
        SqlValue::Real(v) if v >= 0.0 && v.fract() == 0.0 => Ok(Some(RowId::new(v as u64))),
        SqlValue::Null => Ok(None),
        _ => Err(Error::DatatypeMismatch),
    }
}
