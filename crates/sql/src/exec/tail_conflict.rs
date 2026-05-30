use super::*;

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
pub(in crate::exec) enum InsertOutcome {
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
        // disables the conflict check entirely. Build through the same
        // SQL-side key adapter used by DML maintenance so expression-source
        // indexes evaluate their stored key SQL before conflict probing.
        let built = crate::exec::index_dml::build_index_key_with_values(table, index, values)?;
        if built.key.contains_null {
            continue;
        }
        if let Some(handle) =
            crate::exec::index_dml::open_index_handle_for_tx(conn.engine(), tx, index)
        {
            let (kernel_guard, hit) = crate::exec::index_dml::probe_unique_for_conflict(
                conn.engine(),
                &handle,
                tx,
                skip_rowid,
                &built.key,
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
            let sql_lock_key = unique_key_bytes(table.table_id.0, index.index_id.0, &built.values)?;
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
        let built = crate::exec::index_dml::build_index_key_with_values(table, index, values)?;
        if built.key.contains_null {
            continue;
        }
        let lock_key = unique_key_bytes(table.table_id.0, index.index_id.0, &built.values)?;
        let guard = conn.unique_locks().lock(lock_key, tx.id().0)?;
        session.unique_guards.push(guard);
        for row in &rows {
            if skip_rowid == Some(row.rowid) {
                continue;
            }
            let other =
                crate::exec::index_dml::build_index_key_with_values(table, index, &row.values)?;
            if key_values_equal(&built.values, &other.values) {
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
    let existing =
        match load_table_row_by_rowid(ctx.conn.engine(), ctx.tx, ctx.table, ctx.conflict.rowid)? {
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
    let mut scratch = EvalScratch::default();
    for (ordinal, expr) in &update.assignments {
        if *ordinal >= values.len() {
            return Err(Error::UnknownColumn(format!("ordinal {ordinal}")));
        }
        values[*ordinal] = evaluate_dml_value(
            ctx.table,
            *ordinal,
            expr,
            &upsert_row,
            ctx.bindings,
            &mut scratch,
        )?;
    }
    values = apply_row_affinity(ctx.table, values)?;
    // Phase-11 SQL-D A6: an UPSERT DO UPDATE may have touched an input
    // to a STORED generated column. Recompute every STORED column
    // before the persisted row is materialised.
    values = compute_stored_generated_columns(ctx.table, values)?;
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

pub(in crate::exec) fn insert_row_with_resolution(
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
    // Phase-11 SQL-D A6: compute STORED generated columns from inputs
    // (idempotent if already computed by a build_row* path) before any
    // constraint validation or unique-conflict checks observe the row.
    *values = compute_stored_generated_columns(table, std::mem::take(values))?;
    let rowid = choose_rowid_for_insert(session, conn.engine(), tx, table, values)?;

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
        // A6 SQLite parity: verify every declared FK against its parent.
        // Deferred constraints are queued for COMMIT; immediate ones must
        // resolve right now.
        crate::exec::fk::enforce_fk_on_insert(conn, session, tx, table, values, rowid)?;
        super::record_sqlite_sequence_rowid(session, table, rowid);
        session.last_insert_rowid = Some(rowid.0 as i64);
        return Ok(InsertOutcome::Inserted {
            rowid,
            values: values.clone(),
        });
    }

    apply_unique_conflict_resolution(
        conn, session, tx, table, rowid, values, &conflicts, conflict, action, bindings,
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
    rowid: RowId,
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
            let mut same_row_old_values = None;
            for conflict in conflicts {
                if deleted.insert(conflict.rowid) {
                    let old_row =
                        load_table_row_by_rowid(conn.engine(), tx, table, conflict.rowid)?;
                    if conflict.rowid == rowid {
                        let Some(old_row) = old_row else {
                            return Err(Error::ConstraintViolation(format!(
                                "REPLACE conflict row missing for table {}",
                                table.name
                            )));
                        };
                        same_row_old_values = Some(old_row.values);
                        continue;
                    }
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
                        crate::exec::fk::enforce_fk_on_parent_delete(
                            conn,
                            session,
                            tx,
                            table,
                            &old_row.values,
                        )?;
                    }
                }
            }
            let payload = encode_sql_row(table.table_id.0, values)?;
            if let Some(old_values) = same_row_old_values {
                conn.engine()
                    .update_for_relation(tx, table.relation_id, rowid, payload)?;
                crate::exec::index_dml::maintain_indexes_on_update(
                    conn.engine(),
                    tx,
                    table,
                    &old_values,
                    values,
                    rowid,
                    rowid,
                )?;
                crate::exec::fk::enforce_fk_on_insert(conn, session, tx, table, values, rowid)?;
                crate::exec::fk::enforce_fk_on_parent_update(
                    conn,
                    session,
                    tx,
                    table,
                    &old_values,
                    values,
                )?;
            } else {
                conn.engine()
                    .insert_for_relation(tx, table.relation_id, rowid, payload)?;
                crate::exec::index_dml::maintain_indexes_on_insert(
                    conn.engine(),
                    tx,
                    table,
                    values,
                    rowid,
                )?;
                crate::exec::fk::enforce_fk_on_insert(conn, session, tx, table, values, rowid)?;
            }
            super::record_sqlite_sequence_rowid(session, table, rowid);
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
    for arm in upsert.arms.iter() {
        let hit = if let Some(target) = &arm.target {
            conflicts
                .iter()
                .find(|conflict| unique_conflict_matches_target(conflict, target))
        } else {
            conflicts.first()
        };
        let Some(hit) = hit else {
            continue;
        };
        return match &arm.action {
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
        };
    }
    Err(Error::ConstraintViolation(format!(
        "UNIQUE constraint failed: {}",
        table.name
    )))
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
