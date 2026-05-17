use super::*;

pub(super) fn begin_select_tx(conn: &Connection) -> Result<(SelectRuntimeTx, bool)> {
    if let Some(tx_ptr) = current_tx() {
        return Ok((SelectRuntimeTx::Borrowed(tx_ptr), false));
    }
    conn.with_session(|session| {
        if let Some(tx) = session.tx.take() {
            return Ok((SelectRuntimeTx::Owned(tx), true));
        }
        let tx = conn.engine().begin(Isolation::Snapshot)?;
        Ok((SelectRuntimeTx::Owned(tx), false))
    })
}

pub(super) fn execute_select(
    conn: &Connection,
    plan: &crate::statement::SelectPlan,
    bindings: &[Option<SqlValue>],
) -> Result<SelectRuntime> {
    let (mut tx, restore_tx) = begin_select_tx(conn)?;
    let temp_dir = conn.temp_dir().map(|path| path.to_path_buf());
    let mut memory = QueryMemoryBroker::new(
        conn.query_memory().work_mem_bytes,
        conn.query_memory().max_spill_bytes,
        temp_dir.clone(),
    );
    let result = (|| -> Result<SelectRuntime> {
        let limit = match &plan.limit {
            Some(expr) => scalar_to_usize(&eval_scalar(expr, &RowContext::Empty, bindings)?)?,
            None => usize::MAX,
        };
        let offset = match &plan.offset {
            Some(expr) => scalar_to_usize(&eval_scalar(expr, &RowContext::Empty, bindings)?)?,
            None => 0,
        };

        // Phase 11 W1-E: SELECT COUNT(*) FROM t WHERE k BETWEEN ? AND ?
        // fast-path. Drains the cursor without loading heap rows; the
        // result is a single integer row that flows through the rest
        // of the pipeline (LIMIT/OFFSET) like any other StaticRows
        // source. Materializes early-return rows here; runtime
        // assembly happens at the end of `result` so the outer
        // closure can move the transaction out of the runtime without
        // surprising the borrow checker.
        let mut fast_path_rows: Option<Vec<Vec<SqlValue>>> = None;
        if let SelectSource::Table(table) = &plan.source
            && plan.group_by.is_empty()
            && !plan.distinct
            && plan.order_by.is_empty()
            && plan.having.is_none()
            && is_count_star_only_projection(&plan.projection)
            && let Some(matched) = index_access::try_match_index_access(
                conn.engine(),
                table,
                &plan.selection,
                bindings,
            )
            && let index_access::IndexProbe::Range { start, end } = &matched.probe
            && index_access::open_handle(conn.engine(), &matched.index).is_some()
        {
            let tx_ref = tx.as_mut().expect("tx present");
            let count = index_access::execute_index_count_range(
                conn.engine(),
                tx_ref,
                &matched.index,
                start,
                end,
            )?;
            fast_path_rows = Some(vec![vec![SqlValue::Integer(count)]]);
        }

        // Phase 11 W1-E: simple covering scan. Same shape as above:
        // build the result rows up front, defer runtime assembly to
        // the unified bottom block.
        if fast_path_rows.is_none()
            && let SelectSource::Table(table) = &plan.source
            && plan.group_by.is_empty()
            && !plan.distinct
            && !select_requires_aggregation(plan)
            && plan.having.is_none()
            && let Some(matched) = index_access::try_match_index_access(
                conn.engine(),
                table,
                &plan.selection,
                bindings,
            )
            && let index_access::IndexProbe::Range { start, end } = &matched.probe
            && index_access::open_handle(conn.engine(), &matched.index).is_some()
            && let Some(out_columns) =
                covering_projection_for_index(table, &matched.index, &plan.projection)
            && covering_order_satisfies(&matched.index, table, &plan.order_by)
        {
            let tx_ref = tx.as_mut().expect("tx present");
            let cover_limit = if plan.order_by.is_empty() {
                None
            } else if limit < usize::MAX {
                Some(limit.saturating_add(offset))
            } else {
                None
            };
            let rows = index_access::execute_index_covering_range(
                conn.engine(),
                tx_ref,
                &matched.index,
                start,
                end,
                &out_columns,
                cover_limit,
            )?;
            fast_path_rows = Some(rows);
        }

        if let Some(rows) = fast_path_rows {
            let runtime_tx = std::mem::replace(&mut tx, SelectRuntimeTx::Empty);
            return Ok(SelectRuntime {
                tx: runtime_tx,
                restore_tx,
                source: SelectRuntimeSource::StaticRows {
                    rows: Arc::from(rows),
                    cursor: 0,
                },
                selection: None,
                projection: Vec::new(),
                limit,
                offset,
                seen: 0,
                yielded: 0,
                memory,
            });
        }

        // Window-function fast-path: when the projection contains an
        // `OVER (...)` call, route the entire SELECT through the window
        // pipeline. The window post-processor handles materialization,
        // partitioning, framing, projection, ORDER BY, and LIMIT/OFFSET
        // in one pass so non-window items still evaluate per-row against
        // the original row source.
        if super::window::projection_has_window(&plan.projection) {
            let base_rows = collect_select_rows(
                conn,
                conn.engine(),
                tx.as_mut().expect("tx present"),
                &plan.source,
                &plan.selection,
                bindings,
            )?;
            let filtered: Vec<SqlRow> = base_rows
                .into_iter()
                .filter_map(|row| match selection_passes(&plan.selection, &row, bindings) {
                    Ok(true) => Some(Ok(row)),
                    Ok(false) => None,
                    Err(e) => Some(Err(e)),
                })
                .collect::<Result<Vec<_>>>()?;
            let mut projected = super::window::evaluate_window_functions(
                &filtered,
                &plan.projection,
                bindings,
            )?;
            if !plan.order_by.is_empty() {
                super::agg::sort_projected_rows_by_order_by(
                    &mut projected,
                    &plan.projection,
                    &plan.order_by,
                    bindings,
                )?;
            }
            let projected: Vec<Vec<SqlValue>> = projected
                .into_iter()
                .skip(offset)
                .take(limit)
                .collect();
            let runtime_tx = std::mem::replace(&mut tx, SelectRuntimeTx::Empty);
            return Ok(SelectRuntime {
                tx: runtime_tx,
                restore_tx,
                source: SelectRuntimeSource::StaticRows {
                    rows: Arc::from(projected),
                    cursor: 0,
                },
                selection: None,
                projection: Vec::new(),
                limit: usize::MAX,
                offset: 0,
                seen: 0,
                yielded: 0,
                memory,
            });
        }

        let source = if plan.group_by.is_empty()
            && !select_requires_aggregation(plan)
            && !plan.distinct
        {
            match &plan.source {
                SelectSource::Table(table) => {
                    if plan.order_by.is_empty() {
                        // Lane C access-path resolution. Order of
                        // preference matches the planner:
                        //   1. rowid PK fast path (already covered by
                        //      `selection_rowid_eq` / RowIdGet).
                        //   2. physical-index probe (point or range).
                        //   3. default path: full heap scan.
                        let rowids = if let Some(rowid) =
                            selection_rowid_eq(table, &plan.selection, bindings)?
                        {
                            vec![rowid]
                        } else if let Some(matched) = index_access::try_match_index_access(
                            conn.engine(),
                            table,
                            &plan.selection,
                            bindings,
                        ) {
                            let tx = tx.as_mut().expect("tx present");
                            // Conservatism: if the kernel can't honor
                            // the probe (e.g. the index has no live
                            // physical handle yet), fall through to a
                            // table scan rather than returning an empty
                            // result. The planner only emits
                            // IndexPointLookup/IndexRangeScan when the
                            // executor can satisfy them, but a lagging
                            // schema snapshot can still exist.
                            if index_access::open_handle(conn.engine(), &matched.index).is_some() {
                                index_access::execute_index_probe(
                                    conn.engine(),
                                    tx,
                                    table,
                                    &matched.index,
                                    &matched.probe,
                                )?
                            } else {
                                collect_table_rowids(conn.engine(), tx, table)?
                            }
                        } else {
                            let tx = tx.as_mut().expect("tx present");
                            collect_table_rowids(conn.engine(), tx, table)?
                        };
                        SelectRuntimeSource::Table {
                            table: Arc::clone(table),
                            rowids,
                            cursor: 0,
                        }
                    } else if let Some(rowids) = try_ordered_index_limit_path(
                        conn,
                        tx.as_mut().expect("tx present"),
                        plan,
                        bindings,
                        table,
                        limit,
                        offset,
                    )? {
                        // Phase 11 W1-D: ORDER BY k LIMIT n where the
                        // index leading column matches `k`. The cursor
                        // emits in key order, so we collect rowids in
                        // that order with an early stop and let the
                        // standard runtime project + apply LIMIT/OFFSET
                        // without re-sorting.
                        SelectRuntimeSource::Table {
                            table: Arc::clone(table),
                            rowids,
                            cursor: 0,
                        }
                    } else {
                        let tx = tx.as_mut().expect("tx present");
                        let rows =
                            table_rows_for_select(conn, tx, table, &plan.selection, bindings)?
                                .into_iter()
                                .map(SqlRow::Table)
                                .collect::<Vec<_>>();
                        SelectRuntimeSource::Batched {
                            node: MaterializeNode::new(order_and_project_rows(
                                rows,
                                &plan.selection,
                                &plan.order_by,
                                bindings,
                                &plan.projection,
                                limit,
                                offset,
                                &mut memory,
                            )?),
                            ctx: ExecContext::new(
                                conn.query_memory().work_mem_bytes,
                                conn.query_memory().max_spill_bytes,
                                temp_dir.clone(),
                            ),
                            batch: RowBatch::new(Arc::new(RowLayout {
                                columns: Arc::from([]),
                            })),
                            cursor: 0,
                        }
                    }
                }
                SelectSource::Tables(tables) => {
                    let rows =
                        collect_join_rows(conn.engine(), tx.as_mut().expect("tx present"), tables)?;
                    SelectRuntimeSource::Batched {
                        node: MaterializeNode::new(order_and_project_rows(
                            rows,
                            &plan.selection,
                            &plan.order_by,
                            bindings,
                            &plan.projection,
                            limit,
                            offset,
                            &mut memory,
                        )?),
                        ctx: ExecContext::new(
                            conn.query_memory().work_mem_bytes,
                            conn.query_memory().max_spill_bytes,
                            temp_dir.clone(),
                        ),
                        batch: RowBatch::new(Arc::new(RowLayout {
                            columns: Arc::from([]),
                        })),
                        cursor: 0,
                    }
                }
                SelectSource::Joined(source) => {
                    let rows = collect_join_source_rows(
                        conn.engine(),
                        tx.as_mut().expect("tx present"),
                        source,
                        bindings,
                    )?;
                    SelectRuntimeSource::Batched {
                        node: MaterializeNode::new(order_and_project_rows(
                            rows,
                            &plan.selection,
                            &plan.order_by,
                            bindings,
                            &plan.projection,
                            limit,
                            offset,
                            &mut memory,
                        )?),
                        ctx: ExecContext::new(
                            conn.query_memory().work_mem_bytes,
                            conn.query_memory().max_spill_bytes,
                            temp_dir.clone(),
                        ),
                        batch: RowBatch::new(Arc::new(RowLayout {
                            columns: Arc::from([]),
                        })),
                        cursor: 0,
                    }
                }
                SelectSource::SqliteSchema => {
                    let rows = conn.engine().sqlite_schema();
                    if !plan.order_by.is_empty() {
                        let sqlite_rows = rows
                            .into_iter()
                            .map(SqlRow::SqliteSchema)
                            .collect::<Vec<_>>();
                        SelectRuntimeSource::Batched {
                            node: MaterializeNode::new(order_and_project_rows(
                                sqlite_rows,
                                &plan.selection,
                                &plan.order_by,
                                bindings,
                                &plan.projection,
                                limit,
                                offset,
                                &mut memory,
                            )?),
                            ctx: ExecContext::new(
                                conn.query_memory().work_mem_bytes,
                                conn.query_memory().max_spill_bytes,
                                temp_dir.clone(),
                            ),
                            batch: RowBatch::new(Arc::new(RowLayout {
                                columns: Arc::from([]),
                            })),
                            cursor: 0,
                        }
                    } else {
                        SelectRuntimeSource::SqliteSchema { rows, cursor: 0 }
                    }
                }
                SelectSource::StaticRows { rows } => SelectRuntimeSource::StaticRows {
                    rows: Arc::clone(rows),
                    cursor: 0,
                },
                SelectSource::Cte {
                    name,
                    alias,
                    columns,
                    rows,
                } => {
                    // CTE rows go through the Batched path so projection /
                    // selection / order-by can resolve column names. Pre-wrap
                    // each row as `SqlRow::Cte` to retain column metadata.
                    let sql_rows: Vec<SqlRow> = rows
                        .iter()
                        .cloned()
                        .map(|values| {
                            SqlRow::Cte(crate::exec::expr::scalar::row::CteRow {
                                name: Arc::clone(name),
                                alias: alias.clone(),
                                columns: Arc::clone(columns),
                                values,
                            })
                        })
                        .collect();
                    SelectRuntimeSource::Batched {
                        node: MaterializeNode::new(order_and_project_rows(
                            sql_rows,
                            &plan.selection,
                            &plan.order_by,
                            bindings,
                            &plan.projection,
                            limit,
                            offset,
                            &mut memory,
                        )?),
                        ctx: ExecContext::new(
                            conn.query_memory().work_mem_bytes,
                            conn.query_memory().max_spill_bytes,
                            temp_dir.clone(),
                        ),
                        batch: RowBatch::new(Arc::new(RowLayout {
                            columns: Arc::from([]),
                        })),
                        cursor: 0,
                    }
                }
                SelectSource::CompoundAll(branches) => {
                    let rows = collect_compound_all_rows(conn, branches, bindings)?
                        .into_iter()
                        .map(SqlRow::Static)
                        .collect::<Vec<_>>();
                    SelectRuntimeSource::Batched {
                        node: MaterializeNode::new(order_and_project_rows(
                            rows,
                            &plan.selection,
                            &plan.order_by,
                            bindings,
                            &plan.projection,
                            limit,
                            offset,
                            &mut memory,
                        )?),
                        ctx: ExecContext::new(
                            conn.query_memory().work_mem_bytes,
                            conn.query_memory().max_spill_bytes,
                            temp_dir.clone(),
                        ),
                        batch: RowBatch::new(Arc::new(RowLayout {
                            columns: Arc::from([]),
                        })),
                        cursor: 0,
                    }
                }
                SelectSource::Empty => SelectRuntimeSource::Empty,
            }
        } else {
            let rows = collect_select_rows(
                conn,
                conn.engine(),
                tx.as_mut().expect("tx present"),
                &plan.source,
                &plan.selection,
                bindings,
            )?;
            let rows = execute_grouped_select(plan, rows, bindings, limit, offset, &mut memory)?;
            SelectRuntimeSource::Batched {
                node: MaterializeNode::new(rows),
                ctx: ExecContext::new(
                    conn.query_memory().work_mem_bytes,
                    conn.query_memory().max_spill_bytes,
                    temp_dir.clone(),
                ),
                batch: RowBatch::new(Arc::new(RowLayout {
                    columns: Arc::from([]),
                })),
                cursor: 0,
            }
        };

        let runtime_tx = std::mem::replace(&mut tx, SelectRuntimeTx::Empty);
        Ok(SelectRuntime {
            tx: runtime_tx,
            restore_tx,
            source,
            selection: plan.selection.clone(),
            projection: plan.projection.clone(),
            limit,
            offset,
            seen: 0,
            yielded: 0,
            memory,
        })
    })();

    match result {
        Ok(runtime) => Ok(runtime),
        Err(err) => {
            if let Some(tx) = tx.take_owned() {
                if restore_tx {
                    conn.with_session(|session| {
                        session.tx = Some(tx);
                        Ok(())
                    })?;
                } else {
                    let _ = conn.engine().rollback(tx);
                }
            }
            Err(err)
        }
    }
}

fn table_rows_for_select(
    conn: &Connection,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    selection: &Option<Expr>,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<TableRow>> {
    if let Some(rowid) = selection_rowid_eq(table, selection, bindings)?
        && let Some(row) = load_table_row_by_rowid(conn.engine(), tx, table, rowid)?
    {
        return Ok(vec![row]);
    }

    if let Some(matched) =
        index_access::try_match_index_access(conn.engine(), table, selection, bindings)
        && index_access::open_handle(conn.engine(), &matched.index).is_some()
    {
        let rowids = index_access::execute_index_probe(
            conn.engine(),
            tx,
            table,
            &matched.index,
            &matched.probe,
        )?;
        let mut rows = Vec::with_capacity(rowids.len());
        for rowid in rowids {
            if let Some(row) = load_table_row_by_rowid(conn.engine(), tx, table, rowid)? {
                rows.push(row);
            }
        }
        return Ok(rows);
    }

    collect_table_rows(conn.engine(), tx, table)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn order_and_project_rows(
    rows: Vec<SqlRow>,
    selection: &Option<Expr>,
    order_by: &[OrderByExpr],
    bindings: &[Option<SqlValue>],
    projection: &[SelectItem],
    limit: usize,
    offset: usize,
    memory: &mut QueryMemoryBroker,
) -> Result<Vec<Vec<SqlValue>>> {
    let mut filtered = Vec::with_capacity(rows.len());
    for row in rows {
        if selection_passes(selection, &row, bindings)? {
            filtered.push(row);
        }
    }

    // Lane VE top-K fast path: ORDER BY ... LIMIT k where k is small wants
    // a fixed-size heap, not a full sort. The threshold matches
    // `vec::TOPK_LIMIT_THRESHOLD`.
    let total_take = limit.saturating_add(offset);
    if !order_by.is_empty()
        && total_take > 0
        && total_take <= vec::TOPK_LIMIT_THRESHOLD
        && limit < usize::MAX
    {
        let directions = directions_from_order_by(order_by);
        let mut heap = vec::TopKHeap::new(total_take, directions);
        for row in &filtered {
            let keys = order_by
                .iter()
                .map(|order| eval_order_key(order, &row.context(), bindings))
                .collect::<Result<Vec<_>>>()?;
            let projected = project_row(projection, row, bindings)?;
            heap.push(keys, projected)?;
        }
        let sorted = heap.into_sorted_rows();
        return Ok(sorted.into_iter().skip(offset).take(limit).collect());
    }

    if order_by.is_empty() {
        let memory_bytes = filtered.iter().try_fold(0usize, |acc, row| {
            row.values().map(|values| acc + row_width(&values))
        })?;
        memory.request(memory_bytes)?;
        let mut out = Vec::new();
        for row in filtered.into_iter().skip(offset).take(limit) {
            out.push(project_row(projection, &row, bindings)?);
        }
        return Ok(out);
    }

    // Spillable sort path: project first so the sort buffer stores only the
    // emitted columns. Memory accounting happens inside `SpillSort` via the
    // configured budget.
    let directions = directions_from_order_by(order_by);
    let mut projected_with_keys: Vec<(Vec<SqlValue>, Vec<SqlValue>)> =
        Vec::with_capacity(filtered.len());
    for row in &filtered {
        let keys = order_by
            .iter()
            .map(|order| eval_order_key(order, &row.context(), bindings))
            .collect::<Result<Vec<_>>>()?;
        let projected = project_row(projection, row, bindings)?;
        projected_with_keys.push((keys, projected));
    }
    let work_mem = memory.work_mem_bytes;
    let max_spill = memory.max_spill_bytes;
    let order_len = order_by.len();
    let mut sorter = vec::SpillSort::new(
        directions,
        work_mem,
        max_spill,
        memory.spill_root().to_path_buf(),
        move |row: &[SqlValue]| -> Result<Vec<SqlValue>> {
            // Keys are stored as the first `order_len` cells in the SpillSort
            // input rows; downstream we strip them.
            Ok(row[..order_len].to_vec())
        },
    );
    for (keys, projected) in projected_with_keys {
        let mut combined = Vec::with_capacity(keys.len() + projected.len());
        combined.extend(keys);
        combined.extend(projected);
        sorter.push(combined)?;
    }
    let spilled = sorter.total_spilled_bytes();
    if spilled > 0 {
        // Surface the spill to the broker so `peak_memory_bytes` /
        // `spill_bytes` telemetry stays accurate.
        memory.request(spilled as usize)?;
    }
    let sorted = sorter.finish()?;
    Ok(sorted
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|row| row[order_len..].to_vec())
        .collect())
}

pub(super) fn directions_from_order_by(order_by: &[OrderByExpr]) -> Vec<vec::SortDirection> {
    order_by
        .iter()
        .map(|order| match order.options.asc {
            Some(false) => vec::SortDirection::Desc,
            _ => vec::SortDirection::Asc,
        })
        .collect()
}

fn eval_order_key(
    order: &OrderByExpr,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    normalize_order_key(order, eval_scalar(&order.expr, row, bindings)?)
}

fn normalize_order_key(order: &OrderByExpr, value: SqlValue) -> Result<SqlValue> {
    let Some(collation) = collation_from_expr(&order.expr) else {
        return Ok(value);
    };
    Ok(match (collation, value) {
        (crate::collation::Collation::NoCase, SqlValue::Text(text)) => {
            SqlValue::Text(Arc::from(text.to_ascii_lowercase()))
        }
        (crate::collation::Collation::RTrim, SqlValue::Text(text)) => {
            SqlValue::Text(Arc::from(text.trim_end_matches(' ')))
        }
        (_, value) => value,
    })
}

pub(super) fn collect_select_rows(
    conn: &Connection,
    engine: &Engine,
    tx: &mut Txn,
    source: &SelectSource,
    selection: &Option<Expr>,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlRow>> {
    match source {
        SelectSource::Table(table) => {
            Ok(table_rows_for_select(conn, tx, table, selection, bindings)?
                .into_iter()
                .map(SqlRow::Table)
                .collect())
        }
        SelectSource::Tables(tables) => collect_join_rows(engine, tx, tables),
        SelectSource::Joined(source) => collect_join_source_rows(engine, tx, source, bindings),
        SelectSource::SqliteSchema => Ok(engine
            .sqlite_schema()
            .into_iter()
            .map(SqlRow::SqliteSchema)
            .collect()),
        SelectSource::StaticRows { rows } => Ok(rows.iter().cloned().map(SqlRow::Static).collect()),
        SelectSource::Cte {
            name,
            alias,
            columns,
            rows,
        } => Ok(rows
            .iter()
            .cloned()
            .map(|values| {
                SqlRow::Cte(crate::exec::expr::scalar::row::CteRow {
                    name: Arc::clone(name),
                    alias: alias.clone(),
                    columns: Arc::clone(columns),
                    values,
                })
            })
            .collect()),
        SelectSource::CompoundAll(branches) => {
            Ok(collect_compound_all_rows(conn, branches, bindings)?
                .into_iter()
                .map(SqlRow::Static)
                .collect())
        }
        SelectSource::Empty => Ok(vec![SqlRow::Empty]),
    }
}

fn collect_compound_all_rows(
    conn: &Connection,
    branches: &[SelectPlan],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<Vec<SqlValue>>> {
    let mut out = Vec::new();
    for branch in branches {
        out.extend(materialize_select_plan_rows(conn, branch, bindings)?);
    }
    Ok(out)
}

// ============================================================
// Phase 11 W1-D / W1-E helpers: index-aware ORDER-BY-LIMIT, COUNT(*)
// fast path, and simple covering scans.
// ============================================================

/// Phase 11 W1-E: returns `true` iff `projection` is exactly one
/// `COUNT(*)` aggregate (with or without an alias) and nothing else.
fn is_count_star_only_projection(projection: &[SelectItem]) -> bool {
    if projection.len() != 1 {
        return false;
    }
    let inner = match &projection[0] {
        SelectItem::UnnamedExpr(expr) => expr,
        SelectItem::ExprWithAlias { expr, .. } => expr,
        _ => return false,
    };
    let Expr::Function(func) = inner else {
        return false;
    };
    if !func.name.to_string().eq_ignore_ascii_case("count") {
        return false;
    }
    let FunctionArguments::List(list) = &func.args else {
        return false;
    };
    list.args.len() == 1
        && matches!(
            list.args[0],
            FunctionArg::Unnamed(FunctionArgExpr::Wildcard)
        )
}

/// Phase 11 W1-E: build an `OutputColumnSource` per projected column
/// when the projection is fully covered by `index`. Returns `None`
/// when the SELECT mentions any column the index does not carry, or
/// when the projection contains expressions / aliases / aggregates.
///
/// Plain column indexes only this wave — no expression / partial /
/// generated-column covers.
fn covering_projection_for_index(
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    index: &redlinedb_kernel::catalog::IndexDef,
    projection: &[SelectItem],
) -> Option<Vec<index_access::OutputColumnSource>> {
    use redlinedb_kernel::catalog::IndexKeySource;
    if projection.is_empty() {
        return None;
    }
    // Map from table-column ordinal -> position within the index keys.
    let mut col_to_index_pos: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    for (pos, key) in index.keys.iter().enumerate() {
        let IndexKeySource::Column { attnum } = key.source;
        col_to_index_pos.insert(attnum as usize, pos);
    }
    let mut out: Vec<index_access::OutputColumnSource> = Vec::with_capacity(projection.len());
    for item in projection {
        let expr = match item {
            SelectItem::UnnamedExpr(expr) => expr,
            // Aliases are fine for the covering case as long as the
            // underlying expression resolves to a covered column.
            SelectItem::ExprWithAlias { expr, .. } => expr,
            // Wildcards / qualified wildcards force a fall-back; a
            // covering scan can't synthesize the full row from a
            // partial index.
            _ => return None,
        };
        let column_name = match expr {
            Expr::Identifier(ident) => ident.value.as_str(),
            Expr::CompoundIdentifier(parts) => parts.last()?.value.as_str(),
            _ => return None,
        };
        // Rowid alias (and explicit `rowid` / `_rowid_` / `oid`) lives
        // on `IndexRowRef.row_id` — covered without decoding the leaf
        // key.
        let rowid_alias_name: Option<String> = table
            .rowid_alias_column
            .and_then(|alias| table.columns.get(alias as usize))
            .map(|col| col.folded.as_ref().to_owned());
        if column_name.eq_ignore_ascii_case("rowid")
            || column_name.eq_ignore_ascii_case("_rowid_")
            || column_name.eq_ignore_ascii_case("oid")
            || rowid_alias_name
                .as_deref()
                .is_some_and(|alias| alias.eq_ignore_ascii_case(column_name))
        {
            out.push(index_access::OutputColumnSource::Rowid);
            continue;
        }
        let table_ord = table
            .columns
            .iter()
            .position(|c| c.folded.as_ref().eq_ignore_ascii_case(column_name))?;
        let index_pos = *col_to_index_pos.get(&table_ord)?;
        out.push(index_access::OutputColumnSource::IndexColumn { ordinal: index_pos });
    }
    Some(out)
}

/// Phase 11 W1-E: for the covering path, the cursor already emits in
/// the index leading-column order. `ORDER BY k` (or no ORDER BY)
/// matches; anything else needs a downstream sort and falls through.
fn covering_order_satisfies(
    index: &redlinedb_kernel::catalog::IndexDef,
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    order_by: &[OrderByExpr],
) -> bool {
    if order_by.is_empty() {
        return true;
    }
    if order_by.len() != 1 {
        return false;
    }
    let item = &order_by[0];
    if matches!(item.options.asc, Some(false)) {
        // Desc ORDER BY does not match an Asc index; the cursor walks
        // left-to-right and does not currently support reverse
        // iteration. Use the sort path instead.
        return false;
    }
    let Expr::Identifier(ident) = &item.expr else {
        return false;
    };
    let Some(first_key) = index.keys.first() else {
        return false;
    };
    let redlinedb_kernel::catalog::IndexKeySource::Column { attnum } = first_key.source;
    table
        .columns
        .get(attnum as usize)
        .is_some_and(|col| col.folded.as_ref().eq_ignore_ascii_case(&ident.value))
}

fn order_by_rowid_alias(
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    order_by: &[OrderByExpr],
) -> bool {
    if order_by.len() != 1 {
        return false;
    }
    let item = &order_by[0];
    if matches!(item.options.asc, Some(false)) {
        return false;
    }
    let rowid_col = |name: &str| {
        name.eq_ignore_ascii_case("rowid")
            || name.eq_ignore_ascii_case("_rowid_")
            || name.eq_ignore_ascii_case("oid")
            || table
                .rowid_alias_column
                .and_then(|alias| table.columns.get(alias as usize))
                .is_some_and(|col| col.folded.as_ref().eq_ignore_ascii_case(name))
    };
    match &item.expr {
        Expr::Identifier(ident) => rowid_col(&ident.value),
        Expr::CompoundIdentifier(parts) => {
            parts.last().is_some_and(|ident| rowid_col(&ident.value))
        }
        _ => false,
    }
}

/// Phase 11 W1-D: when the SELECT has `ORDER BY k LIMIT n` and `k`
/// matches the leading column of the index implied by `selection`,
/// return rowids in that order with the limit honored as a hard
/// early-stop. Returns `None` when the conditions don't fit, so the
/// caller falls back to the full sort+limit path.
fn try_ordered_index_limit_path(
    conn: &Connection,
    tx: &mut Txn,
    plan: &SelectPlan,
    bindings: &[Option<SqlValue>],
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    limit: usize,
    offset: usize,
) -> Result<Option<Vec<RowId>>> {
    if plan.order_by.is_empty() || limit == usize::MAX {
        return Ok(None);
    }
    let Some(matched) =
        index_access::try_match_index_access(conn.engine(), table, &plan.selection, bindings)
    else {
        return Ok(None);
    };
    if matched.index.keys.len() == 1
        && order_by_rowid_alias(table, &plan.order_by)
        && matches!(matched.probe, index_access::IndexProbe::Point { .. })
    {
        if index_access::open_handle(conn.engine(), &matched.index).is_none() {
            return Ok(None);
        }
        let take = limit.saturating_add(offset);
        let rowids = index_access::execute_index_probe_with_limit(
            conn.engine(),
            tx,
            table,
            &matched.index,
            &matched.probe,
            Some(take),
        )?;
        return Ok(Some(rowids));
    }
    if !matches!(matched.probe, index_access::IndexProbe::Range { .. }) {
        return Ok(None);
    }
    if !covering_order_satisfies(&matched.index, table, &plan.order_by) {
        return Ok(None);
    }
    if index_access::open_handle(conn.engine(), &matched.index).is_none() {
        return Ok(None);
    }
    let take = limit.saturating_add(offset);
    let rowids = index_access::execute_index_probe_with_limit(
        conn.engine(),
        tx,
        table,
        &matched.index,
        &matched.probe,
        Some(take),
    )?;
    Ok(Some(rowids))
}
