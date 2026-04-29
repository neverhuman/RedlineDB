use super::*;

pub(super) fn begin_select_tx(conn: &Connection) -> Result<(Option<Txn>, bool)> {
    conn.with_session(|session| {
        if let Some(tx) = session.tx.take() {
            return Ok((Some(tx), true));
        }
        let tx = conn.engine().begin(Isolation::Snapshot)?;
        Ok((Some(tx), false))
    })
}

pub(super) fn execute_select(
    conn: &Connection,
    plan: &crate::statement::SelectPlan,
    bindings: &[Option<SqlValue>],
) -> Result<SelectRuntime> {
    let (mut tx, restore_tx) = begin_select_tx(conn)?;
    let mut memory = QueryMemoryBroker::new(
        conn.query_memory().work_mem_bytes,
        conn.query_memory().max_spill_bytes,
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
                        //   3. fallback: full heap scan.
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
                            // executor can satisfy them, but stale
                            // schema snapshots still exist as a
                            // possibility.
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
                ),
                batch: RowBatch::new(Arc::new(RowLayout {
                    columns: Arc::from([]),
                })),
                cursor: 0,
            }
        };

        let runtime_tx = tx.take();

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
            if let Some(tx) = tx.take() {
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
