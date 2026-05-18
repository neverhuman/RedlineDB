use std::sync::Arc;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::thread;
use std::time::Duration;

use crate::format::{Csn, Lsn, TxId};
use crate::telemetry::{Phase11Counters, phase11_bucket_index};
use crate::wal::WAL_HEADER_LEN;
use crate::wal::{WalPayload, WalRecord, WalRecordKind};
use crate::{Error, Result};

use super::*;

impl WalCoordinator {
    pub fn create(path: impl AsRef<Path>, config: WalConfig) -> Result<Self> {
        let dir = path.as_ref().to_path_buf();
        let wal = WalManager::create(&dir, config.clone())?;
        Self::new(dir, wal, config)
    }

    pub fn open(path: impl AsRef<Path>, config: WalConfig) -> Result<Self> {
        let dir = path.as_ref().to_path_buf();
        let wal = WalManager::open(&dir, config.clone())?;
        Self::new(dir, wal, config)
    }

    fn new(dir: PathBuf, mut wal: WalManager, config: WalConfig) -> Result<Self> {
        let reserved_lsn = wal.written_lsn();
        let prev_lsn = wal.prev_lsn;
        let durable_lsn = wal.durable_lsn();
        let shared = Arc::new(WalCoordinatorShared {
            state: Mutex::new(WalCoordinatorState {
                reserved_lsn,
                written_lsn: reserved_lsn,
                prev_lsn,
                durable_lsn,
                pending: VecDeque::new(),
                pending_bytes: 0,
                flush_requested_lsn: Lsn::ZERO,
                shutdown: false,
                failure: None,
            }),
            cvar: Condvar::new(),
            phase11: std::sync::RwLock::new(None),
        });
        // Lane BH P1 #7: hand the manager an `Arc<WalSyncCounters>`
        // before it moves into the writer thread; the coordinator
        // keeps a clone for `sync_counters_snapshot`.
        let sync_counters = Arc::new(WalSyncCounters::default());
        wal.sync_counters = Some(Arc::clone(&sync_counters));
        let writer_shared = Arc::clone(&shared);
        let writer_config = config.clone();
        let writer = thread::Builder::new()
            .name("redlinedb-wal-writer".to_owned())
            .spawn(move || wal_writer_loop(wal, writer_config, writer_shared))?;
        Ok(Self {
            shared,
            writer: Mutex::new(Some(writer)),
            config,
            dir,
            sync_counters,
        })
    }

    pub fn append(&self, kind: WalRecordKind, tx_id: TxId, payload: Vec<u8>) -> Result<WalAppend> {
        self.append_with_payload(kind, tx_id, payload)
    }

    pub fn append_commit(
        &self,
        tx_id: TxId,
        reserve_csn: impl FnOnce() -> Csn,
    ) -> Result<(Csn, WalAppend)> {
        let encoded_len = WAL_HEADER_LEN
            .checked_add(17)
            .ok_or(Error::CorruptWal("record length overflow"))?;
        let mut state = self.wait_for_wal_buffer(encoded_len)?;
        let csn = reserve_csn();
        let payload = WalPayload::Commit { tx_id, csn }.encode()?;
        let append = enqueue_reserved_record(
            &mut state,
            self.config.segment_bytes,
            WalRecordKind::Commit,
            tx_id,
            payload,
            encoded_len as u64,
        )?;
        self.shared.cvar.notify_all();
        Ok((csn, append))
    }

    pub fn append_commit_with_csn(&self, tx_id: TxId, csn: Csn) -> Result<WalAppend> {
        let encoded_len = WAL_HEADER_LEN
            .checked_add(17)
            .ok_or(Error::CorruptWal("record length overflow"))?;
        let mut state = self.wait_for_wal_buffer(encoded_len)?;
        let payload = WalPayload::Commit { tx_id, csn }.encode()?;
        let append = enqueue_reserved_record(
            &mut state,
            self.config.segment_bytes,
            WalRecordKind::Commit,
            tx_id,
            payload,
            encoded_len as u64,
        )?;
        self.shared.cvar.notify_all();
        Ok(append)
    }

    pub fn flush_until(&self, target_lsn: Lsn) -> Result<Lsn> {
        // Lane E failpoint: coordinator entry to the durability barrier; any
        // injection here aborts before the writer thread is signalled.
        crate::fail_point!("wal::flush_until");
        let mut state = self
            .shared
            .state
            .lock()
            .map_err(|_| Error::CorruptWal("wal coordinator mutex poisoned"))?;

        loop {
            check_wal_failure(&state)?;
            if state.durable_lsn >= target_lsn {
                return Ok(state.durable_lsn);
            }

            if target_lsn > state.flush_requested_lsn {
                state.flush_requested_lsn = target_lsn;
                self.shared.cvar.notify_all();
            }

            state = self
                .shared
                .cvar
                .wait(state)
                .map_err(|_| Error::CorruptWal("wal coordinator wait poisoned"))?;
        }
    }

    pub fn write_until(&self, target_lsn: Lsn) -> Result<Lsn> {
        let mut state = self
            .shared
            .state
            .lock()
            .map_err(|_| Error::CorruptWal("wal coordinator mutex poisoned"))?;

        loop {
            check_wal_failure(&state)?;
            if state.written_lsn >= target_lsn {
                return Ok(state.written_lsn);
            }
            self.shared.cvar.notify_all();
            state = self
                .shared
                .cvar
                .wait(state)
                .map_err(|_| Error::CorruptWal("wal coordinator wait poisoned"))?;
        }
    }

    pub fn flush_all(&self) -> Result<Lsn> {
        // Lane E failpoint: full-WAL flush is the checkpoint barrier; injection
        // here lets harnesses crash between commit-fsync and checkpoint-fsync.
        crate::fail_point!("wal::flush_all");
        let written_lsn = self
            .shared
            .state
            .lock()
            .map_err(|_| Error::CorruptWal("wal coordinator mutex poisoned"))?
            .reserved_lsn;
        self.flush_until(written_lsn)
    }

    pub fn written_lsn(&self) -> Result<Lsn> {
        self.shared
            .state
            .lock()
            .map(|state| state.written_lsn)
            .map_err(|_| Error::CorruptWal("wal coordinator mutex poisoned"))
    }

    pub fn durable_lsn(&self) -> Result<Lsn> {
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

    fn append_with_payload(
        &self,
        kind: WalRecordKind,
        tx_id: TxId,
        payload: Vec<u8>,
    ) -> Result<WalAppend> {
        let encoded_len = WAL_HEADER_LEN
            .checked_add(payload.len())
            .ok_or(Error::CorruptWal("record length overflow"))?;
        if encoded_len as u64 > self.config.segment_bytes {
            return Err(Error::CorruptWal("record larger than wal segment"));
        }
        if encoded_len > self.config.wal_buffer_bytes {
            return Err(Error::CorruptWal("record larger than wal buffer"));
        }

        let mut state = self.wait_for_wal_buffer(encoded_len)?;
        let append = enqueue_reserved_record(
            &mut state,
            self.config.segment_bytes,
            kind,
            tx_id,
            payload,
            encoded_len as u64,
        )?;
        self.shared.cvar.notify_all();
        Ok(append)
    }

    fn wait_for_wal_buffer(
        &self,
        encoded_len: usize,
    ) -> Result<std::sync::MutexGuard<'_, WalCoordinatorState>> {
        if encoded_len as u64 > self.config.segment_bytes {
            return Err(Error::CorruptWal("record larger than wal segment"));
        }
        if encoded_len > self.config.wal_buffer_bytes {
            return Err(Error::CorruptWal("record larger than wal buffer"));
        }

        let mut state = self
            .shared
            .state
            .lock()
            .map_err(|_| Error::CorruptWal("wal coordinator mutex poisoned"))?;

        loop {
            check_wal_failure(&state)?;
            let available = self
                .config
                .wal_buffer_bytes
                .saturating_sub(state.pending_bytes);
            if available >= encoded_len {
                return Ok(state);
            }
            state = self
                .shared
                .cvar
                .wait(state)
                .map_err(|_| Error::CorruptWal("wal coordinator wait poisoned"))?;
        }
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

fn wal_writer_loop(mut wal: WalManager, config: WalConfig, shared: Arc<WalCoordinatorShared>) {
    // Lane GC: track records and bytes that have been written but
    // not yet fsynced, so the histogram bump after `wal.flush()`
    // captures the *exact* batch size covered by that one syscall.
    // Reset on every successful group fsync.
    let mut group_records: u64 = 0;
    let mut group_bytes: u64 = 0;
    loop {
        let mut batch = Vec::new();
        let mut flush_target = Lsn::ZERO;
        let mut should_flush = false;
        let shutdown;

        {
            let mut state = match shared.state.lock() {
                Ok(state) => state,
                Err(_) => return,
            };

            while state.pending.is_empty()
                && state.flush_requested_lsn <= state.durable_lsn
                && !state.shutdown
            {
                state = match shared.cvar.wait(state) {
                    Ok(state) => state,
                    Err(_) => return,
                };
            }

            if state.shutdown && state.pending.is_empty() {
                return;
            }

            let mut batch_bytes = 0_usize;
            while let Some(record) = state.pending.pop_front() {
                batch_bytes += record.encoded.len();
                state.pending_bytes = state.pending_bytes.saturating_sub(record.encoded.len());
                batch.push(record);
                if batch_bytes >= config.wal_write_batch_bytes.max(1) {
                    break;
                }
            }

            if state.flush_requested_lsn > state.durable_lsn {
                flush_target = state.flush_requested_lsn;
                should_flush = true;
            }
            shutdown = state.shutdown;
            shared.cvar.notify_all();
        }

        let mut last_written = Lsn::ZERO;
        for record in batch {
            last_written = record.append.end_lsn;
            // Lane GC: accumulate before the write so a torn-write
            // failpoint doesn't desync the counter from durable
            // state — the failure path returns immediately.
            group_records = group_records.saturating_add(1);
            group_bytes = group_bytes.saturating_add(record.encoded.len() as u64);
            if let Err(_err) = wal.write_encoded(record.append, &record.encoded) {
                publish_wal_failure(&shared);
                return;
            }
        }
        if last_written != Lsn::ZERO {
            publish_written_lsn(&shared, last_written);
        }

        if should_flush {
            wait_for_group_commit_window(&shared, &config, &mut wal, flush_target);
            // Wave 1A-F: re-sample `flush_requested_lsn` after the
            // group-commit window. Late-arriving commits within the
            // same fdatasync interval are now folded into this train
            // so they don't have to wait for the next sync. The
            // widening MUST happen before `wal.flush()` so durability
            // is preserved: every commit whose LSN <= the post-resample
            // target lands on disk before the corresponding writer is
            // told the commit succeeded. We do not extend the window —
            // just one re-sample, then sync.
            flush_target = resample_flush_target(&shared, flush_target);
            // Lane GC: drain_until may pop & write further records
            // that share this fsync; it returns the count and bytes
            // it wrote so we attribute them to the same group.
            match drain_until(
                &shared,
                &mut wal,
                flush_target,
                config.wal_write_batch_bytes,
            ) {
                Ok(drained) => {
                    group_records = group_records.saturating_add(drained.records);
                    group_bytes = group_bytes.saturating_add(drained.bytes);
                }
                Err(_err) => {
                    publish_wal_failure(&shared);
                    return;
                }
            }
            match wal.flush() {
                Ok(durable_lsn) => {
                    // Lane GC: bump group-commit telemetry on the
                    // shared counters before resetting locals. Skip
                    // empty drains (latency-only flushes) so the
                    // mean-fan-in stat stays meaningful.
                    if let Some(counters) = wal.sync_counters.as_ref()
                        && group_records > 0
                    {
                        counters.record_group_commit(group_records, group_bytes);
                    }
                    // Wave 1A-F: bump Phase 11 wal_batch_size_buckets
                    // with the per-fdatasync record count. Same shape
                    // as `group_commit_batch_buckets` so the paper
                    // figs can reuse the existing bucketing.
                    bump_phase11_wal_batch(&shared, group_records);
                    group_records = 0;
                    group_bytes = 0;
                    if let Ok(mut state) = shared.state.lock() {
                        state.durable_lsn = durable_lsn;
                        if state.flush_requested_lsn <= durable_lsn {
                            state.flush_requested_lsn = Lsn::ZERO;
                        }
                        shared.cvar.notify_all();
                    } else {
                        return;
                    }
                }
                Err(_err) => {
                    publish_wal_failure(&shared);
                    return;
                }
            }
        } else if shutdown && last_written != Lsn::ZERO {
            match wal.flush() {
                Ok(durable_lsn) => {
                    // Lane GC: shutdown drain still counts as a
                    // group commit if it actually fsynced records.
                    if let Some(counters) = wal.sync_counters.as_ref()
                        && group_records > 0
                    {
                        counters.record_group_commit(group_records, group_bytes);
                    }
                    bump_phase11_wal_batch(&shared, group_records);
                    group_records = 0;
                    group_bytes = 0;
                    if let Ok(mut state) = shared.state.lock() {
                        state.durable_lsn = durable_lsn;
                        shared.cvar.notify_all();
                    } else {
                        return;
                    }
                }
                Err(_err) => {
                    publish_wal_failure(&shared);
                    return;
                }
            }
        }
    }
}

/// Wave 1A-F: re-sample `flush_requested_lsn` immediately before the
/// fdatasync. Late-arriving commits that landed during the
/// `wait_for_group_commit_window` delay must already be visible in
/// `state.flush_requested_lsn` (they bumped it via `flush_until`), so
/// reading it under the state mutex and widening `flush_target` is
/// sufficient — no scan of `pending` needed. Returns the maximum of
/// the original and the freshly sampled target so the widening is
/// monotonic.
fn resample_flush_target(shared: &Arc<WalCoordinatorShared>, current: Lsn) -> Lsn {
    if let Ok(state) = shared.state.lock()
        && state.flush_requested_lsn > current
    {
        return state.flush_requested_lsn;
    }
    current
}

/// Wave 1A-F: bump `Phase11Counters::wal_batch_size_buckets` with the
/// number of records covered by the most recent fdatasync. Skips empty
/// flushes so latency-only syncs do not skew the histogram.
fn bump_phase11_wal_batch(shared: &Arc<WalCoordinatorShared>, record_count: u64) {
    if record_count == 0 {
        return;
    }
    if let Ok(slot) = shared.phase11.read()
        && let Some(counters) = slot.as_ref()
    {
        let bucket = phase11_bucket_index(record_count);
        counters.wal_batch_size_buckets[bucket].fetch_add(1, AtomicOrdering::Relaxed);
    }
}

fn wait_for_group_commit_window(
    shared: &Arc<WalCoordinatorShared>,
    config: &WalConfig,
    wal: &mut WalManager,
    flush_target: Lsn,
) {
    let delay = Duration::from_micros(config.group_commit_delay_us);
    if delay.is_zero() {
        return;
    }
    let durable = wal.durable_lsn().0;
    if flush_target.0.saturating_sub(durable) >= config.group_commit_max_batch_bytes {
        return;
    }
    if let Ok(state) = shared.state.lock() {
        let _ = shared.cvar.wait_timeout(state, delay);
    }
}

/// Lane GC: per-call accounting for [`drain_until`]. The writer
/// loop adds these into the in-flight group totals so the histogram
/// bump after `wal.flush()` reflects the real fan-in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct DrainCounts {
    records: u64,
    bytes: u64,
}

fn drain_until(
    shared: &Arc<WalCoordinatorShared>,
    wal: &mut WalManager,
    target_lsn: Lsn,
    max_batch_bytes: usize,
) -> Result<DrainCounts> {
    let mut totals = DrainCounts::default();
    loop {
        if wal.written_lsn() >= target_lsn {
            return Ok(totals);
        }

        let mut batch = Vec::new();
        {
            let mut state = shared
                .state
                .lock()
                .map_err(|_| Error::CorruptWal("wal coordinator mutex poisoned"))?;
            let mut batch_bytes = 0_usize;
            while let Some(record) = state.pending.pop_front() {
                batch_bytes += record.encoded.len();
                state.pending_bytes = state.pending_bytes.saturating_sub(record.encoded.len());
                batch.push(record);
                if batch_bytes >= max_batch_bytes.max(1) {
                    break;
                }
                if batch
                    .last()
                    .map(|record| record.append.end_lsn >= target_lsn)
                    .unwrap_or(false)
                {
                    break;
                }
            }
            shared.cvar.notify_all();
        }

        if batch.is_empty() {
            let mut state = shared
                .state
                .lock()
                .map_err(|_| Error::CorruptWal("wal coordinator mutex poisoned"))?;
            while state.pending.is_empty() && state.failure.is_none() && !state.shutdown {
                state = shared
                    .cvar
                    .wait(state)
                    .map_err(|_| Error::CorruptWal("wal coordinator wait poisoned"))?;
            }
            check_wal_failure(&state)?;
            continue;
        }

        let mut last_written = Lsn::ZERO;
        for record in batch {
            last_written = record.append.end_lsn;
            totals.records = totals.records.saturating_add(1);
            totals.bytes = totals.bytes.saturating_add(record.encoded.len() as u64);
            wal.write_encoded(record.append, &record.encoded)?;
        }
        if last_written != Lsn::ZERO {
            publish_written_lsn(shared, last_written);
        }
    }
}

fn reserve_queued_append(
    reserved_lsn: &mut Lsn,
    segment_bytes: u64,
    encoded_len: u64,
) -> Result<WalAppend> {
    if encoded_len > segment_bytes {
        return Err(Error::CorruptWal("record larger than wal segment"));
    }
    let current_segment = segment_for_lsn(*reserved_lsn, segment_bytes);
    let current_offset = offset_for_lsn(*reserved_lsn, segment_bytes);
    let start_lsn = if current_offset > 0 && current_offset + encoded_len > segment_bytes {
        Lsn(current_segment * segment_bytes)
    } else {
        *reserved_lsn
    };
    let append = WalAppend {
        start_lsn,
        end_lsn: Lsn(start_lsn.0 + encoded_len),
    };
    *reserved_lsn = append.end_lsn;
    Ok(append)
}

fn enqueue_reserved_record(
    state: &mut WalCoordinatorState,
    segment_bytes: u64,
    kind: WalRecordKind,
    tx_id: TxId,
    payload: Vec<u8>,
    encoded_len: u64,
) -> Result<WalAppend> {
    let append = reserve_queued_append(&mut state.reserved_lsn, segment_bytes, encoded_len)?;
    let record = WalRecord {
        lsn: append.start_lsn,
        prev_lsn: state.prev_lsn,
        tx_id,
        kind,
        payload,
    };
    let encoded = record.encode()?;
    state.prev_lsn = append.start_lsn;
    state.pending_bytes += encoded.len();
    state.pending.push_back(QueuedWalRecord { append, encoded });
    Ok(append)
}

fn check_wal_failure(state: &WalCoordinatorState) -> Result<()> {
    if let Some(message) = state.failure {
        return Err(Error::CorruptWal(message));
    }
    Ok(())
}

fn publish_wal_failure(shared: &Arc<WalCoordinatorShared>) {
    if let Ok(mut state) = shared.state.lock() {
        state.failure = Some("wal writer failed");
        shared.cvar.notify_all();
    }
}

fn publish_written_lsn(shared: &Arc<WalCoordinatorShared>, written_lsn: Lsn) {
    if let Ok(mut state) = shared.state.lock() {
        if state.written_lsn < written_lsn {
            state.written_lsn = written_lsn;
        }
        shared.cvar.notify_all();
    }
}
