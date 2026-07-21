//! Transaction runtime, row mutation, locking, and commit/rollback behavior.

use std::sync::Arc;
use std::time::Duration;

use crate::engine::page_heap::RelationWriteTarget;
use crate::engine::tx::PendingIndexHandle;
use crate::format::{Csn, Lsn, RelId, RowId};
use crate::txn::Isolation;
use crate::wal::{WalPayload, WalRecordKind};
use crate::{Error, Result};

use super::{BEGIN_LOCK_KEY, CommitDurability, CommitOutcome, Engine, EngineConfig, Txn};

#[path = "runtime/commit.rs"]
mod commit;
#[path = "runtime/mutation.rs"]
mod mutation;

#[cfg(feature = "failpoints")]
pub use commit::arm_commit_failure_for_thread;

impl Engine {
    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    pub fn set_busy_timeout(&self, timeout: Duration) {
        self.locks.set_timeout(timeout);
    }

    pub fn begin(&self, isolation: Isolation) -> Result<Txn> {
        if isolation == Isolation::Serializable {
            return Err(Error::UnsupportedIsolation);
        }
        Ok(self.txs.begin_txn(isolation))
    }

    /// Select the isolation behavior for an already-open transaction.
    ///
    /// SQL `SET TRANSACTION` reaches this method before it updates any
    /// session-visible label, so unsupported isolation never becomes a
    /// false promise. Switching to read committed also refreshes the
    /// statement snapshot immediately; later statements refresh again at
    /// their execution boundary.
    pub fn set_transaction_isolation(&self, tx: &mut Txn, isolation: Isolation) -> Result<()> {
        if isolation == Isolation::Serializable {
            return Err(Error::UnsupportedIsolation);
        }
        tx.set_isolation(isolation);
        self.refresh_statement_snapshot(tx);
        Ok(())
    }

    /// Refresh visibility at a statement boundary for read committed.
    pub fn refresh_statement_snapshot(&self, tx: &mut Txn) {
        self.refresh_read_committed(tx);
    }

    pub fn reserve_begin_lock(&self, tx: &mut Txn) -> Result<()> {
        if tx.has_row_lock(BEGIN_LOCK_KEY) {
            return Ok(());
        }
        self.locks
            .lock(BEGIN_LOCK_KEY.rel_id, BEGIN_LOCK_KEY.row_id, tx.id())?;
        tx.push_row_lock(BEGIN_LOCK_KEY);
        Ok(())
    }
}
