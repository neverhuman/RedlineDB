//! Inspection, checkpoint, statistics, vacuum, and integrity entrypoints.

use std::collections::HashMap;
use std::sync::Arc;

use crate::catalog::IndexId as CatalogIndexId;
use crate::engine::page_heap::{PageBackedHeap, VacuumStats};
use crate::format::{Csn, PageId, RelId, RowId, TuplePtr, TxId};
use crate::index::BtreeIndex;
use crate::storage::{
    BufferPool, BufferPoolStats, ControlFile, DEFAULT_CHECKPOINT_BATCH_PAGES, PageFile,
    TxStatusCheckpoint,
};
use crate::telemetry::{Phase11Counters, Phase11CountersSnapshot};
use crate::txn::TxState;
use crate::{Error, Result};

use super::{CheckpointStats, ConcurrentTxStatus, Engine, StorageStatsSnapshot, TxStatusStats};

impl Engine {
    pub fn tx_state(&self, tx_id: TxId) -> TxState {
        self.txs.state(tx_id)
    }

    pub fn tx_status_stats(&self) -> TxStatusStats {
        self.txs.stats()
    }

    pub fn tx_status(&self) -> &ConcurrentTxStatus {
        &self.txs
    }

    #[doc(hidden)]
    pub fn buffer_pool_for_tests(&self) -> &BufferPool {
        &self.buffer
    }

    pub fn integrity_check(&self) -> Result<Vec<String>> {
        let snapshot = self.catalog.current();
        let mut errors = Vec::new();
        match PageFile::open(&self.data_path, self.config.page_size) {
            Ok(page_file) => match page_file.page_count() {
                Ok(page_count) => {
                    for page_no in 1..=page_count {
                        if let Err(err) = page_file.read_page(PageId(page_no)) {
                            errors.push(format!("page {page_no}: {err}"));
                        }
                    }
                }
                Err(err) => errors.push(format!("page file count: {err}")),
            },
            Err(err) => errors.push(format!("page file open: {err}")),
        }
        for index in &snapshot.indexes {
            if snapshot.table_by_id(index.table_id).is_none() {
                errors.push(format!(
                    "catalog index {} references missing table",
                    index.name
                ));
            }
        }
        if !self.volatile {
            let mut wal_reader = crate::wal::WalReader::new(&self.wal_dir, self.config.wal.clone());
            if let Err(err) = wal_reader.scan_report() {
                errors.push(format!("wal prefix scan: {err}"));
            }
        }
        let handles = self
            .index_handles
            .lock()
            .map_err(|_| Error::CorruptPage("engine index handles mutex poisoned"))?;
        for index in &snapshot.indexes {
            if index.meta_page_id.is_none() {
                continue;
            }
            let Some(handle) = handles.get(&index.index_id) else {
                errors.push(format!("index {} has no open handle", index.name));
                continue;
            };
            let report = handle.validate()?;
            for error in report.errors {
                errors.push(format!("index {}: {error}", index.name));
            }
        }
        Ok(errors)
    }

    pub fn oldest_active_snapshot_csn(&self) -> Csn {
        self.txs.oldest_active_snapshot_csn()
    }

    pub fn vacuum(&self) -> Result<VacuumStats> {
        self.vacuum_with_horizon(self.oldest_active_snapshot_csn())
    }

    pub fn vacuum_with_horizon(&self, horizon: Csn) -> Result<VacuumStats> {
        self.heap.vacuum(horizon, &self.txs)
    }

    pub fn flush_heap_pages(&self) -> Result<()> {
        if self.volatile {
            return Ok(());
        }
        let durable_lsn = self.wal.durable_lsn()?;
        self.heap.flush_all(durable_lsn)
    }

    pub fn resident_heap_pages(&self) -> usize {
        self.heap.resident_pages()
    }

    pub fn row_directory_entries(&self) -> Result<Vec<(RowId, TuplePtr)>> {
        self.heap.row_directory_entries()
    }

    pub fn relation_rowids(&self, rel_id: RelId) -> Result<Vec<RowId>> {
        self.heap.relation_rowids(rel_id)
    }

    pub fn relation_entries(&self, rel_id: RelId) -> Result<Vec<(RowId, TuplePtr)>> {
        self.heap.relation_entries(rel_id)
    }

    pub fn buffer_pool_stats(&self) -> BufferPoolStats {
        self.heap.buffer_stats()
    }

    pub fn storage_stats(&self) -> Result<StorageStatsSnapshot> {
        Ok(StorageStatsSnapshot {
            buffer: self.heap.buffer_stats(),
            tx: self.txs.stats(),
            checkpoint: self.checkpoint_info()?,
            resident_heap_pages: self.heap.resident_pages(),
            wal_written_lsn: self.wal.written_lsn()?,
            wal_durable_lsn: self.wal.durable_lsn()?,
            vacuum_horizon_csn: self.oldest_active_snapshot_csn(),
            wal_sync_counters: self.wal.sync_counters_snapshot(),
            phase11_counters: self.phase11_counters_snapshot(),
        })
    }

    /// Phase 11 Wave 0: relaxed-atomic snapshot of the Phase 11
    /// counter aggregator. Mirrors
    /// [`crate::wal::WalCoordinator::sync_counters_snapshot`] for
    /// downstream telemetry callers (the bench harness picks it up
    /// via `Database::benchmark_stats`).
    pub fn phase11_counters_snapshot(&self) -> Phase11CountersSnapshot {
        self.phase11_counters.snapshot()
    }

    /// Phase 11 Wave 0: shared handle to the Phase 11 counter
    /// aggregator. Wave 1+ instrumentation sites can clone this
    /// `Arc` to gain direct access for `.fetch_add` calls without
    /// going through an additional accessor.
    pub fn phase11_counters(&self) -> Arc<Phase11Counters> {
        Arc::clone(&self.phase11_counters)
    }

    pub fn checkpoint(&self) -> Result<ControlFile> {
        self.checkpoint_with_stats().map(|stats| stats.control)
    }

    pub fn checkpoint_with_stats(&self) -> Result<CheckpointStats> {
        if self.volatile {
            return Ok(CheckpointStats {
                control: ControlFile::default(),
                flushed_pages: 0,
                flush_batches: 0,
            });
        }
        let durable_lsn = self.wal.flush_all()?;
        let flush = self
            .heap
            .flush_dirty_batches(durable_lsn, DEFAULT_CHECKPOINT_BATCH_PAGES)?;
        let page_count = self.heap.page_count()?;
        let mut checkpoint = self
            .checkpoint
            .lock()
            .map_err(|_| Error::CorruptPage("checkpoint mutex poisoned"))?;
        let generation = checkpoint
            .map(|control| control.generation + 1)
            .unwrap_or(1);
        self.tx_status_store.write(&TxStatusCheckpoint {
            generation,
            next_tx: self.txs.next_tx(),
            next_csn: self.txs.next_csn(),
            published_csn: self.txs.published_csn(),
            entries: self.txs.committed_states(),
        })?;
        self.catalog_store
            .save_atomic(self.catalog.current().as_ref())?;
        // Lane E failpoint: armed before the new control-file generation lands
        // on disk. A crash here forces recovery to fall back to the previous
        // generation, exercising the dual-control-file protocol.
        crate::fail_point!("engine::checkpoint");
        let next = self
            .control
            .write_next(*checkpoint, durable_lsn, page_count)?;
        self.wal
            .prune_segments_below_checkpoint_lsn(next.checkpoint_lsn)?;
        *checkpoint = Some(next);
        Ok(CheckpointStats {
            control: next,
            flushed_pages: flush.flushed_pages,
            flush_batches: flush.batches,
        })
    }

    pub fn checkpoint_info(&self) -> Result<Option<ControlFile>> {
        self.checkpoint
            .lock()
            .map(|checkpoint| *checkpoint)
            .map_err(|_| Error::CorruptPage("checkpoint mutex poisoned"))
    }

    /// Lane INT: structural validation across every catalog index handle,
    /// returning the per-index `errors` lists for the
    /// `redline_index_check` PRAGMA. Complements the flat
    /// `integrity_check()` which returns errors as strings only.
    pub fn integrity_check_per_index(&self) -> Result<Vec<(String, Vec<String>)>> {
        let snapshot = self.catalog.current();
        let handles = self
            .index_handles
            .lock()
            .map_err(|_| Error::CorruptPage("engine index handles mutex poisoned"))?;
        let mut out = Vec::new();
        for index in &snapshot.indexes {
            let Some(btree) = handles.get(&index.index_id) else {
                continue;
            };
            let validation = btree.validate()?;
            let errors = validation
                .errors
                .iter()
                .map(|s| (*s).to_owned())
                .collect::<Vec<_>>();
            out.push((index.name.to_string(), errors));
        }
        Ok(out)
    }

    /// Lane INT: full heap/index/page equivalence check. Returns the
    /// structured [`crate::integrity::IntegrityReport`] consumed by the
    /// `redline_full_check` PRAGMA and the bench certification harness.
    pub fn integrity_check_full(&self) -> Result<crate::integrity::IntegrityReport> {
        crate::integrity::run_full(self)
    }

    pub(crate) fn buffer_for_integrity(&self) -> &Arc<BufferPool> {
        &self.buffer
    }

    pub(crate) fn heap_for_integrity(&self) -> &PageBackedHeap {
        &self.heap
    }

    pub(crate) fn txs_for_integrity(&self) -> &ConcurrentTxStatus {
        &self.txs
    }

    pub(crate) fn txs_snapshot_for_integrity(&self) -> crate::txn::Snapshot {
        self.txs.snapshot()
    }

    pub(crate) fn read_raw_page_bytes_for_integrity(&self, page_id: PageId) -> Result<Vec<u8>> {
        self.buffer.read_page_bytes_unchecked(page_id)
    }

    pub(crate) fn index_handles_for_integrity(
        &self,
    ) -> Result<HashMap<CatalogIndexId, Arc<BtreeIndex>>> {
        let handles = self
            .index_handles
            .lock()
            .map_err(|_| Error::CorruptPage("engine index handles mutex poisoned"))?;
        Ok(handles.clone())
    }
}
