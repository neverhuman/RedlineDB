use super::*;

#[cfg(feature = "failpoints")]
std::thread_local! {
    /// Lane KH P0 #3: per-thread switch for the
    /// `engine::commit::before_publish` failpoint. The fail-crate
    /// registry is process-wide, so without this guard the closure
    /// would inject the fault on every commit in every parallel test.
    /// Tests call [`arm_commit_failure_for_thread`] before issuing
    /// their commit and clear it afterwards.
    static COMMIT_FAILURE_ARMED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Arm or disarm the thread-local commit-failure injection used by the
/// `engine::commit::before_publish` failpoint closure. Available only
/// when the kernel is built with `failpoints` so production builds pay
/// nothing for it.
#[cfg(feature = "failpoints")]
pub fn arm_commit_failure_for_thread(armed: bool) {
    COMMIT_FAILURE_ARMED.with(|c| c.set(armed));
}

#[cfg(feature = "failpoints")]
fn commit_failure_armed_for_thread() -> bool {
    COMMIT_FAILURE_ARMED.with(|c| c.get())
}

#[cfg(not(feature = "failpoints"))]
#[allow(dead_code)]
fn commit_failure_armed_for_thread() -> bool {
    false
}

impl Engine {
    pub fn commit(&self, mut tx: Txn) -> Result<CommitOutcome> {
        tx.ensure_open()?;
        let pending_schema = tx.pending_schema_snapshot();
        if let Some(snapshot) = pending_schema.as_deref() {
            let snapshot_bytes = crate::catalog::encode_snapshot(snapshot)?;
            self.wal.append(
                WalRecordKind::Logical,
                tx.id(),
                WalPayload::CatalogSnapshot {
                    tx_id: tx.id(),
                    schema_epoch: snapshot.meta.schema_epoch.0,
                    snapshot: snapshot_bytes,
                }
                .encode()?,
            )?;
        }

        let (csn, append) = match self
            .wal
            .append_commit(tx.id(), || self.txs.reserve_commit_csn())
        {
            Ok(value) => value,
            Err(err) => {
                self.txs.abort(tx.id());
                self.release_locks(&mut tx);
                tx.close();
                return Err(err);
            }
        };

        if self.config.commit_durability == CommitDurability::Strict
            && pending_schema.is_none()
            && tx.pending_index_handles().is_empty()
        {
            self.txs.publish_commit(tx.id(), csn);
            self.release_locks(&mut tx);
            tx.close();
            if let Err(err) = self.wal.flush_until(append.end_lsn) {
                panic!("strict WAL flush failed after commit publish: {err}");
            }
            crate::fail_point!("engine::commit::before_publish", |arg: Option<String>| {
                if !commit_failure_armed_for_thread() {
                    let _ = arg;
                    return Ok(CommitOutcome::Committed(csn));
                }
                let _detail = match arg {
                    Some(detail) => detail,
                    None => "engine::commit::before_publish injected fault".to_string(),
                };
                Ok(CommitOutcome::MaybeCommitted)
            });
            return Ok(CommitOutcome::Committed(csn));
        }

        let commit_barrier = match self.config.commit_durability {
            CommitDurability::Strict => self.wal.flush_until(append.end_lsn),
            CommitDurability::Normal => self.wal.write_until(append.end_lsn),
            CommitDurability::UnsafeDev => Ok(append.end_lsn),
        };
        if let Err(err) = commit_barrier {
            self.txs.cancel_reserved_csn(csn);
            self.txs.abort(tx.id());
            self.release_locks(&mut tx);
            tx.close();
            return Err(err);
        }

        // Lane E failpoint: WAL fsync has acked but the CSN is not yet
        // visible to in-memory observers. The injected path returns
        // `MaybeCommitted` after publishing the commit locally, so higher
        // layers can surface the uncertainty without replaying any SQL-side
        // index repair.
        let _pending_schema_for_closure = pending_schema.clone();
        crate::fail_point!("engine::commit::before_publish", |arg: Option<String>| {
            if !commit_failure_armed_for_thread() {
                let _ = arg;
                return Ok(self.finish_commit(
                    &mut tx,
                    csn,
                    _pending_schema_for_closure.clone(),
                    CommitOutcome::Committed(csn),
                ));
            }
            let _detail = match arg {
                Some(detail) => detail,
                None => "engine::commit::before_publish injected fault".to_string(),
            };
            Ok(self.finish_commit(
                &mut tx,
                csn,
                _pending_schema_for_closure.clone(),
                CommitOutcome::MaybeCommitted,
            ))
        });
        Ok(self.finish_commit(&mut tx, csn, pending_schema, CommitOutcome::Committed(csn)))
    }

    fn finish_commit(
        &self,
        tx: &mut Txn,
        csn: Csn,
        pending_schema: Option<Arc<crate::catalog::SchemaSnapshot>>,
        outcome: CommitOutcome,
    ) -> CommitOutcome {
        self.txs.publish_commit(tx.id(), csn);
        if let Some(snapshot) = pending_schema {
            let _ = self.catalog_store.save_atomic(&snapshot);
            self.catalog.publish(snapshot);
        }
        if !tx.pending_index_handles().is_empty()
            && let Ok(mut handles) = self.index_handles.lock()
        {
            for action in tx.pending_index_handles() {
                match action {
                    PendingIndexHandle::Install(index_id, handle) => {
                        handles.insert(*index_id, Arc::clone(handle));
                    }
                    PendingIndexHandle::Remove(index_id) => {
                        handles.remove(index_id);
                    }
                }
            }
        }
        self.release_locks(tx);
        tx.close();
        outcome
    }

    pub fn rollback(&self, mut tx: Txn) -> Result<()> {
        tx.ensure_open()?;
        self.txs.abort(tx.id());
        self.release_locks(&mut tx);
        tx.close();
        Ok(())
    }

    pub(super) fn refresh_read_committed(&self, tx: &mut Txn) {
        if tx.isolation() == Isolation::ReadCommitted {
            tx.replace_snapshot(self.txs.snapshot());
        }
    }

    pub(super) fn lock_row(&self, tx: &mut Txn, row_id: RowId) -> Result<()> {
        let key = crate::engine::lock::RowKey {
            rel_id: self.rel_id,
            row_id,
        };
        if tx.has_row_lock(key) {
            return Ok(());
        }
        self.locks.lock(self.rel_id, row_id, tx.id())?;
        tx.push_row_lock(key);
        Ok(())
    }

    pub(super) fn lock_row_in_rel(&self, tx: &mut Txn, rel_id: RelId, row_id: RowId) -> Result<()> {
        let key = crate::engine::lock::RowKey { rel_id, row_id };
        if tx.has_row_lock(key) {
            return Ok(());
        }
        self.locks.lock(rel_id, row_id, tx.id())?;
        tx.push_row_lock(key);
        Ok(())
    }

    pub(super) fn release_locks(&self, tx: &mut Txn) {
        let tx_id = tx.id();
        for key in tx.drain_row_locks() {
            self.locks.unlock(key.rel_id, key.row_id, tx_id);
        }
    }
}
