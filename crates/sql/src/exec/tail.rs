use super::*;

pub(crate) fn analyze_database(conn: &Connection, plan: &AnalyzePlan) -> Result<()> {
    let schema = conn.engine().schema_snapshot();
    let mut tx = conn.engine().begin(Isolation::Snapshot)?;
    let result = (|| -> Result<()> {
        let current = conn.stats_snapshot();
        let mut next = StatsSnapshot::empty(StatsEpoch(current.epoch.0.saturating_add(1)));
        next.tables = current.tables.clone();
        next.columns = current.columns.clone();
        next.indexes = current.indexes.clone();

        let tables = match &plan.table {
            Some(table) => vec![Arc::clone(table)],
            None => schema.tables.to_vec(),
        };

        for table in tables {
            let rows = collect_table_rows(conn.engine(), &mut tx, &table)?
                .into_iter()
                .map(|row| row.values)
                .collect::<Vec<_>>();
            let table_stats = build_table_stats(conn, &table, &rows)?;
            next.tables.insert(table.table_id, table_stats);

            let sample = sample_rows(conn.stats_config(), &rows);
            for (ordinal, column) in table.columns.iter().enumerate() {
                let stats = build_column_stats(conn.stats_config(), &sample, ordinal);
                next.columns
                    .insert((table.table_id, column.column_id), stats);
            }
            for index in &table.indexes {
                let stats = build_index_stats(conn.stats_config(), &sample, index);
                next.indexes.insert(index.index_id, stats);
            }
        }

        conn.publish_stats(Arc::new(next))
    })();
    let _ = conn.engine().rollback(tx);
    result
}

pub(crate) fn execute_explain(
    conn: &Connection,
    plan: &ExplainPlan,
    bindings: &[Option<SqlValue>],
) -> Result<SelectRuntime> {
    let temp_dir = conn.temp_dir().map(|path| path.to_path_buf());
    let rows = if plan.analyze {
        let start = Instant::now();
        let mut result = execute_prepared(conn, &plan.inner, bindings)?;
        let mut actual_rows = result.affected_rows;
        let mut loops = 0usize;
        if let RuntimeState::Select(runtime) = &mut result.runtime {
            let mut current_row = None;
            loop {
                loops += 1;
                if step_select_runtime(conn, runtime, bindings, &mut current_row)? {
                    break;
                }
                actual_rows += 1;
            }
        }
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        let (peak_memory_bytes, spill_bytes) = match &result.runtime {
            RuntimeState::Select(runtime) => {
                (runtime.memory.used_bytes, runtime.memory.spilled_bytes)
            }
            RuntimeState::Done | RuntimeState::Idle => (0, 0),
        };
        planner::explain_rows(
            conn,
            &plan.inner.kind,
            bindings,
            Some(ExplainMetrics {
                actual_rows: Some(actual_rows),
                loops: Some(loops),
                elapsed_ms: Some(elapsed_ms),
                peak_memory_bytes: Some(peak_memory_bytes),
                spill_bytes: Some(spill_bytes),
            }),
            plan.format,
        )
    } else {
        planner::explain_rows(conn, &plan.inner.kind, bindings, None, plan.format)
    };

    Ok(SelectRuntime {
        tx: SelectRuntimeTx::Empty,
        restore_tx: false,
        source: SelectRuntimeSource::Batched {
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
        },
        selection: None,
        projection: Vec::new(),
        limit: usize::MAX,
        offset: 0,
        seen: 0,
        yielded: 0,
        memory: QueryMemoryBroker::new(
            conn.query_memory().work_mem_bytes,
            conn.query_memory().max_spill_bytes,
            temp_dir.clone(),
        ),
    })
}

pub(crate) fn build_table_stats(
    conn: &Connection,
    table: &TableDef,
    rows: &[Vec<SqlValue>],
) -> Result<TableStats> {
    let current = conn.stats_snapshot();
    let preserved = current.tables.get(&table.table_id).cloned();
    let row_count = rows.len() as u64;
    let avg_row_bytes = if rows.is_empty() {
        0.0
    } else {
        rows.iter().map(|row| row_width(row)).sum::<usize>() as f64 / rows.len() as f64
    };
    Ok(TableStats {
        table_id: table.table_id,
        rel_id: table.relation_id,
        row_count,
        live_row_count: row_count,
        heap_pages: if row_count == 0 {
            0
        } else {
            row_count.div_ceil(64).max(1)
        },
        avg_row_bytes,
        analyzed_at_csn: preserved
            .as_ref()
            .map(|stats| stats.analyzed_at_csn)
            .unwrap_or(redlinedb_kernel::format::Csn::ZERO),
        data_change_count: preserved.map(|stats| stats.data_change_count).unwrap_or(0),
    })
}

pub(crate) fn build_column_stats(
    cfg: &crate::connection::StatsConfig,
    rows: &[Vec<SqlValue>],
    ordinal: usize,
) -> ColumnStats {
    if rows.is_empty() {
        return ColumnStats {
            null_frac: 1.0,
            ndv: 0.0,
            avg_width: 0.0,
            min: None,
            max: None,
            mcv: Vec::new(),
            histogram: Vec::new(),
        };
    }
    let mut nulls = 0usize;
    let mut widths = 0usize;
    let mut min: Option<SqlValue> = None;
    let mut max: Option<SqlValue> = None;
    let mut counts: HashMap<String, (usize, SqlValue)> = HashMap::new();
    let mut non_null_values = Vec::new();
    for row in rows {
        let value = row.get(ordinal).cloned().unwrap_or(SqlValue::Null);
        if matches!(value, SqlValue::Null) {
            nulls += 1;
            continue;
        }
        widths += row_width_value(&value);
        if min
            .as_ref()
            .map(|current| compare_values(&value, current) == Ordering::Less)
            .unwrap_or(true)
        {
            min = Some(value.clone());
        }
        if max
            .as_ref()
            .map(|current| compare_values(&value, current) == Ordering::Greater)
            .unwrap_or(true)
        {
            max = Some(value.clone());
        }
        non_null_values.push(value.clone());
        let key = stats_value_key(&value);
        let entry = counts.entry(key).or_insert((0, value));
        entry.0 += 1;
    }

    non_null_values.sort_by(compare_values);
    let ndv = counts.len() as f64;
    let mut mcv: Vec<_> = counts
        .into_iter()
        .map(|(_, (count, value))| MostCommonValue {
            value,
            frequency: count as f64 / rows.len() as f64,
        })
        .collect();
    mcv.sort_by(|left, right| {
        right
            .frequency
            .partial_cmp(&left.frequency)
            .unwrap_or(Ordering::Equal)
            .then_with(|| compare_values(&left.value, &right.value))
    });
    mcv.truncate(cfg.mcv_capacity);

    let histogram = build_histogram(cfg, &non_null_values, rows.len());
    ColumnStats {
        null_frac: nulls as f64 / rows.len() as f64,
        ndv,
        avg_width: if non_null_values.is_empty() {
            0.0
        } else {
            widths as f64 / non_null_values.len() as f64
        },
        min,
        max,
        mcv,
        histogram,
    }
}

pub(crate) fn build_index_stats(
    _cfg: &crate::connection::StatsConfig,
    rows: &[Vec<SqlValue>],
    index: &redlinedb_kernel::catalog::IndexDef,
) -> IndexStats {
    let mut distinct_prefix_counts = Vec::new();
    for prefix_len in 1..=index.keys.len() {
        let mut seen = std::collections::BTreeSet::new();
        for row in rows {
            let mut key = String::new();
            for key_def in index.keys.iter().take(prefix_len) {
                let value = row
                    .get(key_def.ordinal as usize)
                    .cloned()
                    .unwrap_or(SqlValue::Null);
                key.push_str(&stats_value_key(&value));
                key.push('|');
            }
            seen.insert(key);
        }
        distinct_prefix_counts.push(seen.len() as f64);
    }
    let avg_key_bytes = if rows.is_empty() {
        0.0
    } else {
        let total = rows
            .iter()
            .map(|row| {
                index
                    .keys
                    .iter()
                    .map(|key_def| {
                        row.get(key_def.ordinal as usize)
                            .map(row_width_value)
                            .unwrap_or(0)
                    })
                    .sum::<usize>()
            })
            .sum::<usize>();
        total as f64 / rows.len() as f64
    };
    IndexStats {
        index_id: index.index_id,
        entries: rows.len() as u64,
        leaf_pages: if rows.is_empty() {
            0
        } else {
            (rows.len() as u64).div_ceil(64).max(1)
        },
        height: if rows.is_empty() { 0 } else { 1 },
        distinct_prefix_counts,
        avg_key_bytes,
        clustering_factor: if rows.is_empty() { 0.0 } else { 1.0 },
    }
}

pub(crate) fn sample_rows(
    cfg: &crate::connection::StatsConfig,
    rows: &[Vec<SqlValue>],
) -> Vec<Vec<SqlValue>> {
    if rows.len() <= cfg.exact_analyze_row_threshold {
        return rows.to_vec();
    }
    let mut sample = rows
        .iter()
        .cloned()
        .map(|row| (stable_row_score(&row), row))
        .collect::<Vec<_>>();
    sample.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| compare_rows(&left.1, &right.1))
    });
    sample.truncate(cfg.sample_rows.min(sample.len()));
    sample.into_iter().map(|(_, row)| row).collect()
}

pub(crate) fn stable_row_score(row: &[SqlValue]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for value in row {
        stats_value_key(value).hash(&mut hasher);
    }
    hasher.finish()
}

pub(crate) fn build_histogram(
    cfg: &crate::connection::StatsConfig,
    values: &[SqlValue],
    total_rows: usize,
) -> Vec<HistogramBucket> {
    if values.is_empty() || cfg.histogram_buckets == 0 {
        return Vec::new();
    }
    let bucket_count = cfg.histogram_buckets.min(values.len()).max(1);
    let mut buckets = Vec::with_capacity(bucket_count);
    let chunk = values.len().div_ceil(bucket_count);
    let mut start = 0usize;
    while start < values.len() {
        let end = (start + chunk).min(values.len());
        buckets.push(HistogramBucket {
            lower: Some(values[start].clone()),
            upper: Some(values[end - 1].clone()),
            frequency: (end - start) as f64 / total_rows as f64,
        });
        start = end;
    }
    buckets
}

pub(crate) fn stats_value_key(value: &SqlValue) -> String {
    match value {
        SqlValue::Null => "n".to_owned(),
        SqlValue::Integer(v) => format!("i:{v}"),
        SqlValue::Real(v) => format!("r:{:016x}", v.to_bits()),
        SqlValue::Text(v) => format!("t:{v}"),
        SqlValue::Blob(v) => {
            let mut out = String::from("b:");
            for byte in v.iter() {
                use std::fmt::Write;
                let _ = write!(&mut out, "{byte:02x}");
            }
            out
        }
    }
}

pub(crate) fn execute_update(
    conn: &Connection,
    plan: &crate::statement::UpdatePlan,
    bindings: &[Option<SqlValue>],
) -> Result<ExecutionResult> {
    with_write_tx(conn, |session, tx| {
        let target_rowids =
            if let Some(rowid) = selection_rowid_eq(&plan.table, &plan.selection, bindings)? {
                vec![rowid]
            } else {
                dml_target_rows(conn, tx, &plan.table, &plan.selection, bindings)?
                    .into_iter()
                    .map(|row| row.rowid)
                    .collect()
            };
        let mut count = 0usize;
        let mut returning_rows = Vec::new();
        for rowid in target_rowids {
            // Lock before reloading/evaluating assignments. Autocommit
            // writes run at read-committed isolation, so expressions like
            // `version = version + 1` must be based on the latest tuple
            // after the hot-row handoff, not on the pre-lock probe row.
            conn.engine()
                .lock_row_for_relation(tx, plan.table.relation_id, rowid)?;
            let Some(fresh) = load_table_row_by_rowid(conn.engine(), tx, &plan.table, rowid)?
            else {
                continue;
            };
            if !selection_passes(&plan.selection, &SqlRow::Table(fresh.clone()), bindings)? {
                continue;
            }
            let old_values = fresh.values.clone();
            let mut values = fresh.values.clone();
            for (ordinal, expr) in &plan.assignments {
                if *ordinal >= values.len() {
                    return Err(Error::UnknownColumn(format!("ordinal {ordinal}")));
                }
                values[*ordinal] = eval_scalar(expr, &RowContext::Table(&fresh), bindings)?;
            }
            values = apply_row_affinity(&plan.table, values)?;
            let new_rowid =
                choose_rowid_for_update(conn.engine(), &plan.table, &values, fresh.rowid)?;
            if let Some(alias) = plan.table.rowid_alias_column
                && let Some(slot) = values.get_mut(alias as usize)
                && matches!(slot, SqlValue::Null)
            {
                *slot = SqlValue::Integer(new_rowid.0 as i64);
            }
            apply_constraints(&plan.table, &values)?;
            ensure_unique_constraints(conn, session, tx, &plan.table, &values, Some(fresh.rowid))?;
            let payload = encode_sql_row(plan.table.table_id.0, &values)?;
            if new_rowid == fresh.rowid {
                conn.engine().update_for_relation(
                    tx,
                    plan.table.relation_id,
                    fresh.rowid,
                    payload,
                )?;
            } else {
                conn.engine()
                    .delete_for_relation(tx, plan.table.relation_id, fresh.rowid)?;
                conn.engine().insert_for_relation(
                    tx,
                    plan.table.relation_id,
                    new_rowid,
                    payload,
                )?;
            }
            crate::exec::index_dml::maintain_indexes_on_update(
                conn.engine(),
                tx,
                &plan.table,
                &old_values,
                &values,
                fresh.rowid,
                new_rowid,
            )?;
            if let Some(returning) = &plan.returning {
                returning_rows.push(project_returning_row(
                    &plan.table,
                    &values,
                    new_rowid,
                    returning,
                    bindings,
                )?);
            }
            count += 1;
        }
        Ok(build_dml_execution_result(
            count,
            returning_rows,
            plan.returning.is_some(),
        ))
    })
}

pub(crate) fn execute_delete(
    conn: &Connection,
    plan: &crate::statement::DeletePlan,
    bindings: &[Option<SqlValue>],
) -> Result<ExecutionResult> {
    with_write_tx(conn, |_session, tx| {
        let rows = dml_target_rows(conn, tx, &plan.table, &plan.selection, bindings)?;
        let mut count = 0usize;
        let mut returning_rows = Vec::new();
        for row in rows {
            if !selection_passes(&plan.selection, &SqlRow::Table(row.clone()), bindings)? {
                continue;
            }
            if let Some(returning) = &plan.returning {
                returning_rows.push(project_returning_row(
                    &plan.table,
                    &row.values,
                    row.rowid,
                    returning,
                    bindings,
                )?);
            }
            // Reload the row to make sure we delete-mark the right index
            // entries; the heap state may have moved since plan time.
            let live = load_table_row_by_rowid(conn.engine(), tx, &plan.table, row.rowid)?
                .map(|fresh| fresh.values)
                .unwrap_or_else(|| row.values.clone());
            conn.engine()
                .delete_for_relation(tx, plan.table.relation_id, row.rowid)?;
            crate::exec::index_dml::maintain_indexes_on_delete(
                conn.engine(),
                tx,
                &plan.table,
                &live,
                row.rowid,
            )?;
            count += 1;
        }
        Ok(build_dml_execution_result(
            count,
            returning_rows,
            plan.returning.is_some(),
        ))
    })
}

fn dml_target_rows(
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
        crate::exec::index_access::try_match_index_access(conn.engine(), table, selection, bindings)
        && crate::exec::index_access::open_handle(conn.engine(), &matched.index).is_some()
    {
        let rowids = crate::exec::index_access::execute_index_probe(
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

pub(super) fn project_returning_row(
    table: &Arc<TableDef>,
    values: &[SqlValue],
    rowid: RowId,
    returning: &[SelectItem],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlValue>> {
    let row = TableRow {
        rowid,
        values: values.to_vec(),
        table: Arc::clone(table),
        alias: None,
    };
    project_row(returning, &SqlRow::Table(row), bindings)
}

pub(super) fn build_dml_execution_result(
    affected_rows: usize,
    returning_rows: Vec<Vec<SqlValue>>,
    has_returning: bool,
) -> ExecutionResult {
    if has_returning {
        ExecutionResult {
            runtime: returning_rows.into_returning_runtime(),
            affected_rows,
        }
    } else {
        ExecutionResult {
            runtime: RuntimeState::Done,
            affected_rows,
        }
    }
}

trait ReturningRuntimeExt {
    fn into_returning_runtime(self) -> RuntimeState;
}

impl ReturningRuntimeExt for Vec<Vec<SqlValue>> {
    fn into_returning_runtime(self) -> RuntimeState {
        RuntimeState::Select(SelectRuntime {
            tx: SelectRuntimeTx::Empty,
            restore_tx: false,
            source: SelectRuntimeSource::StaticRows {
                rows: Arc::from(self),
                cursor: 0,
            },
            selection: None,
            projection: Vec::new(),
            limit: usize::MAX,
            offset: 0,
            seen: 0,
            yielded: 0,
            memory: QueryMemoryBroker::new(0, 0, None),
        })
    }
}

pub(crate) fn build_row(
    table: &Arc<TableDef>,
    row: &[Expr],
    columns: &[usize],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlValue>> {
    let mut values = vec![SqlValue::Null; table.columns.len()];
    let mut provided = vec![false; table.columns.len()];
    for (ordinal, expr) in columns.iter().copied().zip(row.iter()) {
        values[ordinal] = eval_scalar(expr, &RowContext::Empty, bindings)?;
        provided[ordinal] = true;
    }
    build_default_values_for_omitted(table, values, &provided)
}

pub(crate) fn build_row_from_values(
    table: &Arc<TableDef>,
    row: &[SqlValue],
    columns: &[usize],
) -> Result<Vec<SqlValue>> {
    let mut values = vec![SqlValue::Null; table.columns.len()];
    let mut provided = vec![false; table.columns.len()];
    for (ordinal, value) in columns.iter().copied().zip(row.iter()) {
        values[ordinal] = value.clone();
        provided[ordinal] = true;
    }
    build_default_values_for_omitted(table, values, &provided)
}

pub(crate) fn build_default_row(table: &Arc<TableDef>) -> Result<Vec<SqlValue>> {
    build_default_values(table, vec![SqlValue::Null; table.columns.len()])
}

pub(crate) fn build_default_values(
    table: &Arc<TableDef>,
    mut values: Vec<SqlValue>,
) -> Result<Vec<SqlValue>> {
    for (idx, column) in table.columns.iter().enumerate() {
        if matches!(values[idx], SqlValue::Null)
            && let Some(default) = &column.default_value
        {
            values[idx] = default.clone();
        }
    }
    apply_row_affinity(table, values)
}

fn build_default_values_for_omitted(
    table: &Arc<TableDef>,
    mut values: Vec<SqlValue>,
    provided: &[bool],
) -> Result<Vec<SqlValue>> {
    for (idx, column) in table.columns.iter().enumerate() {
        if !provided.get(idx).copied().unwrap_or(false)
            && matches!(values[idx], SqlValue::Null)
            && let Some(default) = &column.default_value
        {
            values[idx] = default.clone();
        }
    }
    apply_row_affinity(table, values)
}

pub(crate) fn apply_row_affinity(table: &TableDef, values: Vec<SqlValue>) -> Result<Vec<SqlValue>> {
    let mut out = values;
    for (idx, column) in table.columns.iter().enumerate() {
        out[idx] = apply_affinity(out[idx].clone(), column.affinity)
            .map_err(|_| Error::DatatypeMismatch)?;
    }
    Ok(out)
}

pub(crate) fn apply_constraints(table: &TableDef, values: &[SqlValue]) -> Result<()> {
    let mut scratch = EvalScratch::default();
    for (idx, column) in table.columns.iter().enumerate() {
        let value = values
            .get(idx)
            .ok_or_else(|| Error::UnknownColumn(column.name.to_string()))?;
        if column.not_null && matches!(value, SqlValue::Null) {
            return Err(Error::ConstraintViolation(format!(
                "NOT NULL constraint failed: {}.{}",
                table.name, column.name
            )));
        }
    }

    for check in &table.checks {
        let row = TableRowSource { values };
        let result = eval_expr(&check.expr, &row, &mut scratch).map_err(|_| {
            Error::ConstraintViolation(format!("CHECK constraint failed: {}", table.name))
        })?;
        if matches!(result, SqlValue::Null) || is_truthy(&result) {
            continue;
        }
        return Err(Error::ConstraintViolation(format!(
            "CHECK constraint failed: {}",
            table.name
        )));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct UniqueConflict {
    rowid: RowId,
    constraint_name: Option<Arc<str>>,
    key_ordinals: Vec<usize>,
}

struct UpsertUpdateContext<'a> {
    conn: &'a Connection,
    session: &'a mut SessionState,
    tx: &'a mut Txn,
    table: &'a Arc<TableDef>,
    excluded: &'a [SqlValue],
    conflict: &'a UniqueConflict,
    bindings: &'a [Option<SqlValue>],
}

#[derive(Debug)]
pub(super) enum InsertOutcome {
    Inserted { rowid: RowId, values: Vec<SqlValue> },
    Updated { rowid: RowId, values: Vec<SqlValue> },
    Ignored,
}

fn collect_unique_conflicts(
    conn: &Connection,
    session: &mut SessionState,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    values: &[SqlValue],
    skip_rowid: Option<RowId>,
) -> Result<Vec<UniqueConflict>> {
    // Lane B: the physical-index probe replaces the O(N) heap scan when the
    // index has been allocated by Lane A. The fallback path (no
    // `meta_page_id`) preserves the original O(N) behavior so legacy
    // databases without physical indexes still enforce UNIQUE.
    //
    // SQLite enforces UNIQUE on every unique index — both inline UNIQUE
    // constraints (which create a backing index AND a Constraint row) and
    // standalone `CREATE UNIQUE INDEX` statements (which only create the
    // index). We therefore enumerate `table.indexes` and only fall back to
    // the constraints list to recover the original constraint name when a
    // matching one exists.
    let mut conflicts = Vec::new();
    let mut legacy_indexes: Vec<&redlinedb_kernel::catalog::IndexDef> = Vec::new();
    for index in &table.indexes {
        if !index.unique && !index.primary {
            continue;
        }
        let constraint_name = table
            .constraints
            .iter()
            .find(|c| {
                (c.kind == ConstraintKind::Unique || c.kind == ConstraintKind::PrimaryKey)
                    && c.index_id == Some(index.index_id)
            })
            .and_then(|c| c.name.as_deref().map(Arc::<str>::from))
            .or_else(|| Some(Arc::from(index.name.as_ref())));
        // SQLite NULL parity: a NULL anywhere in the unique-key tuple
        // disables the conflict check entirely. We compute this once from
        // the SQL-side values so both index and legacy paths agree.
        let key_values: Vec<SqlValue> = index
            .keys
            .iter()
            .map(|key| {
                values
                    .get(key.ordinal as usize)
                    .cloned()
                    .unwrap_or(SqlValue::Null)
            })
            .collect();
        if key_values
            .iter()
            .any(|value| matches!(value, SqlValue::Null))
        {
            continue;
        }
        if let Some(handle) = crate::exec::index_dml::open_index_handle(conn.engine(), index) {
            let key = crate::exec::index_dml::build_index_key(index, values);
            let (kernel_guard, hit) = crate::exec::index_dml::probe_unique_for_conflict(
                conn.engine(),
                &handle,
                tx,
                skip_rowid,
                &key,
            )?;
            if let Some(rowid) = hit {
                conflicts.push(UniqueConflict {
                    rowid,
                    constraint_name: constraint_name.clone(),
                    key_ordinals: index.keys.iter().map(|key| key.ordinal as usize).collect(),
                });
            }
            // Hold the kernel `UniqueKeyGuard` until end-of-transaction so
            // the probe-to-insert window stays serialized — dropping the
            // guard between `point_lookup` and `insert_tx` reopens the race
            // where two writers both see "no duplicate" and both commit.
            // The guard lives in `SessionState::kernel_unique_guards` and
            // releases on commit/rollback when that vector is cleared.
            session.kernel_unique_guards.push(kernel_guard);

            // We still take a SQL-side guard so legacy callers that share
            // `unique_locks()` continue to serialize against this key. The
            // dual locking is harmless and matches the legacy fallback
            // below; the SQL guard is also released on commit/rollback.
            let sql_lock_key = unique_key_bytes(table.table_id.0, index.index_id.0, &key_values)?;
            let sql_guard = conn.unique_locks().lock(sql_lock_key, tx.id().0)?;
            session.unique_guards.push(sql_guard);
            continue;
        }
        legacy_indexes.push(index);
    }

    // Legacy O(N) heap scan for any indexes Lane A did not allocate yet.
    if legacy_indexes.is_empty() {
        return Ok(conflicts);
    }
    let rows = collect_table_rows(conn.engine(), tx, table)?;
    for index in legacy_indexes {
        let key_values: Vec<SqlValue> = index
            .keys
            .iter()
            .map(|key| {
                values
                    .get(key.ordinal as usize)
                    .cloned()
                    .unwrap_or(SqlValue::Null)
            })
            .collect();
        if key_values
            .iter()
            .any(|value| matches!(value, SqlValue::Null))
        {
            continue;
        }
        let key = unique_key_bytes(table.table_id.0, index.index_id.0, &key_values)?;
        let guard = conn.unique_locks().lock(key, tx.id().0)?;
        session.unique_guards.push(guard);
        for row in &rows {
            if skip_rowid == Some(row.rowid) {
                continue;
            }
            let other: Vec<SqlValue> = index
                .keys
                .iter()
                .map(|key| {
                    row.values
                        .get(key.ordinal as usize)
                        .cloned()
                        .unwrap_or(SqlValue::Null)
                })
                .collect();
            if key_values_equal(&key_values, &other) {
                let constraint_name = table
                    .constraints
                    .iter()
                    .find(|c| c.index_id == Some(index.index_id))
                    .and_then(|c| c.name.as_deref().map(Arc::<str>::from))
                    .or_else(|| Some(Arc::from(index.name.as_ref())));
                conflicts.push(UniqueConflict {
                    rowid: row.rowid,
                    constraint_name,
                    key_ordinals: index.keys.iter().map(|key| key.ordinal as usize).collect(),
                });
                break;
            }
        }
    }
    Ok(conflicts)
}

fn unique_conflict_matches_target(
    conflict: &UniqueConflict,
    target: &crate::statement::UpsertTarget,
) -> bool {
    match target {
        crate::statement::UpsertTarget::Columns(columns) => conflict.key_ordinals == *columns,
        crate::statement::UpsertTarget::Constraint(name) => conflict
            .constraint_name
            .as_ref()
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name)),
    }
}

fn apply_upsert_update(
    ctx: UpsertUpdateContext<'_>,
    update: &crate::statement::UpsertUpdatePlan,
) -> Result<InsertOutcome> {
    ctx.conn
        .engine()
        .lock_row_for_relation(ctx.tx, ctx.table.relation_id, ctx.conflict.rowid)?;
    let existing =
        load_table_row_by_rowid(ctx.conn.engine(), ctx.tx, ctx.table, ctx.conflict.rowid)?
            .ok_or_else(|| {
                Error::ConstraintViolation(format!(
                    "UPSERT conflict row missing for table {}",
                    ctx.table.name
                ))
            })?;
    let upsert_row = RowContext::Upsert {
        current: &existing,
        excluded: ctx.excluded,
    };
    if let Some(selection) = &update.selection
        && !is_truthy(&eval_scalar(selection, &upsert_row, ctx.bindings)?)
    {
        return Ok(InsertOutcome::Ignored);
    }
    let old_values = existing.values.clone();
    let mut values = existing.values.clone();
    for (ordinal, expr) in &update.assignments {
        if *ordinal >= values.len() {
            return Err(Error::UnknownColumn(format!("ordinal {ordinal}")));
        }
        values[*ordinal] = eval_scalar(expr, &upsert_row, ctx.bindings)?;
    }
    values = apply_row_affinity(ctx.table, values)?;
    let new_rowid = choose_rowid_for_update(ctx.conn.engine(), ctx.table, &values, existing.rowid)?;
    if let Some(alias) = ctx.table.rowid_alias_column
        && let Some(slot) = values.get_mut(alias as usize)
        && matches!(&*slot, SqlValue::Null)
    {
        *slot = SqlValue::Integer(new_rowid.0 as i64);
    }
    apply_constraints(ctx.table, &values)?;
    ensure_unique_constraints(
        ctx.conn,
        ctx.session,
        ctx.tx,
        ctx.table,
        &values,
        Some(existing.rowid),
    )?;
    let payload = encode_sql_row(ctx.table.table_id.0, &values)?;
    if new_rowid == existing.rowid {
        ctx.conn.engine().update_for_relation(
            ctx.tx,
            ctx.table.relation_id,
            existing.rowid,
            payload,
        )?;
    } else {
        ctx.conn
            .engine()
            .delete_for_relation(ctx.tx, ctx.table.relation_id, existing.rowid)?;
        ctx.conn
            .engine()
            .insert_for_relation(ctx.tx, ctx.table.relation_id, new_rowid, payload)?;
    }
    crate::exec::index_dml::maintain_indexes_on_update(
        ctx.conn.engine(),
        ctx.tx,
        ctx.table,
        &old_values,
        &values,
        existing.rowid,
        new_rowid,
    )?;
    Ok(InsertOutcome::Updated {
        rowid: new_rowid,
        values,
    })
}

/// Phase 10 Lane SQL-C: SQLite-style conflict resolution matrix.
///
/// SQLite documents five `ON CONFLICT` resolution algorithms (see
/// <https://sqlite.org/lang_conflict.html>):
///
/// | Action   | NOT NULL / CHECK fail | UNIQUE / PK fail              |
/// |----------|-----------------------|-------------------------------|
/// | ABORT    | error, undo this stmt | error, undo this stmt         |
/// | FAIL     | error, keep prior work| error, keep prior work        |
/// | IGNORE   | skip row silently     | skip row silently             |
/// | REPLACE  | error (no row to del) | delete conflicting row, insert|
/// | ROLLBACK | error, abort whole tx | error, abort whole tx         |
///
/// Centralising the matrix here means NOT NULL/CHECK and UNIQUE failures
/// dispatch to the same helper, which fixes the long-standing bug where
/// `INSERT OR IGNORE` did *not* swallow NOT NULL/CHECK failures because
/// `apply_constraints` ran before any conflict-action plumbing.
///
/// **Deviations from SQLite (documented):**
/// - `OR FAIL` and `OR ROLLBACK` currently behave like `OR ABORT` because
///   the surrounding `with_write_tx` machinery in `exec.rs` (out of scope
///   for this lane) treats every `Err` return as "rollback the implicit
///   tx" and every error inside an explicit tx as "session is poisoned".
///   The parser still distinguishes the verbs and the conflict helper
///   below records the intended action; full FAIL/ROLLBACK semantics
///   require statement-level partial commit and explicit-tx-aware error
///   classification, both of which span outside Lane SQL-C's allowed
///   files. See the `phase10_sqlc_conflict_matrix` tests for the
///   currently-asserted behaviour.
fn conflict_action_for(conflict: Option<&crate::statement::InsertConflict>) -> ConflictAction {
    match conflict {
        Some(crate::statement::InsertConflict::Sqlite(algo)) => match algo {
            crate::statement::ConflictAlgorithm::Abort => ConflictAction::Abort,
            crate::statement::ConflictAlgorithm::Fail => ConflictAction::Fail,
            crate::statement::ConflictAlgorithm::Ignore => ConflictAction::Ignore,
            crate::statement::ConflictAlgorithm::Replace => ConflictAction::Replace,
            crate::statement::ConflictAlgorithm::Rollback => ConflictAction::Rollback,
        },
        Some(crate::statement::InsertConflict::Upsert(_)) | None => ConflictAction::Abort,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConflictAction {
    Abort,
    Fail,
    Ignore,
    Replace,
    Rollback,
}

impl ConflictAction {
    /// True iff a NOT NULL or CHECK violation should be silently ignored
    /// (turning the row into `InsertOutcome::Ignored`). REPLACE does NOT
    /// help NOT NULL/CHECK because there is no conflicting row to delete
    /// — those constraints fire before any unique probe.
    fn ignores_check_or_not_null(self) -> bool {
        matches!(self, ConflictAction::Ignore)
    }
}

/// Run NOT NULL and CHECK validation against `values`, applying the
/// conflict-action verb. Returns `Ok(true)` to mean "row was dropped per
/// IGNORE", `Ok(false)` to mean "constraints passed", and `Err` for any
/// other action where a violation was found.
fn apply_constraints_with_action(
    table: &TableDef,
    values: &[SqlValue],
    action: ConflictAction,
) -> Result<bool> {
    match apply_constraints(table, values) {
        Ok(()) => Ok(false),
        Err(err) => {
            if action.ignores_check_or_not_null() {
                Ok(true)
            } else {
                // FAIL/ABORT/ROLLBACK and REPLACE all surface the
                // violation. The deviation note in the helper docs above
                // covers the FAIL/ROLLBACK collapse.
                Err(err)
            }
        }
    }
}

pub(super) fn insert_row_with_resolution(
    conn: &Connection,
    session: &mut SessionState,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    values: &mut Vec<SqlValue>,
    conflict: Option<&crate::statement::InsertConflict>,
    bindings: &[Option<SqlValue>],
) -> Result<InsertOutcome> {
    values.resize(table.columns.len(), SqlValue::Null);
    *values = apply_row_affinity(table, std::mem::take(values))?;
    let rowid = choose_rowid_for_insert(conn.engine(), table, values)?;

    let action = conflict_action_for(conflict);
    if apply_constraints_with_action(table, values, action)? {
        // IGNORE swallowed a NOT NULL or CHECK violation. The row is
        // dropped silently — no heap insert, no rowid bump beyond what
        // `choose_rowid_for_insert` already reserved (matches SQLite,
        // which advances the implicit rowid even for ignored rows).
        return Ok(InsertOutcome::Ignored);
    }

    let conflicts = collect_unique_conflicts(conn, session, tx, table, values, None)?;
    if conflicts.is_empty() {
        // Order: index unique-conflict check (already done above) ->
        // heap insert -> index inserts. If the heap insert succeeds but
        // the index insert fails, the kernel rolls back the whole tx so
        // recovery either replays both or neither.
        let payload = encode_sql_row(table.table_id.0, values)?;
        conn.engine()
            .insert_for_relation(tx, table.relation_id, rowid, payload)?;
        crate::exec::index_dml::maintain_indexes_on_insert(
            conn.engine(),
            tx,
            table,
            values,
            rowid,
        )?;
        session.last_insert_rowid = Some(rowid.0 as i64);
        return Ok(InsertOutcome::Inserted {
            rowid,
            values: values.clone(),
        });
    }

    apply_unique_conflict_resolution(
        conn, session, tx, table, values, &conflicts, conflict, action, bindings,
    )
}

/// UNIQUE / PK conflict-resolution matrix. Called only when at least one
/// conflicting row was found. NOT NULL / CHECK violations are handled
/// separately by `apply_constraints_with_action` because they can fire
/// even when there is no other row to compare against.
#[allow(clippy::too_many_arguments)]
fn apply_unique_conflict_resolution(
    conn: &Connection,
    session: &mut SessionState,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    values: &mut Vec<SqlValue>,
    conflicts: &[UniqueConflict],
    conflict: Option<&crate::statement::InsertConflict>,
    action: ConflictAction,
    bindings: &[Option<SqlValue>],
) -> Result<InsertOutcome> {
    // UPSERT (`ON CONFLICT(col) DO ...`) is its own branch; `action` is
    // ABORT for that case (we map UPSERT to ABORT in `conflict_action_for`)
    // because the dispatch lives below.
    if let Some(crate::statement::InsertConflict::Upsert(upsert)) = conflict {
        return apply_upsert_branch(
            conn, session, tx, table, values, conflicts, upsert, bindings,
        );
    }

    match action {
        ConflictAction::Ignore => Ok(InsertOutcome::Ignored),
        ConflictAction::Replace => {
            // INSERT OR REPLACE: delete each conflicting heap row (and
            // its index entries) before inserting the new tuple. Note
            // that REPLACE does NOT bypass NOT NULL/CHECK — those were
            // already validated above.
            let mut deleted = std::collections::HashSet::new();
            for conflict in conflicts {
                if deleted.insert(conflict.rowid) {
                    let old_row =
                        load_table_row_by_rowid(conn.engine(), tx, table, conflict.rowid)?;
                    conn.engine()
                        .delete_for_relation(tx, table.relation_id, conflict.rowid)?;
                    if let Some(old_row) = old_row {
                        crate::exec::index_dml::maintain_indexes_on_delete(
                            conn.engine(),
                            tx,
                            table,
                            &old_row.values,
                            conflict.rowid,
                        )?;
                    }
                }
            }
            let rowid = choose_rowid_for_insert(conn.engine(), table, values)?;
            let payload = encode_sql_row(table.table_id.0, values)?;
            conn.engine()
                .insert_for_relation(tx, table.relation_id, rowid, payload)?;
            crate::exec::index_dml::maintain_indexes_on_insert(
                conn.engine(),
                tx,
                table,
                values,
                rowid,
            )?;
            session.last_insert_rowid = Some(rowid.0 as i64);
            Ok(InsertOutcome::Inserted {
                rowid,
                values: values.clone(),
            })
        }
        // ABORT, FAIL and ROLLBACK all surface the violation here. See
        // the deviation note on `conflict_action_for` — full FAIL /
        // ROLLBACK semantics require changes outside Lane SQL-C's allowed
        // files.
        ConflictAction::Abort | ConflictAction::Fail | ConflictAction::Rollback => Err(
            Error::ConstraintViolation(format!("UNIQUE constraint failed: {}", table.name)),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_upsert_branch(
    conn: &Connection,
    session: &mut SessionState,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    values: &mut Vec<SqlValue>,
    conflicts: &[UniqueConflict],
    upsert: &crate::statement::UpsertPlan,
    bindings: &[Option<SqlValue>],
) -> Result<InsertOutcome> {
    let hit = if let Some(target) = &upsert.target {
        conflicts
            .iter()
            .find(|conflict| unique_conflict_matches_target(conflict, target))
    } else {
        conflicts.first()
    };
    let Some(hit) = hit else {
        return Err(Error::ConstraintViolation(format!(
            "UNIQUE constraint failed: {}",
            table.name
        )));
    };
    match &upsert.action {
        crate::statement::UpsertAction::DoNothing => Ok(InsertOutcome::Ignored),
        crate::statement::UpsertAction::DoUpdate(update) => apply_upsert_update(
            UpsertUpdateContext {
                conn,
                session,
                tx,
                table,
                excluded: values,
                conflict: hit,
                bindings,
            },
            update,
        ),
    }
}

pub(crate) fn ensure_unique_constraints(
    conn: &Connection,
    session: &mut SessionState,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    values: &[SqlValue],
    skip_rowid: Option<RowId>,
) -> Result<()> {
    if collect_unique_conflicts(conn, session, tx, table, values, skip_rowid)?.is_empty() {
        Ok(())
    } else {
        Err(Error::ConstraintViolation(format!(
            "UNIQUE constraint failed: {}",
            table.name
        )))
    }
}

pub(crate) fn choose_rowid_for_insert(
    engine: &Engine,
    table: &TableDef,
    values: &mut [SqlValue],
) -> Result<RowId> {
    if let Some(alias) = table.rowid_alias_column {
        let slot = alias as usize;
        match values.get(slot).cloned().unwrap_or(SqlValue::Null) {
            SqlValue::Null => {
                let rowid = engine.reserve_row_id();
                values[slot] = SqlValue::Integer(rowid.0 as i64);
                Ok(rowid)
            }
            SqlValue::Integer(v) if v >= 0 => Ok(RowId::new(v as u64)),
            SqlValue::Real(v) if v >= 0.0 && v.fract() == 0.0 => Ok(RowId::new(v as u64)),
            SqlValue::Integer(_) | SqlValue::Real(_) => Err(Error::DatatypeMismatch),
            _ => Err(Error::DatatypeMismatch),
        }
    } else {
        Ok(engine.reserve_row_id())
    }
}

pub(crate) fn choose_rowid_for_update(
    engine: &Engine,
    table: &TableDef,
    values: &[SqlValue],
    current_rowid: RowId,
) -> Result<RowId> {
    if let Some(alias) = table.rowid_alias_column {
        match values
            .get(alias as usize)
            .cloned()
            .unwrap_or(SqlValue::Null)
        {
            SqlValue::Null => Ok(engine.reserve_row_id()),
            SqlValue::Integer(v) if v >= 0 => Ok(RowId::new(v as u64)),
            SqlValue::Real(v) if v >= 0.0 && v.fract() == 0.0 => Ok(RowId::new(v as u64)),
            SqlValue::Integer(_) | SqlValue::Real(_) => Err(Error::DatatypeMismatch),
            _ => Err(Error::DatatypeMismatch),
        }
    } else {
        Ok(current_rowid)
    }
}

pub(crate) fn collect_table_rows(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
) -> Result<Vec<TableRow>> {
    collect_table_rows_with_alias(engine, tx, table, None)
}

pub(crate) fn collect_table_rows_with_alias(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    alias: Option<Arc<str>>,
) -> Result<Vec<TableRow>> {
    let mut rows = Vec::new();
    let rowids = engine.relation_rowids(table.relation_id)?;
    for rowid in rowids {
        if let Some(row) = load_table_row_by_rowid(engine, tx, table, rowid)? {
            let mut row = row;
            row.alias = alias.clone();
            rows.push(row);
        }
    }
    Ok(rows)
}

pub(crate) fn collect_join_rows(
    engine: &Engine,
    tx: &mut Txn,
    tables: &[crate::statement::BoundTable],
) -> Result<Vec<SqlRow>> {
    let mut joined: Vec<Vec<JoinedRow>> = vec![Vec::new()];
    for table in tables {
        let rows = collect_table_rows_with_alias(engine, tx, &table.table, table.alias.clone())?;
        let mut next = Vec::new();
        for prefix in &joined {
            for row in &rows {
                let mut combined = prefix.clone();
                combined.push(joined_row_from_table_row(table, Some(row.clone())));
                next.push(combined);
            }
        }
        joined = next;
    }
    Ok(joined.into_iter().map(SqlRow::Joined).collect())
}

pub(crate) fn collect_join_source_rows(
    engine: &Engine,
    tx: &mut Txn,
    source: &crate::statement::JoinSource,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlRow>> {
    let base_rows =
        collect_table_rows_with_alias(engine, tx, &source.base.table, source.base.alias.clone())?;
    let mut joined: Vec<Vec<JoinedRow>> = base_rows
        .into_iter()
        .map(|row| vec![joined_row_from_table_row(&source.base, Some(row))])
        .collect();

    for step in &source.joins {
        let right_rows =
            collect_table_rows_with_alias(engine, tx, &step.right.table, step.right.alias.clone())?;
        let mut next = Vec::new();
        for prefix in &joined {
            let mut matched = false;
            for row in &right_rows {
                let mut combined = prefix.clone();
                combined.push(joined_row_from_table_row(&step.right, Some(row.clone())));
                if selection_passes(&step.selection, &SqlRow::Joined(combined.clone()), bindings)? {
                    matched = true;
                    next.push(combined);
                }
            }
            if !matched && matches!(step.kind, crate::statement::JoinKind::Left) {
                let mut combined = prefix.clone();
                combined.push(joined_row_from_table_row(&step.right, None));
                next.push(combined);
            }
        }
        joined = next;
    }

    Ok(joined.into_iter().map(SqlRow::Joined).collect())
}

fn joined_row_from_table_row(
    table: &crate::statement::BoundTable,
    row: Option<TableRow>,
) -> JoinedRow {
    let alias = table.alias.clone();
    let row = row.map(|mut row| {
        row.alias = alias.clone();
        row
    });
    JoinedRow {
        table: Arc::clone(&table.table),
        alias,
        row,
    }
}

pub(crate) fn collect_table_rowids(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
) -> Result<Vec<RowId>> {
    let mut rowids = Vec::new();
    let scan = engine.relation_rowids(table.relation_id)?;
    for rowid in scan {
        if load_table_row_by_rowid(engine, tx, table, rowid)?.is_some() {
            rowids.push(rowid);
        }
    }
    Ok(rowids)
}

pub(crate) fn load_table_row_by_rowid(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    rowid: RowId,
) -> Result<Option<TableRow>> {
    if let Some(payload) = engine.get_for_relation(tx, table.relation_id, rowid)?
        && let Some((table_id, values)) = decode_sql_row(&payload)?
        && table_id == table.table_id.0
    {
        let mut values = values;
        if values.len() < table.columns.len() {
            values.resize(table.columns.len(), SqlValue::Null);
            values = build_default_values(table, values)?;
        }
        return Ok(Some(TableRow {
            rowid,
            values,
            table: Arc::clone(table),
            alias: None,
        }));
    }
    Ok(None)
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
    let value = eval_scalar(value_side, &RowContext::Empty, bindings)?;
    match value {
        SqlValue::Integer(v) if v >= 0 => Ok(Some(RowId::new(v as u64))),
        SqlValue::Real(v) if v >= 0.0 && v.fract() == 0.0 => Ok(Some(RowId::new(v as u64))),
        SqlValue::Null => Ok(None),
        _ => Err(Error::DatatypeMismatch),
    }
}
