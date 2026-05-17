use std::sync::Arc;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::time::Duration;

use crate::format::Lsn;
use crate::format::TxId;
use crate::telemetry::phase11_bucket_index;
use crate::wal::{WalRecord, WalRecordKind};
use crate::{Error, Result};

use super::*;

/// Lane GC: per-call accounting for [`drain_until`]. The writer
/// loop adds these into the in-flight group totals so the histogram
/// bump after `wal.flush()` reflects the real fan-in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct DrainCounts {
    pub(super) records: u64,
    pub(super) bytes: u64,
}

/// Wave 1A-F: re-sample `flush_requested_lsn` immediately before the
/// fdatasync. Late-arriving commits that landed during the
/// `wait_for_group_commit_window` delay must already be visible in
/// `state.flush_requested_lsn` (they bumped it via `flush_until`), so
/// reading it under the state mutex and widening `flush_target` is
/// sufficient — no scan of `pending` needed. Returns the maximum of
/// the original and the freshly sampled target so the widening is
/// monotonic.
pub(super) fn resample_flush_target(shared: &Arc<WalCoordinatorShared>, current: Lsn) -> Lsn {
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
pub(super) fn bump_phase11_wal_batch(shared: &Arc<WalCoordinatorShared>, record_count: u64) {
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

pub(super) fn wait_for_group_commit_window(
    shared: &Arc<WalCoordinatorShared>,
    config: &WalConfig,
    wal: &mut WalManager,
    flush_target: Lsn,
) {
    let delay = Duration::from_micros(config.group_commit_delay_us);
    if delay.is_zero() {
        return;
    }
    let durable = wal.durable_lsn();
    if flush_target.0.saturating_sub(durable.0) >= config.group_commit_max_batch_bytes {
        return;
    }
    if let Ok(state) = shared.state.lock() {
        let _ = shared.cvar.wait_timeout(state, delay);
    }
}

pub(super) fn drain_until(
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

pub(super) fn reserve_queued_append(
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

pub(super) fn enqueue_reserved_record(
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

pub(super) fn check_wal_failure(state: &WalCoordinatorState) -> Result<()> {
    if let Some(message) = state.failure {
        return Err(Error::CorruptWal(message));
    }
    Ok(())
}

pub(super) fn publish_wal_failure(shared: &Arc<WalCoordinatorShared>) {
    if let Ok(mut state) = shared.state.lock() {
        state.failure = Some("wal writer failed");
        shared.cvar.notify_all();
    }
}

pub(super) fn publish_written_lsn(shared: &Arc<WalCoordinatorShared>, written_lsn: Lsn) {
    if let Ok(mut state) = shared.state.lock() {
        if state.written_lsn < written_lsn {
            state.written_lsn = written_lsn;
        }
        shared.cvar.notify_all();
    }
}
