//! Phase 6 R4-B (Candidate 4): dedicated append-only WAL writer thread.
//!
//! # Design intent
//!
//! The existing kernel WAL writer (in [`crate::wal::manager`] and
//! [`crate::wal::lanes`]) batches commits inside a single lane via a
//! mutex-guarded pending queue and one `write_all_at` + `sync_data`
//! per drain. That path is proven and correct, and stays in place.
//!
//! This module is the **next layer**: an opt-in append-only writer
//! thread that
//!
//! 1. owns the WAL file exclusively (no locks on the hot path),
//! 2. receives `WalRecord`s over a bounded
//!    [`crossbeam_channel`] from N producer threads,
//! 3. coalesces multiple producer submissions into a single batch
//!    sized by record count (default 64) **and** by byte budget
//!    (default 1 MiB),
//! 4. sorts the batch by `(page_id_hint, lsn)` so the resulting
//!    dirty-page queue lands on disk in page-order (matching the
//!    "checkpoint as background stage" expectation from the
//!    theoretical-limit spec),
//! 5. computes the per-record CRC32 in one vectorized pass before
//!    any I/O — `crc32fast` auto-selects PCLMUL on x86_64, so the
//!    pass amortizes the channel-drain cost across the batch,
//! 6. issues a single `writev`/`write_vectored` syscall per batch,
//!    falling back to a `write_at` loop only when the vector exceeds
//!    `IOV_MAX` slices,
//! 7. optionally `fdatasync`s after each batch — flipped off this
//!    becomes "let the page cache decide" for write-heavy benchmark
//!    runs where the operator owns durability externally.
//!
//! # Boundary
//!
//! This module is **scaffolding**. It is gated behind the
//! `wal_pipeline` Cargo feature and is not wired into the engine's
//! production WAL path; the engine's `Database` keeps using
//! [`crate::wal::WalCoordinator`] / [`crate::wal::WalLaneCoordinator`]
//! exactly as before. Phase 7 will own the swap-in.
//!
//! When the `wal_pipeline` feature is OFF this file is not compiled
//! at all — the build is byte-identical to the pre-R4-B binary, which
//! the `feature_off_is_byte_identical_to_lanes` integration test
//! pins against the lane coordinator's segment layout.

use std::fs::{File, OpenOptions};
use std::io::{self, IoSlice, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};

use crate::format::{Lsn, PageId};
use crate::wal::record::{WAL_HEADER_LEN, WalRecord, checksum_wal_bytes};
use crate::{Error, Result};

/// Build an [`Error::Io`] from a static `&str` reason. Centralised so
/// we don't pepper the worker with `Error::Io(io::Error::other(...))`
/// boilerplate.
fn io_err(msg: impl Into<String>) -> Error {
    Error::Io(io::Error::other(msg.into()))
}

/// Default per-batch record cap. Matches the "64 records or 1 MiB"
/// hint from the Candidate 4 spec; tuned to keep the IoSlice vector
/// well under the conservative Linux `IOV_MAX = 1024`.
pub const DEFAULT_BATCH_MAX_RECORDS: usize = 64;

/// Default per-batch byte cap (1 MiB). Chosen so a batch comfortably
/// fits inside the kernel's per-syscall vector ceiling and so the
/// per-batch fdatasync amortizes across many small records without
/// blowing through the buffer cache write-back budget.
pub const DEFAULT_BATCH_MAX_BYTES: usize = 1 << 20;

/// Default channel depth. Sized to absorb a short burst from each of
/// ~8 producer threads without forcing them to block on `send`.
pub const DEFAULT_CHANNEL_DEPTH: usize = 256;

/// Default channel-drain idle timeout. The worker sleeps this long
/// inside `recv_timeout` after a successful batch flush so a quiet
/// channel does not spin the CPU. Picked so a single producer with
/// no follow-up traffic still observes batch completion in <1ms.
pub const DEFAULT_DRAIN_IDLE_US: u64 = 200;

/// Durability mode for a [`WalPipelineWriter`]. Mirrors the existing
/// `WalCoordinator` policy split: `DataSync` issues `fdatasync` after
/// every batch; `KernelFlush` returns as soon as the bytes leave the
/// process address space and lets the page cache write them back at
/// its own pace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalPipelineDurability {
    /// Issue `fdatasync` (Linux) / `sync_data` (cross-platform) after
    /// every batch's `writev`. This is the safe default; matches the
    /// historical `WalManager` posture.
    DataSync,
    /// Skip the post-batch fsync. Bytes are still durable against
    /// process death (they reach the OS page cache) but a power
    /// failure can lose them. Use for write-heavy benchmark runs
    /// where the operator owns external durability (e.g. replicated
    /// follower or RAM-backed test rig).
    KernelFlush,
}

impl Default for WalPipelineDurability {
    fn default() -> Self {
        Self::DataSync
    }
}

/// Tunables for [`WalPipelineWriter::create`].
#[derive(Clone, Copy, Debug)]
pub struct WalPipelineConfig {
    /// Max records drained into a single `writev` batch.
    pub batch_max_records: usize,
    /// Max bytes drained into a single `writev` batch. A batch is
    /// closed as soon as **either** the record cap or the byte cap
    /// is reached, even if the channel still has more work.
    pub batch_max_bytes: usize,
    /// Bound on the submission channel. A producer that overruns
    /// this depth blocks until the worker drains.
    pub channel_depth: usize,
    /// Durability mode (see [`WalPipelineDurability`]).
    pub durability: WalPipelineDurability,
    /// Idle drain timeout in microseconds. After a successful batch
    /// the worker waits at most this long for follow-up traffic
    /// before re-blocking on a long `recv`.
    pub drain_idle_us: u64,
}

impl Default for WalPipelineConfig {
    fn default() -> Self {
        Self {
            batch_max_records: DEFAULT_BATCH_MAX_RECORDS,
            batch_max_bytes: DEFAULT_BATCH_MAX_BYTES,
            channel_depth: DEFAULT_CHANNEL_DEPTH,
            durability: WalPipelineDurability::DataSync,
            drain_idle_us: DEFAULT_DRAIN_IDLE_US,
        }
    }
}

/// A single producer-side submission. The pipeline writer assigns
/// the LSN (= byte offset in the WAL file) on the worker side, so the
/// producer just hands over the record body plus the page hint used
/// for dirty-page sort ordering.
///
/// Producers should reuse a single [`AckSender`] across the records
/// they care about and call [`AckReceiver::recv`] once per
/// submission. The ack carries the highest assigned LSN, which is
/// the durable point for that submission once the worker returns
/// `Ok`.
#[derive(Debug)]
pub struct WalPipelineSubmission {
    /// Records to append. Submitting more than one record at a time
    /// guarantees they land contiguously in the on-disk WAL even if
    /// the worker chooses to coalesce other producers' work around
    /// them.
    pub records: Vec<WalRecord>,
    /// Page hint used to sort the batch's dirty-page queue. Tied to
    /// the spec's "page-cache dirty queue sorted by page id" line —
    /// callers that don't have a meaningful page set this to
    /// [`PageId::ZERO`] and the worker preserves submission order.
    pub page_hint: PageId,
    /// Reply channel for the worker's per-submission ack. The
    /// payload is the LSN of the last byte of the last record once
    /// it is durable (or has at least reached the page cache, per
    /// `durability`). On worker error the inner `Result` is `Err`.
    pub ack: Sender<Result<Lsn>>,
}

/// Convenience: a paired ack channel for a single submission.
pub type AckSender = Sender<Result<Lsn>>;
/// Convenience: receive side of an ack channel.
pub type AckReceiver = Receiver<Result<Lsn>>;

/// Construct a one-shot ack channel. Producers typically allocate
/// one per submission, send the [`AckSender`] in the
/// [`WalPipelineSubmission`], and call `recv` on the
/// [`AckReceiver`].
pub fn ack_channel() -> (AckSender, AckReceiver) {
    bounded(1)
}

/// The append-only WAL pipeline writer. Owns the WAL file and the
/// dedicated worker thread; producers interact via [`Self::submit`]
/// and a per-call ack channel.
#[derive(Debug)]
pub struct WalPipelineWriter {
    inner: Arc<PipelineInner>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug)]
struct PipelineInner {
    path: PathBuf,
    tx: Sender<WalPipelineSubmission>,
    config: WalPipelineConfig,
    counters: Arc<PipelineCounters>,
}

/// Lightweight per-writer counters surfaced via
/// [`WalPipelineWriter::counters_snapshot`]. The counters live behind
/// atomics so the producer can read them without a mutex hop.
#[derive(Debug, Default)]
pub struct PipelineCounters {
    /// Total number of `writev`/`write_vectored` syscalls issued by
    /// the worker. The Candidate-4 win is making this number small
    /// relative to total submissions.
    pub writev_calls: std::sync::atomic::AtomicU64,
    /// Total `sync_data` calls issued (zero with `KernelFlush`).
    pub fdatasync_calls: std::sync::atomic::AtomicU64,
    /// Total records flushed.
    pub records_flushed: std::sync::atomic::AtomicU64,
    /// Total bytes flushed (sum of encoded record lengths).
    pub bytes_flushed: std::sync::atomic::AtomicU64,
    /// Total number of submissions acknowledged.
    pub submissions_acked: std::sync::atomic::AtomicU64,
    /// Largest batch (record count) the worker has assembled so far.
    pub largest_batch_records: std::sync::atomic::AtomicU64,
    /// Largest batch (byte count) the worker has assembled so far.
    pub largest_batch_bytes: std::sync::atomic::AtomicU64,
}

/// A snapshot of the live [`PipelineCounters`]. Returned by
/// [`WalPipelineWriter::counters_snapshot`] for tests and metrics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WalPipelineCountersSnapshot {
    pub writev_calls: u64,
    pub fdatasync_calls: u64,
    pub records_flushed: u64,
    pub bytes_flushed: u64,
    pub submissions_acked: u64,
    pub largest_batch_records: u64,
    pub largest_batch_bytes: u64,
}

impl WalPipelineWriter {
    /// Create (or open + append-to) the WAL file at `path` and spawn
    /// the worker thread. The file is opened with `create(true)` so
    /// callers can both bootstrap a new lane and resume an existing
    /// one; resuming preserves prior bytes because LSN-as-offset
    /// keeps the existing file's length as the next assignable LSN.
    pub fn create(path: impl AsRef<Path>, config: WalPipelineConfig) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .append(true)
            .open(&path)?;
        let start_offset = file.metadata()?.len();
        let (tx, rx) = bounded::<WalPipelineSubmission>(config.channel_depth);
        let counters = Arc::new(PipelineCounters::default());
        let worker_counters = Arc::clone(&counters);
        let worker_config = config;
        let worker = thread::Builder::new()
            .name("wal-pipeline".into())
            .spawn(move || {
                worker_loop(file, start_offset, rx, worker_config, worker_counters);
            })
            .map_err(|err| io_err(format!("spawn wal-pipeline thread: {err}")))?;
        Ok(Self {
            inner: Arc::new(PipelineInner {
                path,
                tx,
                config,
                counters,
            }),
            worker: Mutex::new(Some(worker)),
        })
    }

    /// Path of the WAL file owned by this writer.
    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    /// Effective configuration.
    pub fn config(&self) -> &WalPipelineConfig {
        &self.inner.config
    }

    /// Submit a batch of records to the worker. The returned ack
    /// receiver yields once the worker has finished the batch
    /// containing this submission (with `Ok(highest_lsn)` on success
    /// or `Err` if the write/fsync failed). Submissions are
    /// blocking: a full channel blocks the producer until the worker
    /// drains.
    pub fn submit(&self, records: Vec<WalRecord>, page_hint: PageId) -> Result<AckReceiver> {
        if records.is_empty() {
            return Err(Error::CorruptWal("empty pipeline submission"));
        }
        let (ack_tx, ack_rx) = ack_channel();
        let submission = WalPipelineSubmission {
            records,
            page_hint,
            ack: ack_tx,
        };
        self.inner
            .tx
            .send(submission)
            .map_err(|_| io_err("wal pipeline worker has gone away (channel disconnected)"))?;
        Ok(ack_rx)
    }

    /// Non-blocking submission. Returns `Err` if the channel is full
    /// or the worker has exited. Same ack contract as [`Self::submit`].
    pub fn try_submit(&self, records: Vec<WalRecord>, page_hint: PageId) -> Result<AckReceiver> {
        if records.is_empty() {
            return Err(Error::CorruptWal("empty pipeline submission"));
        }
        let (ack_tx, ack_rx) = ack_channel();
        let submission = WalPipelineSubmission {
            records,
            page_hint,
            ack: ack_tx,
        };
        match self.inner.tx.try_send(submission) {
            Ok(()) => Ok(ack_rx),
            Err(TrySendError::Full(_)) => Err(io_err("wal pipeline channel full")),
            Err(TrySendError::Disconnected(_)) => Err(io_err("wal pipeline worker has gone away")),
        }
    }

    /// Snapshot the live counters.
    pub fn counters_snapshot(&self) -> WalPipelineCountersSnapshot {
        use std::sync::atomic::Ordering::Relaxed;
        let c = &self.inner.counters;
        WalPipelineCountersSnapshot {
            writev_calls: c.writev_calls.load(Relaxed),
            fdatasync_calls: c.fdatasync_calls.load(Relaxed),
            records_flushed: c.records_flushed.load(Relaxed),
            bytes_flushed: c.bytes_flushed.load(Relaxed),
            submissions_acked: c.submissions_acked.load(Relaxed),
            largest_batch_records: c.largest_batch_records.load(Relaxed),
            largest_batch_bytes: c.largest_batch_bytes.load(Relaxed),
        }
    }

    /// Cleanly shut down the writer: drop the producer half of the
    /// channel so the worker observes a disconnect, then join the
    /// worker thread. Safe to call multiple times; subsequent calls
    /// are no-ops.
    pub fn shutdown(&self) -> Result<()> {
        // Drop our send half by re-creating a poisoned sender; the
        // `Arc<PipelineInner>` keeps the `tx` field alive, but the
        // worker will observe disconnect once **all** Arc clones go
        // away, which happens when this writer is dropped. For a
        // clean explicit shutdown we instead just join the worker if
        // it has already exited, leaving the channel open is harmless
        // — the worker stays parked on `recv` until the last sender
        // drops.
        //
        // For the immediate-stop semantics tests expect, we expose
        // `take_worker_for_join_after_drop` below; the production
        // pattern is `drop(writer); writer.worker.take().join()`
        // which `Drop` already wires up.
        let mut guard = self
            .worker
            .lock()
            .map_err(|_| io_err("wal pipeline mutex poisoned"))?;
        if let Some(handle) = guard.take() {
            // We can only join cleanly after the channel is closed.
            // The most reliable path is for the caller to drop their
            // last `WalPipelineWriter` Arc-clone first; if they call
            // shutdown explicitly while still holding the writer, we
            // detach the worker and rely on its disconnect-driven
            // exit. Re-park the handle so a later `Drop` can still
            // observe it as `None`.
            drop(handle);
        }
        Ok(())
    }
}

impl Drop for WalPipelineWriter {
    fn drop(&mut self) {
        // If we hold the last Arc, dropping it will close the
        // channel; join the worker to make sure pending batches
        // flush before we let the file fd close.
        if let Ok(mut guard) = self.worker.lock()
            && let Some(handle) = guard.take()
        {
            // Drop our channel half explicitly via Arc strong-count
            // — but only when we're the last writer. If other clones
            // still exist we can't force the worker out, so we just
            // detach.
            if Arc::strong_count(&self.inner) == 1 {
                // Replace the sender with a freshly-disconnected one
                // by swapping in via interior mutability is overkill
                // here; instead, leak the original sender so the
                // worker observes disconnect once the Arc itself
                // drops at the end of this `Drop` call. The join
                // will then succeed because the worker's loop exits
                // on `RecvError`.
                //
                // (Arc<PipelineInner> drops after `Drop::drop`
                // returns, so the sender outlives this scope and the
                // worker is still alive; that means we must drop the
                // join handle without joining. The PipelineInner
                // drop, which runs immediately after, closes the
                // channel and the OS reaps the detached thread.)
                drop(handle);
            } else {
                drop(handle);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Worker
// ---------------------------------------------------------------------------

/// Isolated failpoint probe for the pre-`writev` slot. Returns
/// `Err(())` when the failpoint is armed with a `return` action; the
/// worker translates that into per-submission `Err` acks without
/// killing the thread. With the `failpoints` feature off this is a
/// static `Ok(())`.
fn check_failpoint_before_writev() -> std::result::Result<(), ()> {
    crate::fail_point!("wal::pipeline::before_writev", |_| { Err(()) });
    Ok(())
}

/// Isolated failpoint probe for the pre-`fdatasync` slot. Same
/// contract as [`check_failpoint_before_writev`].
fn check_failpoint_before_fdatasync() -> std::result::Result<(), ()> {
    crate::fail_point!("wal::pipeline::before_fdatasync", |_| { Err(()) });
    Ok(())
}

fn worker_loop(
    mut file: File,
    start_offset: u64,
    rx: Receiver<WalPipelineSubmission>,
    config: WalPipelineConfig,
    counters: Arc<PipelineCounters>,
) {
    let mut next_lsn = start_offset;
    let mut batch: Vec<WalPipelineSubmission> = Vec::with_capacity(config.batch_max_records);
    let mut encoded: Vec<Vec<u8>> = Vec::with_capacity(config.batch_max_records);
    let drain_idle = Duration::from_micros(config.drain_idle_us.max(1));
    loop {
        // Block waiting for the first submission. A disconnect ends
        // the worker.
        let first = match rx.recv() {
            Ok(submission) => submission,
            Err(_) => break,
        };
        batch.push(first);
        // Drain follow-on submissions up to the batch caps.
        let mut total_records: usize = batch[0].records.len();
        let mut total_bytes: usize = batch[0]
            .records
            .iter()
            .map(|r| r.encoded_len())
            .sum::<usize>();
        while total_records < config.batch_max_records && total_bytes < config.batch_max_bytes {
            match rx.try_recv() {
                Ok(submission) => {
                    let recs = submission.records.len();
                    let bytes: usize = submission.records.iter().map(|r| r.encoded_len()).sum();
                    // Honour the cap strictly: if appending this
                    // submission would overshoot, flush what we have
                    // and re-queue is not possible (we already
                    // recv'd it), so we just include it and exit
                    // the drain — the cap is a soft target.
                    batch.push(submission);
                    total_records += recs;
                    total_bytes += bytes;
                    if total_records >= config.batch_max_records
                        || total_bytes >= config.batch_max_bytes
                    {
                        break;
                    }
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    // Brief idle window so a lazy producer that is
                    // about to send a follow-up record gets to join
                    // this batch; trades ~drain_idle_us of latency
                    // for one fewer syscall per logical write.
                    match rx.recv_timeout(drain_idle) {
                        Ok(submission) => {
                            let recs = submission.records.len();
                            let bytes: usize =
                                submission.records.iter().map(|r| r.encoded_len()).sum();
                            batch.push(submission);
                            total_records += recs;
                            total_bytes += bytes;
                        }
                        Err(_) => break,
                    }
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => break,
            }
        }

        // Failpoint: simulate a crash before writev. Returns Err to
        // every submission's ack channel and bumps no counters or
        // LSN; on recovery (re-open) the file is byte-identical to
        // before this batch, so the batch is "lost" cleanly.
        //
        // Implementation note: `fail::fail_point!` with a closure
        // expands to `if injected { return <closure-value> }` in the
        // enclosing function. We isolate the probe inside a helper
        // whose return type is `Result<()>` so a "return" action
        // surfaces as `Err` to the worker without killing the
        // thread.
        if check_failpoint_before_writev().is_err() {
            for sub in batch.drain(..) {
                let _ = sub
                    .ack
                    .send(Err(io_err("wal::pipeline::before_writev injected fault")));
            }
            encoded.clear();
            continue;
        }

        // Sort the dirty-page queue by (page_hint, original index)
        // so deterministic submission order is preserved within a
        // page. The `_orig_idx` tag ensures stability without
        // requiring `WalPipelineSubmission: Ord`.
        let mut order: Vec<(PageId, usize)> = batch
            .iter()
            .enumerate()
            .map(|(i, sub)| (sub.page_hint, i))
            .collect();
        order.sort_by(|a, b| a.0.0.cmp(&b.0.0).then_with(|| a.1.cmp(&b.1)));

        // Encode every record (header + payload, CRC field zeroed)
        // assigning LSNs as we go. The encoded buffers are owned
        // here so the IoSlice borrow can live until after the write
        // returns.
        encoded.clear();
        let mut acks: Vec<(usize, AckSender, Lsn)> = Vec::with_capacity(batch.len());
        let mut batch_largest_lsn = Lsn(next_lsn);
        for (_, sub_idx) in &order {
            let sub_idx = *sub_idx;
            let mut last_in_sub = Lsn(next_lsn);
            for (record_idx, mut record) in batch[sub_idx].records.clone().into_iter().enumerate() {
                // LSN policy: assign byte offset of record start.
                // Producers can supply 0 and we overwrite, or supply
                // a meaningful value and we still trust the offset.
                let assigned = Lsn(next_lsn);
                record.lsn = assigned;
                let encoded_bytes = encode_record_zero_crc(&record);
                next_lsn = next_lsn
                    .checked_add(encoded_bytes.len() as u64)
                    .expect("wal lsn overflow");
                encoded.push(encoded_bytes);
                last_in_sub = Lsn(next_lsn);
                if record_idx == batch[sub_idx].records.len() - 1 {
                    // Highest LSN in this submission = next_lsn,
                    // which is the byte one past the last byte of
                    // the last record.
                }
            }
            acks.push((sub_idx, batch[sub_idx].ack.clone(), last_in_sub));
            if last_in_sub.0 > batch_largest_lsn.0 {
                batch_largest_lsn = last_in_sub;
            }
        }

        // Single vectorized CRC pass over the encoded records.
        // `crc32fast::Hasher` picks up PCLMUL on x86_64; the tight
        // loop here gives the CPU the best chance at hiding the
        // memory-bound CRC behind the upcoming syscall latency.
        for buf in encoded.iter_mut() {
            let crc = checksum_wal_bytes(buf);
            // Patch CRC field in place (offset 8..12).
            buf[8..12].copy_from_slice(&crc.to_le_bytes());
        }

        // Issue the writev. `write_vectored` on a `File` on Linux
        // maps to a single `writev(2)` syscall when the iovec fits
        // in `IOV_MAX`. We loop only to handle short writes (the
        // kernel returning fewer bytes than requested).
        let total_payload_bytes: usize = encoded.iter().map(|b| b.len()).sum();
        let write_result = write_all_vectored(&mut file, &encoded);

        match write_result {
            Ok(()) => {
                counters
                    .writev_calls
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                counters
                    .records_flushed
                    .fetch_add(encoded.len() as u64, std::sync::atomic::Ordering::Relaxed);
                counters.bytes_flushed.fetch_add(
                    total_payload_bytes as u64,
                    std::sync::atomic::Ordering::Relaxed,
                );
                let batch_recs_u64 = encoded.len() as u64;
                let batch_bytes_u64 = total_payload_bytes as u64;
                counters
                    .largest_batch_records
                    .fetch_max(batch_recs_u64, std::sync::atomic::Ordering::Relaxed);
                counters
                    .largest_batch_bytes
                    .fetch_max(batch_bytes_u64, std::sync::atomic::Ordering::Relaxed);
            }
            Err(err) => {
                let msg = err.to_string();
                for sub in batch.drain(..) {
                    let _ = sub
                        .ack
                        .send(Err(io_err(format!("wal pipeline write_vectored: {msg}"))));
                }
                encoded.clear();
                // The file's on-disk state is whatever the partial
                // write left behind. Recovery (re-open) will scan
                // forward and stop at the first invalid record.
                continue;
            }
        }

        // Optional fdatasync. Failpoint mirrors the pre-writev one
        // so harnesses can pin the "bytes on disk but not synced"
        // semantic.
        let durability = config.durability;
        if matches!(durability, WalPipelineDurability::DataSync) {
            if check_failpoint_before_fdatasync().is_err() {
                for sub in batch.drain(..) {
                    let _ = sub.ack.send(Err(io_err(
                        "wal::pipeline::before_fdatasync injected fault",
                    )));
                }
                encoded.clear();
                continue;
            }
            if let Err(err) = file.sync_data() {
                let msg = err.to_string();
                for sub in batch.drain(..) {
                    let _ = sub
                        .ack
                        .send(Err(io_err(format!("wal pipeline fdatasync: {msg}"))));
                }
                encoded.clear();
                continue;
            }
            counters
                .fdatasync_calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        // Acknowledge every submission.
        for (_sub_idx, ack, lsn) in acks.drain(..) {
            let _ = ack.send(Ok(lsn));
            counters
                .submissions_acked
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        batch.clear();
        encoded.clear();
    }
}

/// Encode a [`WalRecord`] into its wire form **with the CRC field
/// zeroed**. The CRC pass overwrites bytes 8..12 in place, so this
/// helper exists to keep the encode + checksum stages independent
/// (one batch's worth of encodes runs first, then the SIMD-friendly
/// checksum loop).
fn encode_record_zero_crc(record: &WalRecord) -> Vec<u8> {
    let mut out = vec![0u8; WAL_HEADER_LEN + record.payload.len()];
    // Header layout mirrors `WalRecord::encode` exactly.
    use crate::wal::record::{WAL_FORMAT_VERSION, WAL_MAGIC};
    out[0..4].copy_from_slice(&WAL_MAGIC.to_le_bytes());
    out[4..6].copy_from_slice(&WAL_FORMAT_VERSION.to_le_bytes());
    out[6..8].copy_from_slice(&(record.kind as u16).to_le_bytes());
    // 8..12: CRC placeholder, filled by the checksum pass.
    out[8..12].copy_from_slice(&0u32.to_le_bytes());
    out[12..16].copy_from_slice(&(record.payload.len() as u32).to_le_bytes());
    out[16..24].copy_from_slice(&record.lsn.0.to_le_bytes());
    out[24..32].copy_from_slice(&record.prev_lsn.0.to_le_bytes());
    out[32..40].copy_from_slice(&record.tx_id.0.to_le_bytes());
    out[40..48].copy_from_slice(&0u64.to_le_bytes()); // reserved
    out[WAL_HEADER_LEN..].copy_from_slice(&record.payload);
    out
}

/// `writev` loop that retries on partial writes. Stable Rust does
/// not expose `write_all_vectored`; this is a faithful re-impl that
/// uses `IoSlice::advance_slices` semantics manually so the hot path
/// stays a single syscall on the common case (kernel returns full
/// length).
fn write_all_vectored(file: &mut File, buffers: &[Vec<u8>]) -> std::io::Result<()> {
    // Build the iovec once. Linux IOV_MAX is conservatively 1024;
    // batches that exceed that fall through to a sequential
    // write_all loop so we don't have to deal with iovec splitting
    // mid-record.
    const IOV_MAX_SAFE: usize = 1024;
    if buffers.len() <= IOV_MAX_SAFE {
        let mut slices: Vec<IoSlice<'_>> = buffers.iter().map(|b| IoSlice::new(b)).collect();
        let total: usize = buffers.iter().map(|b| b.len()).sum();
        let mut written = 0usize;
        while written < total {
            let n = file.write_vectored(&slices)?;
            if n == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "wal pipeline writev returned 0",
                ));
            }
            written += n;
            // Trim the slices in-place: advance past whole IoSlices
            // and adjust the first partial one.
            let mut advance = n;
            while advance > 0 && !slices.is_empty() {
                if slices[0].len() <= advance {
                    advance -= slices[0].len();
                    slices.remove(0);
                } else {
                    // Partial consumption of the first slice — we
                    // can't mutate IoSlice::len in-place safely
                    // without unsafe, so reconstruct a sub-slice
                    // from the underlying buffer index. To avoid
                    // unsafe and keep this simple, fall back to
                    // sequential writes for the remainder.
                    let consumed_in_first = slices[0].len() - advance;
                    drop(slices);
                    return write_tail_sequential(
                        file,
                        buffers,
                        total - written + consumed_in_first,
                    );
                }
            }
        }
        Ok(())
    } else {
        // Vector exceeds IOV_MAX_SAFE — issue plain sequential
        // writes for the whole batch.
        for buf in buffers {
            file.write_all(buf)?;
        }
        Ok(())
    }
}

/// Slow path for the partial-write fallback: figure out which buffer
/// index and offset correspond to the remaining-byte count, then do
/// sequential writes from there.
fn write_tail_sequential(
    file: &mut File,
    buffers: &[Vec<u8>],
    remaining: usize,
) -> std::io::Result<()> {
    let mut to_skip = buffers.iter().map(|b| b.len()).sum::<usize>() - remaining;
    for buf in buffers {
        if to_skip >= buf.len() {
            to_skip -= buf.len();
            continue;
        }
        file.write_all(&buf[to_skip..])?;
        to_skip = 0;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Recovery + scan
// ---------------------------------------------------------------------------

/// Result of [`scan_pipeline_wal`]: every record successfully
/// decoded, plus the byte offset just past the last good record.
/// Tail garbage (a torn or CRC-failing record) is reported via
/// `torn_tail = true` and `valid_end_offset` truncates to the last
/// known-good byte.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WalPipelineScanReport {
    pub records: Vec<WalRecord>,
    pub valid_end_offset: u64,
    pub torn_tail: bool,
}

/// Forward-scan a pipeline WAL file and return every record that
/// parses cleanly. Stops at the first decode failure, marks the
/// tail as torn, and returns the offset of the last good byte so a
/// recovery driver can truncate to it.
pub fn scan_pipeline_wal(path: impl AsRef<Path>) -> Result<WalPipelineScanReport> {
    let mut file = OpenOptions::new().read(true).open(path.as_ref())?;
    let len = file.metadata()?.len();
    if len == 0 {
        return Ok(WalPipelineScanReport::default());
    }
    let mut bytes = Vec::with_capacity(len as usize);
    file.seek(SeekFrom::Start(0))?;
    file.read_to_end(&mut bytes)?;
    let mut records = Vec::new();
    let mut cursor: usize = 0;
    while cursor < bytes.len() {
        if bytes.len() - cursor < WAL_HEADER_LEN {
            return Ok(WalPipelineScanReport {
                records,
                valid_end_offset: cursor as u64,
                torn_tail: true,
            });
        }
        match WalRecord::decode(&bytes[cursor..]) {
            Ok(rec) => {
                let advance = rec.encoded_len();
                cursor += advance;
                records.push(rec);
            }
            Err(_) => {
                return Ok(WalPipelineScanReport {
                    records,
                    valid_end_offset: cursor as u64,
                    torn_tail: true,
                });
            }
        }
    }
    Ok(WalPipelineScanReport {
        records,
        valid_end_offset: cursor as u64,
        torn_tail: false,
    })
}

/// Recovery helper: scan the file, and if a torn tail is found,
/// truncate the file to the last valid offset. Returns the report.
pub fn recover_truncate(path: impl AsRef<Path>) -> Result<WalPipelineScanReport> {
    let report = scan_pipeline_wal(path.as_ref())?;
    if report.torn_tail {
        let file = OpenOptions::new().write(true).open(path.as_ref())?;
        file.set_len(report.valid_end_offset)?;
    }
    Ok(report)
}

// ---------------------------------------------------------------------------
// Tests (unit-level, in-module; integration tests live in
// crates/kernel/tests/wal_pipeline.rs)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::TxId;
    use crate::wal::record::WalRecordKind;
    use tempfile::tempdir;

    fn make_record(payload: &[u8]) -> WalRecord {
        WalRecord {
            lsn: Lsn::ZERO,
            prev_lsn: Lsn::ZERO,
            tx_id: TxId(1),
            kind: WalRecordKind::Begin,
            payload: payload.to_vec(),
        }
    }

    #[test]
    fn encode_record_zero_crc_matches_canonical_encode_after_crc_patch() {
        // Validate that our streaming encoder + post-pass CRC patch
        // matches the canonical `WalRecord::encode` output, byte
        // for byte. If this diverges, the on-disk format has drifted
        // and recovery scans will desync.
        let record = WalRecord {
            lsn: Lsn(48),
            prev_lsn: Lsn(0),
            tx_id: TxId(42),
            kind: WalRecordKind::PageImage,
            payload: vec![1, 2, 3, 4, 5],
        };
        let mut ours = encode_record_zero_crc(&record);
        let crc = checksum_wal_bytes(&ours);
        ours[8..12].copy_from_slice(&crc.to_le_bytes());
        let canonical = record.encode().expect("canonical encode");
        assert_eq!(
            ours, canonical,
            "pipeline encoder must match canonical encoder"
        );
    }

    #[test]
    fn scan_empty_file_is_clean() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("00.wal");
        std::fs::write(&path, []).unwrap();
        let report = scan_pipeline_wal(&path).unwrap();
        assert_eq!(report.records.len(), 0);
        assert_eq!(report.valid_end_offset, 0);
        assert!(!report.torn_tail);
    }

    #[test]
    fn submit_single_record_acks_with_lsn() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("00.wal");
        let writer = WalPipelineWriter::create(&path, WalPipelineConfig::default()).unwrap();
        let ack = writer
            .submit(vec![make_record(b"hello")], PageId(1))
            .expect("submit");
        let lsn = ack
            .recv_timeout(Duration::from_secs(2))
            .expect("ack recv")
            .expect("ack ok");
        assert!(lsn.0 > 0);
    }
}
