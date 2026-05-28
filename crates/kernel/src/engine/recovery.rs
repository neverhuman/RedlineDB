//! Engine create/open paths and WAL recovery replay.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use crate::catalog::{
    CatalogManager, CatalogStore, CatalogSyncPolicy, IndexId as CatalogIndexId, bootstrap_schema,
};
use crate::engine::lock::RowLockManager;
use crate::engine::page_heap::PageBackedHeap;
use crate::format::{Csn, Lsn, Page, RelId};
use crate::storage::{BufferPool, ControlStore, PageFile, TxStatusStore};
use crate::telemetry::Phase11Counters;
use crate::wal::{WalCoordinator, WalPayload, WalReader, WalRecord, WalRecordKind};
use crate::{Error, Result};

use super::{
    CommitDurability, ConcurrentTxStatus, Engine, EngineConfig, RecoveryMetrics, RecoveryReport,
    RecoveryTarget,
};

impl Engine {
    pub fn create(path: impl AsRef<Path>, config: EngineConfig) -> Result<Arc<Self>> {
        Self::create_inner(path.as_ref(), config, false)
    }

    /// Create a private volatile engine for process-local in-memory database
    /// handles. The engine still uses the regular heap and catalog state
    /// machines, but skips recovery sidecars and WAL writer startup because
    /// there is no durable image to recover.
    pub fn create_volatile(path: impl AsRef<Path>, config: EngineConfig) -> Result<Arc<Self>> {
        Self::create_inner(path.as_ref(), config, true)
    }

    fn create_inner(path: &Path, config: EngineConfig, volatile: bool) -> Result<Arc<Self>> {
        // For persistent databases, ensure the directory exists and scale shard
        // counts to match the available CPU parallelism.  For volatile (in-memory)
        // databases:
        //   • The caller (EphemeralRoot / OwnedTempRoot) already created the dir,
        //     so skipping create_dir_all saves 3–4 extra syscalls per process.
        //   • The config was built from EngineConfig::default() which deliberately
        //     does NOT call cached_available_parallelism() — avoiding the 4–6
        //     syscall cgroup walk on every fresh process.  Volatile databases use
        //     the small fixed shard counts; only persistent engines need full CPU
        //     scaling for multi-writer throughput.
        let config = if volatile {
            config
        } else {
            std::fs::create_dir_all(path)?;
            config.with_detected_parallelism()
        };
        let data_path = path.join(&config.data_file_name);
        let wal_dir = path.join("wal");
        let page_file = Arc::new(PageFile::create(&data_path, config.page_size)?);
        // W7-perf: for volatile databases use the caller-supplied shard hint so
        // we skip the cgroup walk inside BufferPool::new.  The parallelism value
        // from the (already-scaled) config is what we want; persistent databases
        // already went through with_detected_parallelism() above.
        let buffer = if volatile {
            // lock_shards is already the right scale (config default = 16, so
            // parallelism_hint = lock_shards / 4 = 4 matches heap_lanes default).
            let parallelism_hint = (config.lock_shards / 4).max(1);
            Arc::new(BufferPool::new_with_parallelism(
                page_file,
                config.buffer_pool_pages,
                parallelism_hint,
            )?)
        } else {
            Arc::new(BufferPool::new(page_file, config.buffer_pool_pages)?)
        };
        let wal = if volatile {
            Arc::new(WalCoordinator::volatile(config.wal.clone()))
        } else {
            Arc::new(WalCoordinator::create_with_shutdown_flush(
                &wal_dir,
                config.wal.clone(),
                flush_wal_on_shutdown(config.commit_durability),
            )?)
        };
        // Use the volatile constructors for in-memory engines: they skip
        // create_dir_all (already done by the caller) saving another 4–6
        // syscalls per process start.  Persistent engines use the regular
        // constructors which also guarantee the subdirectory exists.
        let control = if volatile {
            ControlStore::new_volatile(path)
        } else {
            ControlStore::new(path)?
        };
        let tx_status_store = if volatile {
            TxStatusStore::new_volatile(path)
        } else {
            TxStatusStore::new(path)?
        };
        let checkpoint = if volatile {
            None
        } else {
            control.load_latest()?
        };
        let catalog_store = CatalogStore::new_with_sync_policy(path, catalog_sync_policy(&config));
        let loaded_catalog = if volatile {
            None
        } else {
            catalog_store.load().ok().flatten()
        };
        let initial_catalog = match loaded_catalog.clone() {
            Some(catalog) => catalog,
            None => bootstrap_schema(RelId(10_000)),
        };
        if !volatile && loaded_catalog.is_none() {
            catalog_store.save_atomic(&initial_catalog)?;
        }
        let buffer = Arc::clone(&buffer);
        let phase11_counters = Arc::new(Phase11Counters::default());
        let locks = RowLockManager::new(config.lock_shards, config.busy_timeout);
        // Wave 1A-F: pipe Phase11 telemetry into the row-lock manager so
        // contended-acquire waits land in `lock_wait_us_buckets`. Same
        // Arc piped into the WAL coordinator so `wal_batch_size_buckets`
        // gets bumped per fdatasync.
        locks.set_phase11_counters(Arc::clone(&phase11_counters));
        wal.set_phase11_counters(Arc::clone(&phase11_counters));
        let heap_wal = if volatile {
            None
        } else {
            Some(Arc::clone(&wal))
        };
        let commit_durability_live = std::sync::atomic::AtomicU8::new(
            commit_durability_initial_u8(config.commit_durability),
        );
        Ok(Arc::new(Self {
            config: config.clone(),
            commit_durability_live,
            volatile,
            data_path,
            wal_dir,
            rel_id: config.rel_id,
            txs: ConcurrentTxStatus::new(),
            buffer: Arc::clone(&buffer),
            heap: PageBackedHeap::new_with_wal(config.rel_id, config.heap_lanes, buffer, heap_wal)?,
            catalog: CatalogManager::new(initial_catalog),
            catalog_store,
            locks,
            wal,
            phase11_counters,
            control,
            tx_status_store,
            checkpoint: std::sync::Mutex::new(checkpoint),
            index_handles: std::sync::Mutex::new(HashMap::new()),
        }))
    }

    pub fn open(path: impl AsRef<Path>, config: EngineConfig) -> Result<Arc<Self>> {
        Self::open_with_recovery_report(path, config).map(|(engine, _report)| engine)
    }

    pub fn open_with_recovery_report(
        path: impl AsRef<Path>,
        config: EngineConfig,
    ) -> Result<(Arc<Self>, RecoveryReport)> {
        Self::open_with_recovery_report_and_target(path, config, RecoveryTarget::Latest)
    }

    pub fn open_with_recovery_target(
        path: impl AsRef<Path>,
        config: EngineConfig,
        target: RecoveryTarget,
    ) -> Result<Arc<Self>> {
        Self::open_with_recovery_report_and_target(path, config, target).map(|(engine, _)| engine)
    }

    pub fn open_with_recovery_report_and_target(
        path: impl AsRef<Path>,
        config: EngineConfig,
        target: RecoveryTarget,
    ) -> Result<(Arc<Self>, RecoveryReport)> {
        // W7-perf: persistent-open path — scale shards up to CPU count just
        // like create_inner does for persistent databases.
        let config = config.with_detected_parallelism();
        let wal_dir = path.as_ref().join("wal");
        let mut reader = WalReader::new(&wal_dir, config.wal.clone());
        let scan_report = reader.scan_report()?;
        let wal_open_summary = scan_report.open_summary();
        let txs = ConcurrentTxStatus::new();
        std::fs::create_dir_all(path.as_ref())
            .map_err(|_| Error::CorruptPage("create engine directory failed"))?;
        let control = ControlStore::new(path.as_ref())
            .map_err(|_| Error::CorruptPage("create control store failed"))?;
        let tx_status_store = TxStatusStore::new(path.as_ref())
            .map_err(|_| Error::CorruptPage("create tx status store failed"))?;
        let checkpoint = control
            .load_latest()
            .map_err(|_| Error::CorruptPage("load control file failed"))?;
        let page_path = path.as_ref().join(&config.data_file_name);
        let page_file = if checkpoint.is_some() || page_path.exists() {
            Arc::new(
                PageFile::open(&page_path, config.page_size)
                    .map_err(|_| Error::CorruptPage("open recovered page file failed"))?,
            )
        } else {
            Arc::new(
                PageFile::create(&page_path, config.page_size)
                    .map_err(|_| Error::CorruptPage("create recovered page file failed"))?,
            )
        };
        let buffer = Arc::new(
            BufferPool::new(page_file, config.buffer_pool_pages)
                .map_err(|_| Error::CorruptPage("create buffer pool failed"))?,
        );
        let wal = Arc::new(
            WalCoordinator::open_with_scan_summary_and_shutdown_flush(
                &wal_dir,
                config.wal.clone(),
                wal_open_summary,
                flush_wal_on_shutdown(config.commit_durability),
            )
            .map_err(|_| Error::CorruptWal("open wal coordinator failed"))?,
        );
        let heap = PageBackedHeap::new_with_wal(
            config.rel_id,
            config.heap_lanes,
            Arc::clone(&buffer),
            Some(Arc::clone(&wal)),
        )
        .map_err(|_| Error::CorruptPage("create heap failed"))?;
        let catalog_store =
            CatalogStore::new_with_sync_policy(path.as_ref(), catalog_sync_policy(&config));
        let initial_catalog = match catalog_store.load().ok().flatten() {
            Some(catalog) => catalog,
            None => bootstrap_schema(RelId(10_000)),
        };
        let replay_from_lsn = if let Some(checkpoint) = checkpoint {
            let tx_status = tx_status_store.load(checkpoint.generation)?;
            if tx_status.generation != checkpoint.generation {
                return Err(Error::CorruptPage(
                    "tx status checkpoint generation mismatch",
                ));
            }
            match target {
                RecoveryTarget::Latest => {}
                RecoveryTarget::Lsn(limit) if limit < checkpoint.checkpoint_lsn => {
                    return Err(Error::CorruptWal(
                        "requested recovery target is older than checkpoint base",
                    ));
                }
                RecoveryTarget::Csn(limit) if limit < tx_status.published_csn => {
                    return Err(Error::CorruptWal(
                        "requested recovery target is older than checkpoint base",
                    ));
                }
                RecoveryTarget::Lsn(_) | RecoveryTarget::Csn(_) => {}
            }
            for (tx_id, csn) in tx_status.entries {
                txs.publish_recovered_commit(tx_id, csn);
            }
            txs.restore_frontier(
                tx_status.next_tx,
                tx_status.next_csn,
                tx_status.published_csn,
            );
            checkpoint.checkpoint_lsn
        } else {
            // LSN sentinel: legit init. No checkpoint exists yet, so recovery
            // replays the entire WAL starting from the very beginning.
            Lsn::ZERO
        };
        recover_index_page_images(&scan_report.records, replay_from_lsn, target, &buffer)?;
        let metrics = recover_heap(&scan_report.records, replay_from_lsn, target, &txs, &heap)?;
        let recovered_catalog = recover_catalog_snapshot(&scan_report.records, target)?;
        let catalog = CatalogManager::new(recovered_catalog.unwrap_or(initial_catalog));
        let phase11_counters = Arc::new(Phase11Counters::default());
        let locks = RowLockManager::new(config.lock_shards, config.busy_timeout);
        // Wave 1A-F: same telemetry pipe on the open path.
        locks.set_phase11_counters(Arc::clone(&phase11_counters));
        wal.set_phase11_counters(Arc::clone(&phase11_counters));
        let commit_durability_live = std::sync::atomic::AtomicU8::new(
            commit_durability_initial_u8(config.commit_durability),
        );
        let engine = Arc::new(Self {
            config: config.clone(),
            commit_durability_live,
            volatile: false,
            data_path: page_path,
            wal_dir,
            rel_id: config.rel_id,
            txs,
            buffer: Arc::clone(&buffer),
            heap,
            catalog,
            catalog_store,
            locks,
            wal,
            phase11_counters,
            control,
            tx_status_store,
            checkpoint: std::sync::Mutex::new(checkpoint),
            index_handles: std::sync::Mutex::new(HashMap::new()),
        });
        engine.rehydrate_index_handles()?;
        recover_indexes(&scan_report.records, replay_from_lsn, target, &engine)?;
        if checkpoint.is_some() {
            let page_count = engine.heap.page_count()?;
            engine.heap.load_row_directory_from_pages(page_count)?;
            engine.heap.load_reusable_pages_from_pages(page_count)?;
        }
        Ok((
            engine,
            RecoveryReport::from_scan(scan_report, metrics, replay_from_lsn),
        ))
    }
}

fn catalog_sync_policy(config: &EngineConfig) -> CatalogSyncPolicy {
    match config.commit_durability {
        // A6-b: Normal durability means write schema changes but do NOT
        // fsync them — matching the WAL-commit policy.  Only Strict
        // requires a catalog fsync for power-failure durability.
        CommitDurability::Strict => CatalogSyncPolicy::Durable,
        CommitDurability::Normal | CommitDurability::UnsafeDev => CatalogSyncPolicy::Volatile,
    }
}

fn flush_wal_on_shutdown(commit_durability: CommitDurability) -> bool {
    !matches!(commit_durability, CommitDurability::UnsafeDev)
}

/// Encode the open-time `CommitDurability` to the u8 representation used by
/// `Engine::commit_durability_live`. Kept private to the kernel so the
/// `Engine::commit_durability` / `set_commit_durability` accessors are the
/// only public surface.
fn commit_durability_initial_u8(durability: CommitDurability) -> u8 {
    match durability {
        CommitDurability::Strict => 0,
        CommitDurability::Normal => 1,
        CommitDurability::UnsafeDev => 2,
    }
}

fn recover_heap(
    records: &[WalRecord],
    replay_from_lsn: Lsn,
    target: RecoveryTarget,
    txs: &ConcurrentTxStatus,
    heap: &PageBackedHeap,
) -> Result<RecoveryMetrics> {
    let mut committed = HashMap::new();
    let mut metrics = RecoveryMetrics::default();
    for record in records {
        if record.kind == WalRecordKind::Commit {
            match WalPayload::decode(&record.payload)? {
                WalPayload::Commit { tx_id, csn } => {
                    if commit_visible(record.lsn, csn, target) {
                        committed.insert(tx_id, csn);
                        txs.publish_recovered_commit(tx_id, csn);
                        metrics.commits_recovered += 1;
                    }
                }
                _ => return Err(Error::CorruptWal("commit record has non-commit payload")),
            }
        }
    }

    for record in records {
        if record.lsn < replay_from_lsn {
            continue;
        }
        if matches!(target, RecoveryTarget::Lsn(limit) if record.lsn >= limit) {
            continue;
        }
        if record.kind == WalRecordKind::Commit {
            continue;
        }
        match WalPayload::decode(&record.payload)? {
            WalPayload::PageImage {
                page_id: _,
                page_lsn: _,
                page_bytes,
            } if committed.contains_key(&record.tx_id) => {
                let page = Page::from_bytes(page_bytes)?;
                if page.header()?.kind == crate::format::PageKind::Heap {
                    heap.redo_page_image(page, record_end_lsn(record))?;
                    metrics.page_images_redone += 1;
                }
            }
            WalPayload::PageImage { .. } => {}
            WalPayload::HeapInsert {
                tx_id,
                rel_id,
                row_id,
                payload,
            } if record.kind == WalRecordKind::PageDelta && committed.contains_key(&tx_id) => {
                heap.insert_recovered_for_relation(tx_id, rel_id, row_id, payload)?;
                metrics.legacy_mutations_redone += 1;
            }
            WalPayload::HeapUpdate {
                tx_id,
                rel_id,
                row_id,
                payload,
            } if record.kind == WalRecordKind::PageDelta && committed.contains_key(&tx_id) => {
                heap.update_recovered_for_relation(tx_id, rel_id, row_id, payload)?;
                metrics.legacy_mutations_redone += 1;
            }
            WalPayload::HeapDelete {
                tx_id,
                rel_id,
                row_id,
            } if record.kind == WalRecordKind::PageDelta && committed.contains_key(&tx_id) => {
                heap.delete_recovered_for_relation(tx_id, rel_id, row_id)?;
                metrics.legacy_mutations_redone += 1;
            }
            WalPayload::HeapInsert { .. }
            | WalPayload::HeapUpdate { .. }
            | WalPayload::HeapDelete { .. } => {}
            WalPayload::IndexInsert { .. } | WalPayload::IndexDelete { .. } => {}
            WalPayload::SegmentSeal { .. }
            | WalPayload::BackupBegin { .. }
            | WalPayload::BackupEnd { .. }
            | WalPayload::TimelineFork { .. }
            | WalPayload::LogicalTxn { .. }
            | WalPayload::CatalogSnapshot { .. } => {}
            // WS-A6 multi-writer hot-row: the coordinator emits this
            // record alongside the per-batch HeapUpdate, so recovery's
            // heap-state reconstruction comes from the HeapUpdate path
            // above. The CombinedSemanticDelta serves as an audit /
            // observability marker (batched_count = how many original
            // UPDATEs the coordinator merged); we accept and decode it
            // here for forward-compat but do not re-apply.
            WalPayload::CombinedSemanticDelta { .. } => {}
            WalPayload::Commit { .. } => unreachable!("commit records are skipped above"),
        }
    }

    Ok(metrics)
}

fn recover_index_page_images(
    records: &[WalRecord],
    replay_from_lsn: Lsn,
    target: RecoveryTarget,
    buffer: &Arc<BufferPool>,
) -> Result<()> {
    let mut committed = HashSet::new();
    for record in records {
        if record.kind == WalRecordKind::Commit
            && let WalPayload::Commit { tx_id, csn } = WalPayload::decode(&record.payload)?
            && commit_visible(record.lsn, csn, target)
        {
            committed.insert(tx_id);
        }
    }

    for record in records {
        if record.lsn < replay_from_lsn {
            continue;
        }
        if matches!(target, RecoveryTarget::Lsn(limit) if record.lsn >= limit) {
            continue;
        }
        if record.kind == WalRecordKind::Commit || !committed.contains(&record.tx_id) {
            continue;
        }
        if let WalPayload::PageImage {
            page_id: _,
            page_lsn: _,
            page_bytes,
        } = WalPayload::decode(&record.payload)?
        {
            let page = Page::from_bytes(page_bytes)?;
            match page.header()?.kind {
                crate::format::PageKind::BtreeMeta
                | crate::format::PageKind::BtreeLeaf
                | crate::format::PageKind::BtreeInternal => {
                    buffer.write_page_direct(&page)?;
                    if let Ok(guard) = buffer.pin(page.header()?.page_id) {
                        guard.with_page_mut(|resident| {
                            *resident = page.clone();
                            Ok(())
                        })?;
                        guard.mark_dirty(page.header()?.page_lsn)?;
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn recover_catalog_snapshot(
    records: &[WalRecord],
    target: RecoveryTarget,
) -> Result<Option<Arc<crate::catalog::SchemaSnapshot>>> {
    let mut committed = HashSet::new();
    for record in records {
        if record.kind == WalRecordKind::Commit
            && let WalPayload::Commit { tx_id, csn } = WalPayload::decode(&record.payload)?
            && commit_visible(record.lsn, csn, target)
        {
            committed.insert(tx_id);
        }
    }

    let mut latest: Option<(Lsn, Arc<crate::catalog::SchemaSnapshot>)> = None;
    for record in records {
        if record.kind != WalRecordKind::Logical || !committed.contains(&record.tx_id) {
            continue;
        }
        let WalPayload::CatalogSnapshot {
            tx_id: _,
            schema_epoch: _,
            snapshot,
        } = WalPayload::decode(&record.payload)?
        else {
            continue;
        };
        let snapshot = Arc::new(crate::catalog::decode_snapshot(&snapshot)?);
        match &latest {
            Some((lsn, _)) if *lsn >= record.lsn => {}
            _ => latest = Some((record.lsn, snapshot)),
        }
    }

    Ok(latest.map(|(_, snapshot)| snapshot))
}

fn recover_indexes(
    records: &[WalRecord],
    replay_from_lsn: Lsn,
    target: RecoveryTarget,
    engine: &Arc<Engine>,
) -> Result<()> {
    let mut committed = HashSet::new();
    for record in records {
        if record.kind == WalRecordKind::Commit
            && let WalPayload::Commit { tx_id, csn } = WalPayload::decode(&record.payload)?
            && commit_visible(record.lsn, csn, target)
        {
            committed.insert(tx_id);
        }
    }

    for record in records {
        if record.lsn < replay_from_lsn {
            continue;
        }
        if matches!(target, RecoveryTarget::Lsn(limit) if record.lsn >= limit) {
            continue;
        }
        if record.kind == WalRecordKind::Commit {
            continue;
        }
        match WalPayload::decode(&record.payload)? {
            WalPayload::PageImage {
                page_id: _,
                page_lsn: _,
                page_bytes,
            } if committed.contains(&record.tx_id) => {
                let page = Page::from_bytes(page_bytes)?;
                match page.header()?.kind {
                    crate::format::PageKind::BtreeMeta
                    | crate::format::PageKind::BtreeLeaf
                    | crate::format::PageKind::BtreeInternal => {
                        engine.heap.redo_page_image(page, record_end_lsn(record))?;
                    }
                    _ => {}
                }
            }
            WalPayload::PageImage { .. } => {}
            WalPayload::IndexInsert {
                tx_id,
                index_id,
                logical_key,
                row,
            } if record.kind == WalRecordKind::PageDelta && committed.contains(&tx_id) => {
                let handles = engine
                    .index_handles
                    .lock()
                    .map_err(|_| Error::CorruptPage("engine index handles mutex poisoned"))?;
                if let Some(handle) = handles.get(&CatalogIndexId(index_id)) {
                    handle.insert_recovered_tx(tx_id, &logical_key, row, record_end_lsn(record))?;
                }
            }
            WalPayload::IndexDelete {
                tx_id,
                index_id,
                logical_key,
                row,
            } if record.kind == WalRecordKind::PageDelta && committed.contains(&tx_id) => {
                let handles = engine
                    .index_handles
                    .lock()
                    .map_err(|_| Error::CorruptPage("engine index handles mutex poisoned"))?;
                if let Some(handle) = handles.get(&CatalogIndexId(index_id)) {
                    handle.delete_mark_recovered_tx(
                        tx_id,
                        &logical_key,
                        row,
                        record_end_lsn(record),
                    )?;
                }
            }
            WalPayload::HeapInsert { .. }
            | WalPayload::HeapUpdate { .. }
            | WalPayload::HeapDelete { .. }
            | WalPayload::IndexInsert { .. }
            | WalPayload::IndexDelete { .. }
            | WalPayload::Commit { .. }
            | WalPayload::SegmentSeal { .. }
            | WalPayload::BackupBegin { .. }
            | WalPayload::BackupEnd { .. }
            | WalPayload::TimelineFork { .. }
            | WalPayload::LogicalTxn { .. }
            | WalPayload::CatalogSnapshot { .. }
            | WalPayload::CombinedSemanticDelta { .. } => {}
        }
    }
    Ok(())
}

fn commit_visible(record_lsn: Lsn, csn: Csn, target: RecoveryTarget) -> bool {
    match target {
        RecoveryTarget::Latest => true,
        RecoveryTarget::Lsn(limit) => record_lsn < limit,
        RecoveryTarget::Csn(limit) => csn <= limit,
    }
}

fn record_end_lsn(record: &WalRecord) -> Lsn {
    Lsn(record.lsn.0 + record.encoded_len() as u64)
}
