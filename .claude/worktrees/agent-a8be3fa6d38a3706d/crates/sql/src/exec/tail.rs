#[path = "tail_rows.rs"]
mod rows;
#[path = "tail_stats.rs"]
mod stats;

pub(crate) use rows::*;
pub(crate) use stats::*;

use super::*;

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
            let live = match load_table_row_by_rowid(conn.engine(), tx, &plan.table, row.rowid)?
                .map(|fresh| fresh.values)
            {
                Some(v) => v,
                None => row.values.clone(),
            };
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
        let value = match values.get(idx) {
            Some(v) => v,
            None => return Err(Error::UnknownColumn(column.name.to_string())),
        };
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
    // index has been allocated by Lane A. The default path (no
    // `meta_page_id`) preserves the original O(N) behavior so databases
    // without physical indexes still enforce UNIQUE.
    //
    // SQLite enforces UNIQUE on every unique index — both inline UNIQUE
    // constraints (which create a backing index AND a Constraint row) and
    // standalone `CREATE UNIQUE INDEX` statements (which only create the
    // index). We therefore enumerate `table.indexes` and only fall back to
    // the constraints list to recover the original constraint name when a
    // matching one exists.
    let mut conflicts = Vec::new();
    let mut pending_indexes: Vec<&redlinedb_kernel::catalog::IndexDef> = Vec::new();
    for index in &table.indexes {
        if !index.unique && !index.primary {
            continue;
        }
        let constraint_name = match table
            .constraints
            .iter()
            .find(|c| {
                (c.kind == ConstraintKind::Unique || c.kind == ConstraintKind::PrimaryKey)
                    && c.index_id == Some(index.index_id)
            })
            .and_then(|c| c.name.as_deref().map(Arc::<str>::from))
        {
            Some(name) => Some(name),
            None => Some(Arc::from(index.name.as_ref())),
        };
        // SQLite NULL parity: a NULL anywhere in the unique-key tuple
        // disables the conflict check entirely. We compute this once from
        // the SQL-side values so both index and default paths agree.
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

            // We still take a SQL-side guard so callers that share
            // `unique_locks()` continue to serialize against this key. The
            // dual locking is harmless and matches the default path below;
            // the SQL guard is also released on commit/rollback.
            let sql_lock_key = unique_key_bytes(table.table_id.0, index.index_id.0, &key_values)?;
            let sql_guard = conn.unique_locks().lock(sql_lock_key, tx.id().0)?;
            session.unique_guards.push(sql_guard);
            continue;
        }
        pending_indexes.push(index);
    }

    // O(N) heap scan for any indexes Lane A did not allocate yet.
    if pending_indexes.is_empty() {
        return Ok(conflicts);
    }
    let rows = collect_table_rows(conn.engine(), tx, table)?;
    for index in pending_indexes {
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
                let constraint_name = match table
                    .constraints
                    .iter()
                    .find(|c| c.index_id == Some(index.index_id))
                    .and_then(|c| c.name.as_deref().map(Arc::<str>::from))
                {
                    Some(name) => Some(name),
                    None => Some(Arc::from(index.name.as_ref())),
                };
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
    let existing = match load_table_row_by_rowid(
        ctx.conn.engine(),
        ctx.tx,
        ctx.table,
        ctx.conflict.rowid,
    )? {
        Some(row) => row,
        None => {
            return Err(Error::ConstraintViolation(format!(
                "UPSERT conflict row missing for table {}",
                ctx.table.name
            )));
        }
    };
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
