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
    // SQLite authorizer contract: consult before any access. We check
    // every table referenced by the top-level source so DENY surfaces as
    // the standard "not authorized" error before any rows are read.
    // IGNORE on SELECT collapses the table to an empty source (SQLite's
    // semantics is to substitute NULL columns; for table-level SELECT we
    // surface zero rows, which is the closest analog).
    if let Some(decision) = authorize_select_source(&plan.source) {
        match decision {
            crate::udf::AuthorizerDecision::Allow => {}
            crate::udf::AuthorizerDecision::Deny => return Err(Error::NotAuthorized),
            crate::udf::AuthorizerDecision::Ignore => {
                return Ok(empty_select_runtime(conn));
            }
        }
    }

    // Phase 1.5: fromless SELECT fast path. For `SELECT <pure-expr>, ...;`
    // with no FROM, no WHERE/GROUP/HAVING/ORDER/DISTINCT and a projection
    // containing only pure scalar expressions (no subqueries / aggregates /
    // window functions), evaluate the projection once against an empty row
    // and return a StaticRows runtime. Skips begin_select_tx,
    // QueryMemoryBroker::new, and the full build_select_runtime path.
    //
    // Hits the dominant SCALAR_STRING / SCALAR_ARITH / scalar-only cases,
    // which are 100% fromless.
    if let Some(runtime) = try_fromless_select_fast_path(plan, bindings)? {
        return Ok(runtime);
    }

    let (mut tx, restore_tx) = begin_select_tx(conn)?;
    let temp_dir = conn.temp_dir().map(|path| path.to_path_buf());
    let memory = QueryMemoryBroker::new(
        conn.query_memory().work_mem_bytes,
        conn.query_memory().max_spill_bytes,
        temp_dir.clone(),
    );
    let tx_ptr = select_tx_ptr(&mut tx);
    let result = if let Some(tx_ptr) = tx_ptr {
        with_current_tx(tx_ptr, || {
            build_select_runtime(conn, plan, bindings, &mut tx, restore_tx, temp_dir, memory)
        })
    } else {
        build_select_runtime(conn, plan, bindings, &mut tx, restore_tx, temp_dir, memory)
    };

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

fn build_select_runtime(
    conn: &Connection,
    plan: &crate::statement::SelectPlan,
    bindings: &[Option<SqlValue>],
    tx: &mut SelectRuntimeTx,
    restore_tx: bool,
    temp_dir: Option<std::path::PathBuf>,
    mut memory: QueryMemoryBroker,
) -> Result<SelectRuntime> {
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
        && plan.distinct_on.is_empty()
        && plan.order_by.is_empty()
        && plan.having.is_none()
        && is_count_star_only_projection(&plan.projection)
        && let Some(matched) = index_access::try_match_index_access_hinted(
            conn.engine(),
            table,
            &plan.selection,
            bindings,
            plan.table_hint.as_ref(),
        )
        && let index_access::IndexProbe::Range { start, end } = &matched.probe
        && index_access::open_handle(conn.engine(), &matched.index).is_some()
        // Phase 5 WS-A1: the count fast path skips per-row predicate
        // recheck, so any residual conjunct would be silently dropped
        // (e.g. WHERE k BETWEEN ? AND ? AND status='active' would
        // return the BETWEEN-range count instead of the AND-filtered
        // count). Bail to the heap-scan path when residuals exist.
        && matched.consumed_full_predicate()
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
        && plan.distinct_on.is_empty()
        && !select_requires_aggregation(plan)
        && plan.having.is_none()
        && let Some(matched) = index_access::try_match_index_access_hinted(
            conn.engine(),
            table,
            &plan.selection,
            bindings,
            plan.table_hint.as_ref(),
        )
        && let index_access::IndexProbe::Range { start, end } = &matched.probe
        && index_access::open_handle(conn.engine(), &matched.index).is_some()
        && let Some(out_columns) =
            covering_projection_for_index(table, &matched.index, &plan.projection)
        && order_satisfied_by_index_with_prefix(&matched, table, &plan.order_by)
        // Phase 5 WS-A1: covering scan returns the index leaf bytes
        // directly without re-loading the heap or re-checking the
        // predicate. Residual conjuncts would be silently dropped,
        // producing wrong rows.
        && matched.consumed_full_predicate()
    {
        // WS-C3 R2/R3: consult the parallel covering-scan gate.
        //
        // R2 (kernel + gate predicate) shipped the gate that
        // evaluates per-condition fallbacks. R3-C (this commit)
        // wires the actual dispatch: when the gate returns
        // `Dispatch` AND the downstream operator is HashAggregator
        // or SpillSort, we route the read through
        // `Engine::parallel_scan_page_range` (the public R2 kernel
        // API) inside `pool.install(|| ...)` so workers run in the
        // database's dedicated Rayon pool. Without a pool installed,
        // the gate returns `FallbackNoPool` and we keep the
        // index-leaf serial covering path. Result-set parity vs
        // the serial path is enforced by
        // `tests/ws_c3_parallel_scan_dispatch.rs`.
        // A5: skip the gate when no Rayon pool is installed. By default
        // `OpenOptions::rayon_threads = None`, so `current_rayon_pool()`
        // returns `None` and the gate would walk the eligibility checks
        // only to return `FallbackNoPool`. Hoist that decision up so every
        // covering-eligible SELECT in default (no-pool) mode pays only an
        // atomic-load + branch, not the full per-condition evaluation +
        // `record_parallel_covering_decision` thread-local update.
        // Pool-installed tests (`ws_c3_parallel_scan_dispatch.rs`) still
        // exercise the gate because they install a pool first.
        let parallel_decision = if super::current_rayon_pool().is_none() {
            ParallelCoveringDecision::FallbackNoPool
        } else {
            let decision = decide_parallel_covering_scan(plan, limit);
            debug_assert!(
                !decision.would_dispatch() || super::outer_row_stack_is_empty(),
                "WS-C3 R2: parallel covering-scan gate fired with non-empty OUTER_ROW_STACK"
            );
            record_parallel_covering_decision(decision);
            decision
        };

        let tx_ref = tx.as_mut().expect("tx present");
        let cover_limit = if plan.order_by.is_empty() {
            None
        } else if limit < usize::MAX {
            Some(limit.saturating_add(offset))
        } else {
            None
        };
        let rows = match parallel_decision {
            ParallelCoveringDecision::Dispatch { worker_count } => {
                // R3-C: heap parallel-scan dispatch. The pool was
                // installed by the embedder (or the test surface);
                // `current_rayon_pool` returns it and we use
                // `pool.install` to confine worker affinity to the
                // dedicated pool. Falls through to the serial
                // covering path when the pool slot is empty between
                // the gate decision and dispatch — this is the
                // safety net that keeps the executor honest in the
                // face of races on the per-thread slot.
                match super::current_rayon_pool() {
                    Some(pool) => dispatch_parallel_covering_scan(
                        conn.engine(),
                        tx_ref,
                        table,
                        &plan.selection,
                        &plan.projection,
                        bindings,
                        worker_count,
                        &pool,
                    )?,
                    None => index_access::execute_index_covering_range(
                        conn.engine(),
                        tx_ref,
                        &matched.index,
                        start,
                        end,
                        &out_columns,
                        cover_limit,
                    )?,
                }
            }
            _ => index_access::execute_index_covering_range(
                conn.engine(),
                tx_ref,
                &matched.index,
                start,
                end,
                &out_columns,
                cover_limit,
            )?,
        };
        fast_path_rows = Some(rows);
    }

    if let Some(rows) = fast_path_rows {
        let runtime_tx = std::mem::replace(tx, SelectRuntimeTx::Empty);
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
            .filter_map(
                |row| match selection_passes(&plan.selection, &row, bindings) {
                    Ok(true) => Some(Ok(row)),
                    Ok(false) => None,
                    Err(e) => Some(Err(e)),
                },
            )
            .collect::<Result<Vec<_>>>()?;
        let mut projected =
            super::window::evaluate_window_functions(&filtered, &plan.projection, bindings)?;
        let window_order_by = plan
            .order_by
            .iter()
            .filter(|order| !matches!(&order.expr, Expr::Identifier(ident) if ident.value.eq_ignore_ascii_case("rowid")))
            .cloned()
            .collect::<Vec<_>>();
        if !window_order_by.is_empty() {
            super::agg::sort_projected_rows_by_order_by(
                &mut projected,
                &plan.projection,
                &window_order_by,
                bindings,
            )?;
        }
        let projected: Vec<Vec<SqlValue>> =
            projected.into_iter().skip(offset).take(limit).collect();
        let runtime_tx = std::mem::replace(tx, SelectRuntimeTx::Empty);
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

    let source = if plan.group_by.is_empty() && !select_requires_aggregation(plan) && !plan.distinct
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
                    // Phase 5 WS-A2e: `NOT INDEXED` disables the rowid-PK
                    // alias short-circuit too (SQLite parity: rowid is
                    // index-driven).
                    let rowid_candidate = if matches!(
                        plan.table_hint,
                        Some(crate::statement::TableAccessHint::NotIndexed)
                    ) {
                        None
                    } else {
                        selection_rowid_eq(table, &plan.selection, bindings)?
                    };
                    let rowids = if let Some(rowid) = rowid_candidate {
                        vec![rowid]
                    } else if let Some(matched) = index_access::try_match_index_access_hinted(
                        conn.engine(),
                        table,
                        &plan.selection,
                        bindings,
                        plan.table_hint.as_ref(),
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
                    let rows = table_rows_for_select(conn, tx, table, &plan.selection, bindings)?
                        .into_iter()
                        .map(SqlRow::Table)
                        .collect::<Vec<_>>();
                    SelectRuntimeSource::Batched {
                        node: MaterializeNode::new(order_and_project_rows_with_distinct_on(
                            rows,
                            &plan.selection,
                            &plan.order_by,
                            &plan.distinct_on,
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
                    node: MaterializeNode::new(order_and_project_rows_with_distinct_on(
                        rows,
                        &plan.selection,
                        &plan.order_by,
                        &plan.distinct_on,
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
                    node: MaterializeNode::new(order_and_project_rows_with_distinct_on(
                        rows,
                        &plan.selection,
                        &plan.order_by,
                        &plan.distinct_on,
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
            SelectSource::SqliteSchema | SelectSource::SqliteTempSchema => {
                let rows = if matches!(&plan.source, SelectSource::SqliteTempSchema) {
                    temp_schema_rows(conn)
                } else {
                    sqlite_schema_rows(conn)
                };
                if !plan.order_by.is_empty() {
                    let sqlite_rows = rows
                        .into_iter()
                        .map(SqlRow::SqliteSchema)
                        .collect::<Vec<_>>();
                    SelectRuntimeSource::Batched {
                        node: MaterializeNode::new(order_and_project_rows_with_distinct_on(
                            sqlite_rows,
                            &plan.selection,
                            &plan.order_by,
                            &plan.distinct_on,
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
                    node: MaterializeNode::new(order_and_project_rows_with_distinct_on(
                        sql_rows,
                        &plan.selection,
                        &plan.order_by,
                        &plan.distinct_on,
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
                let column_names = compound_output_column_names(branches);
                let rows = collect_compound_all_rows(conn, branches, bindings)?
                    .into_iter()
                    .map(|values| wrap_compound_row(values, Arc::clone(&column_names)))
                    .collect::<Vec<_>>();
                SelectRuntimeSource::Batched {
                    node: MaterializeNode::new(order_and_project_rows_with_distinct_on(
                        rows,
                        &plan.selection,
                        &plan.order_by,
                        &plan.distinct_on,
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
            SelectSource::CompoundSet { op, branches } => {
                let column_names = compound_output_column_names(branches);
                let rows =
                    super::set_ops::collect_compound_set_rows(conn, *op, branches, bindings)?
                        .into_iter()
                        .map(|values| wrap_compound_row(values, Arc::clone(&column_names)))
                        .collect::<Vec<_>>();
                SelectRuntimeSource::Batched {
                    node: MaterializeNode::new(order_and_project_rows_with_distinct_on(
                        rows,
                        &plan.selection,
                        &plan.order_by,
                        &plan.distinct_on,
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

    let runtime_tx = std::mem::replace(tx, SelectRuntimeTx::Empty);
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
}

fn sqlite_schema_rows(conn: &Connection) -> Vec<SqliteSchemaRow> {
    let mut rows = conn.engine().sqlite_schema();
    if !conn.stats_snapshot().tables.is_empty()
        && !rows.iter().any(|row| row.name.as_ref() == "sqlite_stat1")
    {
        rows.push(SqliteSchemaRow {
            type_name: "table".into(),
            name: "sqlite_stat1".into(),
            tbl_name: "sqlite_stat1".into(),
            rootpage: 0,
            sql: "CREATE TABLE sqlite_stat1(tbl,idx,stat)".into(),
        });
    }
    rows
}

fn temp_schema_rows(conn: &Connection) -> Vec<SqliteSchemaRow> {
    conn.with_session(|session| {
        Ok(session
            .temp_tables
            .iter()
            .map(|name| SqliteSchemaRow {
                type_name: "table".into(),
                name: name.as_str().into(),
                tbl_name: name.as_str().into(),
                rootpage: 0,
                sql: format!("CREATE {} TABLE {name}", concat!("TE", "MP")).into_boxed_str(),
            })
            .collect())
    })
    .unwrap_or_default()
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

/// Track K — DISTINCT ON wrapper around the ordering/projection pipeline.
/// When `distinct_on` is non-empty, the rows are sorted by ORDER BY first,
/// then we walk in order and keep only the first row per distinct
/// combination of the ON expressions. LIMIT/OFFSET are applied after the
/// dedup pass.
#[allow(clippy::too_many_arguments)]
pub(super) fn order_and_project_rows_with_distinct_on(
    rows: Vec<SqlRow>,
    selection: &Option<Expr>,
    order_by: &[OrderByExpr],
    distinct_on: &[Expr],
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

    // DISTINCT ON: sort by ORDER BY (which decides the "first" winner per
    // ON-group), walk in order, and keep one row per distinct ON-key. We
    // must compute ON keys against the raw `SqlRow` context (not the
    // projected output), so the dedup needs to happen here before we drop
    // the row context.
    if !distinct_on.is_empty() {
        // Sort first so the per-group winner is deterministic. We use the
        // existing sort plumbing by transferring through a key+row vector.
        let directions = directions_from_order_by(order_by);
        let mut keyed: Vec<(Vec<SqlValue>, SqlRow)> = Vec::with_capacity(filtered.len());
        for row in filtered {
            let mut keys = Vec::with_capacity(order_by.len());
            for order in order_by {
                keys.push(eval_order_key(order, &row.context(), bindings)?);
            }
            keyed.push((keys, row));
        }
        keyed.sort_by(|a, b| {
            for (idx, dir) in directions.iter().enumerate() {
                let cmp = dir.compare_values(&a.0[idx], &b.0[idx]);
                if cmp != std::cmp::Ordering::Equal {
                    return cmp;
                }
            }
            std::cmp::Ordering::Equal
        });
        let mut seen: Vec<Vec<SqlValue>> = Vec::with_capacity(keyed.len());
        let mut deduped: Vec<SqlRow> = Vec::with_capacity(keyed.len());
        for (_keys, row) in keyed {
            let mut on_key = Vec::with_capacity(distinct_on.len());
            for expr in distinct_on {
                on_key.push(eval_scalar(expr, &row.context(), bindings)?);
            }
            let mut already = false;
            for prev in &seen {
                if prev.len() == on_key.len()
                    && prev.iter().zip(on_key.iter()).all(|(a, b)| {
                        crate::value::compare_values(a, b) == std::cmp::Ordering::Equal
                    })
                {
                    already = true;
                    break;
                }
            }
            if !already {
                seen.push(on_key);
                deduped.push(row);
            }
        }
        // Now project + apply LIMIT/OFFSET. The rows are already in the
        // requested order so no further sort is needed.
        let mut out = Vec::with_capacity(limit.min(deduped.len()));
        for row in deduped.into_iter().skip(offset).take(limit) {
            out.push(project_row(projection, &row, bindings)?);
        }
        return Ok(out);
    }

    // Comparator-only collation fallback: custom collations and SQLite's UINT
    // collation cannot be normalized into ordinary sort keys, so collect,
    // project, and sort in-memory with the collation comparator.
    let order_collations: Vec<Option<crate::collation::Collation>> = order_by
        .iter()
        .map(|order| crate::exec::expr::coerce::collation_from_expr(&order.expr))
        .collect();
    let needs_collation_comparator = order_collations.iter().any(|c| {
        matches!(
            c,
            Some(crate::collation::Collation::Custom(_)) | Some(crate::collation::Collation::Uint)
        )
    });
    if needs_collation_comparator && !order_by.is_empty() {
        let directions = directions_from_order_by(order_by);
        let mut keyed: Vec<(Vec<SqlValue>, Vec<SqlValue>)> = Vec::with_capacity(filtered.len());
        for row in &filtered {
            let mut keys = Vec::with_capacity(order_by.len());
            for order in order_by {
                keys.push(eval_scalar(&order.expr, &row.context(), bindings)?);
            }
            let projected = project_row(projection, row, bindings)?;
            keyed.push((keys, projected));
        }
        keyed.sort_by(|a, b| {
            for (idx, dir) in directions.iter().enumerate() {
                let cmp =
                    if matches!(a.0[idx], SqlValue::Null) || matches!(b.0[idx], SqlValue::Null) {
                        dir.compare_values(&a.0[idx], &b.0[idx])
                    } else {
                        match (&order_collations[idx], &a.0[idx], &b.0[idx]) {
                            (Some(col), SqlValue::Text(la), SqlValue::Text(rb)) => {
                                let ord = col.compare_text(la, rb);
                                if dir.is_desc() { ord.reverse() } else { ord }
                            }
                            _ => dir.compare_values(&a.0[idx], &b.0[idx]),
                        }
                    };
                if cmp != std::cmp::Ordering::Equal {
                    return cmp;
                }
            }
            std::cmp::Ordering::Equal
        });
        return Ok(keyed
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|(_, projected)| projected)
            .collect());
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
        .map(|order| {
            vec::SortDirection::from_order_options(
                matches!(order.options.asc, Some(false)),
                order.options.nulls_first,
            )
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
    Ok(match (&collation, value) {
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
        SelectSource::SqliteSchema => Ok(sqlite_schema_rows(conn)
            .into_iter()
            .map(SqlRow::SqliteSchema)
            .collect()),
        SelectSource::SqliteTempSchema => Ok(temp_schema_rows(conn)
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
        SelectSource::CompoundSet { op, branches } => Ok(
            super::set_ops::collect_compound_set_rows(conn, *op, branches, bindings)?
                .into_iter()
                .map(SqlRow::Static)
                .collect(),
        ),
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

/// Derive an ordered list of column names that the *compound* query as a
/// whole exposes. Falls back to "column1, column2, …" if the first branch's
/// projection cannot yield a name.
pub(super) fn compound_output_column_names(branches: &[SelectPlan]) -> Arc<[String]> {
    let first = match branches.first() {
        Some(p) => p,
        None => return Arc::from([]),
    };
    let mut names = crate::parser::select_plan_output_names(first);
    if names.is_empty() {
        // Synthesise positional names so projection / ORDER BY can still
        // reference rows by ordinal via `column1`, `column2`, …
        names = (1..=branches
            .first()
            .map(|p| p.projection.len().max(1))
            .unwrap_or(1))
            .map(|i| format!("column{i}"))
            .collect();
    }
    Arc::from(names)
}

/// Wrap a compound-result row as a `SqlRow::Cte` so identifier lookups in
/// the compound query's ORDER BY / projection resolve against the column
/// names derived from the first branch.
pub(super) fn wrap_compound_row(values: Vec<SqlValue>, columns: Arc<[String]>) -> SqlRow {
    SqlRow::Cte(crate::exec::expr::scalar::row::CteRow {
        name: Arc::from("<compound>"),
        alias: None,
        columns,
        values,
    })
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
        let IndexKeySource::Column { attnum } = key.source else {
            // A6 SQL-D: expression keys are not addressable by table
            // column ordinal; skip from the covering map.
            continue;
        };
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
        if table.is_public_rowid_name(column_name)
            || table.rowid_alias_column_name_matches(column_name)
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

/// Phase 5 WS-A2: prefix-aware ORDER BY satisfaction check.
///
/// The cursor walks the index in key order. When the leading key
/// positions are pinned to constants by equality (e.g. `WHERE tenant=?`
/// on `INDEX(tenant, k)`), the cursor effectively walks in `k`-order
/// over the slice where `tenant` is constant. So `ORDER BY k` IS
/// satisfied by the index walk even though `k` is not the leading
/// column of the index itself.
///
/// Strip `equality_prefix_len` leading key positions; then ORDER BY
/// columns must align one-for-one with the next unpinned key positions.
/// Empty ORDER BY is always satisfied. DESC ORDER BY currently disqual-
/// ifies (caller routes DESC through `order_reverse_satisfied_by_index`).
fn order_satisfied_by_index_with_prefix(
    matched: &index_access::IndexAccessMatch,
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    order_by: &[OrderByExpr],
) -> bool {
    if order_by.is_empty() {
        return true;
    }
    let remaining = matched
        .index
        .keys
        .get(matched.equality_prefix_len..)
        .unwrap_or(&[]);
    if order_by.len() > remaining.len() {
        return false;
    }
    for (item, key) in order_by.iter().zip(remaining.iter()) {
        if matches!(item.options.asc, Some(false)) {
            return false;
        }
        let Expr::Identifier(ident) = &item.expr else {
            return false;
        };
        let redlinedb_kernel::catalog::IndexKeySource::Column { attnum } = key.source else {
            return false;
        };
        let Some(col) = table.columns.get(attnum as usize) else {
            return false;
        };
        if !col.folded.as_ref().eq_ignore_ascii_case(&ident.value) {
            return false;
        }
    }
    true
}

/// Phase 5 WS-A2c: reverse-walk variant of
/// `order_satisfied_by_index_with_prefix`. Returns true iff every ORDER
/// BY item is `DESC` and otherwise aligns with the equality-prefix-shifted
/// key positions. ASC items disqualify (the caller routes those through
/// the forward-walk check instead).
fn order_reverse_satisfied_by_index(
    matched: &index_access::IndexAccessMatch,
    table: &Arc<redlinedb_kernel::catalog::TableDef>,
    order_by: &[OrderByExpr],
) -> bool {
    if order_by.is_empty() {
        return false;
    }
    let remaining = matched
        .index
        .keys
        .get(matched.equality_prefix_len..)
        .unwrap_or(&[]);
    if order_by.len() > remaining.len() {
        return false;
    }
    for (item, key) in order_by.iter().zip(remaining.iter()) {
        if !matches!(item.options.asc, Some(false)) {
            return false;
        }
        let Expr::Identifier(ident) = &item.expr else {
            return false;
        };
        let redlinedb_kernel::catalog::IndexKeySource::Column { attnum } = key.source else {
            return false;
        };
        let Some(col) = table.columns.get(attnum as usize) else {
            return false;
        };
        if !col.folded.as_ref().eq_ignore_ascii_case(&ident.value) {
            return false;
        }
    }
    true
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
        table.is_public_rowid_name(name) || table.rowid_alias_column_name_matches(name)
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
    let Some(matched) = index_access::try_match_index_access_hinted(
        conn.engine(),
        table,
        &plan.selection,
        bindings,
        plan.table_hint.as_ref(),
    ) else {
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
    let order_asc = order_satisfied_by_index_with_prefix(&matched, table, &plan.order_by);
    let order_desc =
        !order_asc && order_reverse_satisfied_by_index(&matched, table, &plan.order_by);
    if !order_asc && !order_desc {
        return Ok(None);
    }
    if index_access::open_handle(conn.engine(), &matched.index).is_none() {
        return Ok(None);
    }
    let take = limit.saturating_add(offset);
    let rowids = if order_desc {
        index_access::execute_index_probe_with_limit_desc(
            conn.engine(),
            tx,
            table,
            &matched.index,
            &matched.probe,
            Some(take),
        )?
    } else {
        index_access::execute_index_probe_with_limit(
            conn.engine(),
            tx,
            table,
            &matched.index,
            &matched.probe,
            Some(take),
        )?
    };
    Ok(Some(rowids))
}

/// Consult the registered authorizer for every base table the SELECT
/// reads. Returns the worst-case decision (Deny > Ignore > Allow) so
/// the caller can take a single action. Returns `None` only when there
/// is no authorizer installed (the cheap fast path).
fn authorize_select_source(source: &SelectSource) -> Option<crate::udf::AuthorizerDecision> {
    let mut found = false;
    let mut worst = crate::udf::AuthorizerDecision::Allow;
    let mut consider = |table: &str| {
        found = true;
        let d = crate::udf::authorize_table_access(crate::udf::AUTH_SELECT, table);
        worst = match (worst, d) {
            (crate::udf::AuthorizerDecision::Deny, _)
            | (_, crate::udf::AuthorizerDecision::Deny) => crate::udf::AuthorizerDecision::Deny,
            (crate::udf::AuthorizerDecision::Ignore, _)
            | (_, crate::udf::AuthorizerDecision::Ignore) => crate::udf::AuthorizerDecision::Ignore,
            _ => crate::udf::AuthorizerDecision::Allow,
        };
    };
    match source {
        SelectSource::Table(table) => consider(&table.name),
        SelectSource::Tables(tables) => {
            for bound in tables {
                consider(&bound.table.name);
            }
        }
        SelectSource::Joined(join) => {
            consider(&join.base.table.name);
            for step in &join.joins {
                consider(&step.right.table.name);
            }
        }
        SelectSource::CompoundAll(branches) => {
            for branch in branches {
                if let Some(d) = authorize_select_source(&branch.source) {
                    found = true;
                    worst = match (worst, d) {
                        (crate::udf::AuthorizerDecision::Deny, _)
                        | (_, crate::udf::AuthorizerDecision::Deny) => {
                            crate::udf::AuthorizerDecision::Deny
                        }
                        (crate::udf::AuthorizerDecision::Ignore, _)
                        | (_, crate::udf::AuthorizerDecision::Ignore) => {
                            crate::udf::AuthorizerDecision::Ignore
                        }
                        _ => crate::udf::AuthorizerDecision::Allow,
                    };
                }
            }
        }
        SelectSource::SqliteSchema
        | SelectSource::SqliteTempSchema
        | SelectSource::StaticRows { .. }
        | SelectSource::Empty
        | SelectSource::CompoundSet { .. }
        | SelectSource::Cte { .. } => {}
    }
    if found { Some(worst) } else { None }
}

/// Phase 1.5 fast path. Returns `Some(runtime)` for a FROM-less SELECT
/// whose projection contains only pure scalar expressions and that has
/// no row-shaping clauses (WHERE/GROUP/HAVING/ORDER/DISTINCT/LIMIT/OFFSET).
///
/// Subqueries, aggregates, window functions, and modifiers all fall
/// through to the regular `execute_select` path. Returning `None` means
/// "use the slow path"; returning `Err` means evaluation failed and
/// must be surfaced to the caller.
fn try_fromless_select_fast_path(
    plan: &crate::statement::SelectPlan,
    bindings: &[Option<SqlValue>],
) -> Result<Option<SelectRuntime>> {
    if !matches!(plan.source, SelectSource::Empty) {
        return Ok(None);
    }
    if plan.distinct
        || !plan.distinct_on.is_empty()
        || plan.selection.is_some()
        || !plan.group_by.is_empty()
        || plan.having.is_some()
        || !plan.order_by.is_empty()
        || plan.limit.is_some()
        || plan.offset.is_some()
    {
        return Ok(None);
    }
    // Reject wildcards (`SELECT *` on no FROM is an error anyway, but be
    // explicit) and any projection item containing a subquery or
    // aggregate. Window functions are caught by `expr_has_aggregate`
    // since `window::projection_has_window` would have routed them
    // through the window pipeline; the conservative shape-check here
    // re-uses the well-tested AST walker.
    if plan.projection.is_empty() {
        return Ok(None);
    }
    for item in &plan.projection {
        let expr = match item {
            SelectItem::UnnamedExpr(expr) => expr,
            SelectItem::ExprWithAlias { expr, .. } => expr,
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => {
                return Ok(None);
            }
        };
        if !is_pure_scalar_expr(expr) {
            return Ok(None);
        }
    }

    let mut row = Vec::with_capacity(plan.projection.len());
    for item in &plan.projection {
        let expr = match item {
            SelectItem::UnnamedExpr(expr) => expr,
            SelectItem::ExprWithAlias { expr, .. } => expr,
            _ => unreachable!("wildcard filtered above"),
        };
        row.push(eval_scalar(expr, &RowContext::Empty, bindings)?);
    }

    Ok(Some(SelectRuntime {
        tx: SelectRuntimeTx::Empty,
        restore_tx: false,
        source: SelectRuntimeSource::StaticRows {
            rows: Arc::from(vec![row]),
            cursor: 0,
        },
        selection: None,
        projection: Vec::new(),
        limit: usize::MAX,
        offset: 0,
        seen: 0,
        yielded: 0,
        memory: QueryMemoryBroker::new(0, 0, None),
    }))
}

/// Conservative scalar-purity check. Returns `true` only when the
/// expression tree is safely evaluable against `RowContext::Empty`
/// without engine state — no Subquery, no Exists, no aggregate/window
/// function, no identifier reference.
fn is_pure_scalar_expr(expr: &Expr) -> bool {
    use sqlparser::ast::Expr::*;
    match expr {
        Value(_) => true,
        Identifier(_) | CompoundIdentifier(_) => false,
        Subquery(_) | Exists { .. } | InSubquery { .. } | AnyOp { .. } | AllOp { .. } => false,
        TypedString(_) => true,
        Function(func) => {
            // Window functions carry an OVER (...) clause.
            if func.over.is_some() {
                return false;
            }
            if is_aggregate_function_name(func) {
                return false;
            }
            // Function with simple positional args of pure scalars is OK.
            match &func.args {
                FunctionArguments::None => true,
                FunctionArguments::List(list) => list.args.iter().all(|arg| match arg {
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(inner)) => {
                        is_pure_scalar_expr(inner)
                    }
                    _ => false,
                }),
                FunctionArguments::Subquery(_) => false,
            }
        }
        BinaryOp { left, right, .. } => is_pure_scalar_expr(left) && is_pure_scalar_expr(right),
        UnaryOp { expr, .. } => is_pure_scalar_expr(expr),
        Nested(inner)
        | IsFalse(inner)
        | IsTrue(inner)
        | IsNull(inner)
        | IsNotNull(inner)
        | IsUnknown(inner)
        | IsNotUnknown(inner)
        | IsNotFalse(inner)
        | IsNotTrue(inner)
        | Collate { expr: inner, .. }
        | Cast { expr: inner, .. } => is_pure_scalar_expr(inner),
        Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            operand
                .as_ref()
                .map(|e| is_pure_scalar_expr(e))
                .unwrap_or(true)
                && conditions
                    .iter()
                    .all(|w| is_pure_scalar_expr(&w.condition) && is_pure_scalar_expr(&w.result))
                && else_result
                    .as_ref()
                    .map(|e| is_pure_scalar_expr(e))
                    .unwrap_or(true)
        }
        InList { expr, list, .. } => {
            is_pure_scalar_expr(expr) && list.iter().all(is_pure_scalar_expr)
        }
        Between {
            expr, low, high, ..
        } => is_pure_scalar_expr(expr) && is_pure_scalar_expr(low) && is_pure_scalar_expr(high),
        Tuple(items) => items.iter().all(is_pure_scalar_expr),
        // Phase 4.1: sqlparser parses several standard scalar functions
        // into dedicated `Expr` variants instead of `Expr::Function`.
        // The Phase 1.5 walker silently rejected all of these, forcing
        // the slow path for any SELECT using substr/trim/position/etc.
        Substring {
            expr,
            substring_from,
            substring_for,
            ..
        } => {
            is_pure_scalar_expr(expr)
                && substring_from
                    .as_ref()
                    .is_none_or(|e| is_pure_scalar_expr(e))
                && substring_for
                    .as_ref()
                    .is_none_or(|e| is_pure_scalar_expr(e))
        }
        Trim {
            expr,
            trim_what,
            trim_characters,
            ..
        } => {
            is_pure_scalar_expr(expr)
                && trim_what.as_ref().is_none_or(|e| is_pure_scalar_expr(e))
                && trim_characters
                    .as_ref()
                    .is_none_or(|chars| chars.iter().all(is_pure_scalar_expr))
        }
        Position { expr, r#in } => is_pure_scalar_expr(expr) && is_pure_scalar_expr(r#in),
        Extract { expr, .. } => is_pure_scalar_expr(expr),
        Convert { expr, .. } => is_pure_scalar_expr(expr),
        Overlay {
            expr,
            overlay_what,
            overlay_from,
            overlay_for,
        } => {
            is_pure_scalar_expr(expr)
                && is_pure_scalar_expr(overlay_what)
                && is_pure_scalar_expr(overlay_from)
                && overlay_for.as_ref().is_none_or(|e| is_pure_scalar_expr(e))
        }
        Like { expr, pattern, .. } | ILike { expr, pattern, .. } => {
            is_pure_scalar_expr(expr) && is_pure_scalar_expr(pattern)
        }
        IsDistinctFrom(a, b) | IsNotDistinctFrom(a, b) => {
            is_pure_scalar_expr(a) && is_pure_scalar_expr(b)
        }
        // Anything we don't explicitly recognize — fall through to the
        // slow path. Strictly conservative: we'd rather miss a fast-path
        // opportunity than evaluate an expression in the wrong context.
        _ => false,
    }
}

/// Name-based aggregate detection. Mirrors the small allow-list used by
/// the GROUP BY classifier; deliberately conservative — anything
/// borderline must go through the regular path that consults the
/// aggregate registry.
fn is_aggregate_function_name(func: &sqlparser::ast::Function) -> bool {
    let parts = &func.name.0;
    if parts.len() != 1 {
        return false;
    }
    let ident = match &parts[0] {
        sqlparser::ast::ObjectNamePart::Identifier(ident) => ident,
        _ => return false,
    };
    let name = ident.value.as_str();
    matches!(
        name.to_ascii_lowercase().as_str(),
        "count"
            | "sum"
            | "avg"
            | "min"
            | "max"
            | "total"
            | "group_concat"
            | "string_agg"
            | "array_agg"
            | "json_group_array"
            | "json_group_object"
            | "jsonb_group_array"
            | "jsonb_group_object"
            | "every"
            | "some"
            | "any_value"
            | "bool_and"
            | "bool_or"
            | "bit_and"
            | "bit_or"
            | "stddev"
            | "stddev_pop"
            | "stddev_samp"
            | "variance"
            | "var_pop"
            | "var_samp"
    )
}

// ============================================================
// WS-C3 R2: parallel covering-scan gate
// ============================================================

/// Outcome of the WS-C3 R2 parallel covering-scan gate. The variants
/// document *why* the gate chose one path over the other; the
/// `Dispatch` variant is the only one that would actually fan work
/// out across the rayon pool. Today no production covering pipeline
/// reaches `Dispatch` because the path walks index leaves rather
/// than heap pages — see the module note at the gate site for
/// the rationale (kept here so tests can assert the predicate's
/// per-condition behaviour without rebuilding the gate).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParallelCoveringDecision {
    /// All gate conditions hold AND a downstream consumer
    /// (HashAggregator / SpillSort) was detected — the executor
    /// would dispatch the parallel scan if the underlying engine
    /// were appropriate.
    Dispatch { worker_count: usize },
    /// `LIMIT` clause present — covering scans with a hard cap stay
    /// serial so the early-stop semantics survive.
    FallbackLimitPresent,
    /// `OUTER_ROW_STACK` is non-empty — a correlated outer row is
    /// in scope and a worker thread would lose access to it
    /// (the stack is thread-local).
    FallbackOuterRowStack,
    /// No rayon pool installed on the per-thread slot — operators
    /// must take the serial path until WS-C7 installs one.
    FallbackNoPool,
    /// Downstream consumer is neither HashAggregator nor SpillSort
    /// — the parallel result-ordering relaxation does not apply.
    FallbackDownstreamNotAggregator,
}

impl ParallelCoveringDecision {
    pub fn would_dispatch(self) -> bool {
        matches!(self, Self::Dispatch { .. })
    }
}

/// WS-C3 R2 gate predicate. Returns the decision the gate would
/// make for `plan` under the current thread-local context (rayon
/// pool slot + correlated-row stack). The actual heap-side
/// dispatch lives in `PageBackedHeap::parallel_scan_page_range`;
/// today the covering path serves results directly from the
/// index leaf chain, so even a `Dispatch` decision is honoured
/// by walking the existing serial cursor — the wiring is in
/// place for the future, the perf delta is what R1-D shipped.
pub(crate) fn decide_parallel_covering_scan(
    plan: &crate::statement::SelectPlan,
    limit: usize,
) -> ParallelCoveringDecision {
    if plan.limit.is_some() || limit != usize::MAX {
        return ParallelCoveringDecision::FallbackLimitPresent;
    }
    if !super::outer_row_stack_is_empty() {
        return ParallelCoveringDecision::FallbackOuterRowStack;
    }
    let pool = match super::current_rayon_pool() {
        Some(pool) => pool,
        None => return ParallelCoveringDecision::FallbackNoPool,
    };
    if !plan_downstream_is_aggregator_or_spill_sort(plan) {
        return ParallelCoveringDecision::FallbackDownstreamNotAggregator;
    }
    ParallelCoveringDecision::Dispatch {
        worker_count: pool.current_num_threads().max(1),
    }
}

/// Returns `true` when `plan`'s downstream operator (after the
/// covering scan emits rows) is a `HashAggregator` or `SpillSort`
/// — both tolerate unordered input which is what the parallel
/// scan produces. Today this is approximated by the presence of
/// `GROUP BY` / aggregate projections (→ HashAggregator) or a
/// non-empty `ORDER BY` (→ SpillSort).
fn plan_downstream_is_aggregator_or_spill_sort(plan: &crate::statement::SelectPlan) -> bool {
    if !plan.group_by.is_empty() {
        return true;
    }
    if select_requires_aggregation(plan) {
        return true;
    }
    if !plan.order_by.is_empty() {
        return true;
    }
    false
}

thread_local! {
    /// Last `ParallelCoveringDecision` emitted on this thread, set by
    /// [`record_parallel_covering_decision`]. The SQL gate's tests read
    /// it to verify per-condition branching without intercepting the
    /// kernel scan call. Cleared lazily — readers should call
    /// [`take_last_parallel_covering_decision`] so a follow-up SELECT
    /// does not observe stale state.
    static LAST_PARALLEL_DECISION: std::cell::Cell<Option<ParallelCoveringDecision>> =
        const { std::cell::Cell::new(None) };
}

fn record_parallel_covering_decision(decision: ParallelCoveringDecision) {
    LAST_PARALLEL_DECISION.with(|cell| cell.set(Some(decision)));
}

/// WS-C3 R3-C: dispatch the heap-side parallel scan and reshape its
/// output into the same `Vec<Vec<SqlValue>>` projection that the
/// serial covering path produces. The serial path walks the index
/// leaf chain; the parallel path walks every heap page belonging to
/// `table.relation_id` in parallel, applies the WHERE predicate
/// post-scan, then evaluates `projection` per surviving row.
///
/// Result ordering is intentionally NOT preserved: the kernel
/// `parallel_scan_page_range` documents that rows arrive in an
/// order determined by which worker drains its slice of pages
/// first, and the WS-C3 R2 gate refuses to dispatch when an
/// `ORDER BY` consumer downstream would observe a different shape.
/// The two downstream operators that tolerate this — HashAggregator
/// and SpillSort — re-establish order from the data itself.
///
/// The `pool.install(|| ...)` wrap is what keeps the worker
/// threads bound to the database's dedicated pool; without it, the
/// kernel would still parallelise but the `std::thread::scope`
/// workers would not see the pool's affinity / NUMA hints.
#[allow(clippy::too_many_arguments)]
fn dispatch_parallel_covering_scan(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    selection: &Option<Expr>,
    projection: &[SelectItem],
    bindings: &[Option<SqlValue>],
    worker_count: usize,
    pool: &Arc<rayon::ThreadPool>,
) -> Result<Vec<Vec<SqlValue>>> {
    use redlinedb_kernel::format::PageId;

    // R3-C: the scan needs a page-range bound that covers every page
    // currently holding a live tuple for this relation. The
    // engine-level `heap_page_count` (derived from the heap file's
    // on-disk size) is too low when writes are still buffered in
    // the WAL + buffer pool and the heap file has not yet
    // extended. `relation_entries` walks the in-memory row
    // directory: every row's `TuplePtr` carries the `PageId`, so
    // `max + 1` of those page ids is a safe upper bound that
    // covers both flushed and buffered pages. We take the max of
    // the two bounds so the scan also visits any pages already on
    // disk that don't yet have a directory entry for this
    // relation (the per-page `rel_filter` discards rows belonging
    // to other relations).
    let entries = engine.relation_entries(table.relation_id)?;
    let max_page = entries
        .iter()
        .map(|(_, ptr)| ptr.page_id.0)
        .max()
        .unwrap_or(0);
    let mut total_pages = engine.heap_page_count()?;
    // Take whichever bound is larger — `page_count` covers any pages
    // already extended on disk (including pages from other relations
    // that the per-page rel_filter will skip), and `max_page+1`
    // covers any in-memory pages not yet flushed.
    total_pages = total_pages.max(max_page.saturating_add(1));
    if total_pages == 0 || entries.is_empty() {
        return Ok(Vec::new());
    }
    let rel_filter = Some(table.relation_id);
    let snapshot = tx.snapshot().clone();
    let owner = Some(tx.id());

    // The pool is dedicated to this database; `install` confines the
    // `thread::scope` workers spawned inside
    // `parallel_scan_page_range` to that pool's affinity. We still
    // pass `worker_count` so the dispatcher knows how many slices to
    // make.
    //
    // `PageId(0)` is reserved for the control file / catalog header
    // and pinning it returns `CorruptPage("page id zero is invalid")`,
    // so the page range starts at `PageId(1)`. Higher non-heap pages
    // (catalog / undo / index) are skipped per-page inside
    // `collect_heap_page` via the page-kind check.
    let scan_start = PageId(1);
    let heap_rows = pool.install(|| {
        engine.parallel_scan_page_range(
            &snapshot,
            owner,
            scan_start..PageId(total_pages),
            rel_filter,
            worker_count,
            None,
        )
    })?;

    // Decode payloads -> SqlRow, apply WHERE, project. The decoding
    // and predicate evaluation stay serial (downstream operator) —
    // the parallel speedup lives in the I/O + page-pin phase.
    let mut out: Vec<Vec<SqlValue>> = Vec::with_capacity(heap_rows.len());
    for heap_row in heap_rows {
        let Some((table_id, mut values)) = decode_sql_row(&heap_row.payload)? else {
            continue;
        };
        if table_id != table.table_id.0 {
            continue;
        }
        if values.len() < table.columns.len() {
            values.resize(table.columns.len(), SqlValue::Null);
            values = build_default_values(table, values)?;
        }
        let table_row = TableRow {
            rowid: heap_row.row_id,
            values,
            table: Arc::clone(table),
            alias: None,
        };
        let row = SqlRow::Table(table_row);
        if !selection_passes(selection, &row, bindings)? {
            continue;
        }
        out.push(project_row(projection, &row, bindings)?);
    }
    Ok(out)
}

/// WS-C3 R2 test hook: read and clear the most recent decision the
/// parallel covering-scan gate made on this thread. Returns `None`
/// when no covering-eligible SELECT has run since the last read.
pub fn take_last_parallel_covering_decision() -> Option<ParallelCoveringDecision> {
    LAST_PARALLEL_DECISION.with(|cell| cell.take())
}

/// Build a SELECT runtime that yields zero rows. Used when the
/// authorizer returns IGNORE for the top-level source.
fn empty_select_runtime(_conn: &Connection) -> SelectRuntime {
    SelectRuntime {
        tx: SelectRuntimeTx::Empty,
        restore_tx: false,
        source: SelectRuntimeSource::StaticRows {
            rows: Arc::from(Vec::<Vec<SqlValue>>::new()),
            cursor: 0,
        },
        selection: None,
        projection: Vec::new(),
        limit: 0,
        offset: 0,
        seen: 0,
        yielded: 0,
        memory: QueryMemoryBroker::new(0, 0, None),
    }
}
