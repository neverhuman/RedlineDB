use std::sync::Arc;

use crate::format::Lsn;
use crate::telemetry::Phase11Counters;
use crate::{Error, Result};

use super::*;

impl WalCoordinator {
    pub fn written_lsn(&self) -> Result<Lsn> {
        if self.volatile {
            return Ok(Lsn::ZERO);
        }
        self.shared
            .state
            .lock()
            .map(|state| state.written_lsn)
            .map_err(|_| Error::CorruptWal("wal coordinator mutex poisoned"))
    }

    pub fn durable_lsn(&self) -> Result<Lsn> {
        if self.volatile {
            return Ok(Lsn::ZERO);
        }
        self.shared
            .state
            .lock()
            .map(|state| state.durable_lsn)
            .map_err(|_| Error::CorruptWal("wal coordinator mutex poisoned"))
    }

    /// Lane BH P1 #7: durability syscall counters captured by the
    /// WAL writer thread. Cheap (relaxed atomic loads) so callers
    /// can sample without holding the coordinator state lock.
    pub fn sync_counters_snapshot(&self) -> WalSyncCountersSnapshot {
        self.sync_counters.snapshot()
    }

    /// Wave 1A-F: install (or replace) the Phase 11 telemetry sink so
    /// the writer thread can bump `wal_batch_size_buckets` once per
    /// fdatasync with the count of records drained. Optional — leaving
    /// this unset keeps every existing call site working unchanged.
    pub fn set_phase11_counters(&self, counters: Arc<Phase11Counters>) {
        if let Ok(mut slot) = self.shared.phase11.write() {
            *slot = Some(counters);
        }
    }

    pub fn prune_segments_below_checkpoint_lsn(&self, checkpoint_lsn: Lsn) -> Result<usize> {
        if self.volatile {
            let _ = checkpoint_lsn;
            return Ok(0);
        }
        let keep_segment = segment_for_lsn(checkpoint_lsn, self.config.segment_bytes);
        let active_segment = self
            .shared
            .state
            .lock()
            .map(|state| segment_for_lsn(state.reserved_lsn, self.config.segment_bytes))
            .map_err(|_| Error::CorruptWal("wal coordinator mutex poisoned"))?;
        let mut removed = 0_usize;
        for candidate in segment_numbers_on_disk(&self.dir)? {
            if candidate < keep_segment && candidate < active_segment {
                let path = segment_path(&self.dir, candidate);
                if let Err(err) = std::fs::remove_file(path)
                    && err.kind() != std::io::ErrorKind::NotFound
                {
                    return Err(err.into());
                }
                removed += 1;
            }
        }
        Ok(removed)
    }
}

impl Drop for WalCoordinator {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared.state.lock() {
            state.shutdown = true;
            self.shared.cvar.notify_all();
        }
        if let Ok(mut writer) = self.writer.lock()
            && let Some(writer) = writer.take()
        {
            let _ = writer.join();
        }
    }
}
