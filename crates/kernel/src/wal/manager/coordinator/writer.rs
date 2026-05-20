use std::sync::Arc;

use crate::format::Lsn;

use super::helpers::{
    bump_phase11_wal_batch, drain_until, publish_wal_failure, publish_written_lsn,
    resample_flush_target, wait_for_group_commit_window,
};
use super::*;

pub(super) fn wal_writer_loop(
    mut wal: WalManager,
    config: WalConfig,
    shared: Arc<WalCoordinatorShared>,
    flush_on_shutdown: bool,
) {
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
        } else if shutdown && flush_on_shutdown && last_written != Lsn::ZERO {
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
