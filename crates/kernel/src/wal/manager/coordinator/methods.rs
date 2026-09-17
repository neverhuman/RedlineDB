use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::{Condvar, Mutex};
use std::thread;

use crate::format::{Csn, Lsn, TxId};
use crate::wal::WAL_HEADER_LEN;
use crate::wal::{CombinedReplacementValue, WalPayload, WalRecord, WalRecordKind};
use crate::{Error, Result};

use super::helpers::{check_wal_failure, enqueue_reserved_record};
use super::writer::wal_writer_loop;
use super::*;

impl WalCoordinator {
    pub(crate) fn volatile(config: WalConfig) -> Self {
        let shared = Arc::new(WalCoordinatorShared {
            state: Mutex::new(WalCoordinatorState {
                reserved_lsn: Lsn::ZERO,
                written_lsn: Lsn::ZERO,
                prev_lsn: Lsn::ZERO,
                durable_lsn: Lsn::ZERO,
                pending: VecDeque::new(),
                pending_bytes: 0,
                write_requested: false,
                flush_requested_lsn: Lsn::ZERO,
                shutdown: false,
                failure: None,
            }),
            cvar: Condvar::new(),
            phase11: std::sync::RwLock::new(None),
        });
        Self {
            shared,
            writer: Mutex::new(None),
            config,
            dir: PathBuf::new(),
            volatile: true,
            sync_counters: Arc::new(WalSyncCounters::default()),
        }
    }

    pub fn create(path: impl AsRef<Path>, config: WalConfig) -> Result<Self> {
        Self::create_with_shutdown_flush(path, config, true)
    }

    pub(crate) fn create_with_shutdown_flush(
        path: impl AsRef<Path>,
        config: WalConfig,
        flush_on_shutdown: bool,
    ) -> Result<Self> {
        let dir = path.as_ref().to_path_buf();
        let wal = WalManager::create(&dir, config.clone())?;
        Self::new(dir, wal, config, flush_on_shutdown)
    }

    pub fn open(path: impl AsRef<Path>, config: WalConfig) -> Result<Self> {
        Self::open_with_shutdown_flush(path, config, true)
    }

    pub(crate) fn open_with_shutdown_flush(
        path: impl AsRef<Path>,
        config: WalConfig,
        flush_on_shutdown: bool,
    ) -> Result<Self> {
        let dir = path.as_ref().to_path_buf();
        let wal = WalManager::open(&dir, config.clone())?;
        Self::new(dir, wal, config, flush_on_shutdown)
    }

    pub(crate) fn open_with_scan_summary_and_shutdown_flush(
        path: impl AsRef<Path>,
        config: WalConfig,
        summary: WalOpenScanSummary,
        flush_on_shutdown: bool,
    ) -> Result<Self> {
        let dir = path.as_ref().to_path_buf();
        let wal = WalManager::open_with_scan_summary(&dir, config.clone(), summary)?;
        Self::new(dir, wal, config, flush_on_shutdown)
    }

    fn new(
        dir: PathBuf,
        mut wal: WalManager,
        config: WalConfig,
        flush_on_shutdown: bool,
    ) -> Result<Self> {
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
                write_requested: false,
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
            .spawn(move || wal_writer_loop(wal, writer_config, writer_shared, flush_on_shutdown))?;
        Ok(Self {
            shared,
            writer: Mutex::new(Some(writer)),
            config,
            dir,
            volatile: false,
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
        if self.volatile {
            let csn = reserve_csn();
            return Ok((
                csn,
                WalAppend {
                    start_lsn: Lsn::ZERO,
                    end_lsn: Lsn::ZERO,
                },
            ));
        }
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
        state.write_requested = true;
        self.shared.cvar.notify_all();
        Ok((csn, append))
    }

    pub fn append_commit_with_csn(&self, tx_id: TxId, csn: Csn) -> Result<WalAppend> {
        if self.volatile {
            let _ = tx_id;
            return Ok(WalAppend {
                start_lsn: Lsn::ZERO,
                end_lsn: Lsn::ZERO,
            });
        }
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
        state.write_requested = true;
        self.shared.cvar.notify_all();
        Ok(append)
    }

    pub fn flush_until(&self, target_lsn: Lsn) -> Result<Lsn> {
        if self.volatile {
            let _ = target_lsn;
            return Ok(Lsn::ZERO);
        }
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
        if self.volatile {
            let _ = target_lsn;
            return Ok(Lsn::ZERO);
        }
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
            state.write_requested = true;
            self.shared.cvar.notify_all();
            state = self
                .shared
                .cvar
                .wait(state)
                .map_err(|_| Error::CorruptWal("wal coordinator wait poisoned"))?;
        }
    }

    pub fn flush_all(&self) -> Result<Lsn> {
        if self.volatile {
            return Ok(Lsn::ZERO);
        }
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

    fn append_with_payload(
        &self,
        kind: WalRecordKind,
        tx_id: TxId,
        payload: Vec<u8>,
    ) -> Result<WalAppend> {
        if self.volatile {
            let _ = (kind, tx_id, payload);
            return Ok(WalAppend {
                start_lsn: Lsn::ZERO,
                end_lsn: Lsn::ZERO,
            });
        }
        let encoded_len = WAL_HEADER_LEN
            .checked_add(payload.len())
            .ok_or(Error::CorruptWal("record length overflow"))?;
        if encoded_len as u64 > self.config.segment_bytes {
            return Err(Error::CorruptWal("record larger than wal segment"));
        }
        if encoded_len > self.config.wal_buffer_bytes {
            return Err(Error::CorruptWal("record larger than wal buffer"));
        }

        let combined_semantic_delta = if self.config.semantic_combiner {
            decode_combined_semantic_delta(kind, tx_id, &payload)
        } else {
            None
        };

        {
            let mut state = self
                .shared
                .state
                .lock()
                .map_err(|_| Error::CorruptWal("wal coordinator mutex poisoned"))?;
            check_wal_failure(&state)?;
            if let Some(candidate) = combined_semantic_delta.as_ref()
                && let Some(append) =
                    self.try_fold_pending_combined_semantic_delta(&mut state, kind, candidate)?
            {
                return Ok(append);
            }
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
        if combined_semantic_delta.is_none() {
            state.write_requested = true;
            self.shared.cvar.notify_all();
        }
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
            state.write_requested = true;
            self.shared.cvar.notify_all();
            state = self
                .shared
                .cvar
                .wait(state)
                .map_err(|_| Error::CorruptWal("wal coordinator wait poisoned"))?;
        }
    }

    fn try_fold_pending_combined_semantic_delta(
        &self,
        state: &mut WalCoordinatorState,
        kind: WalRecordKind,
        candidate: &CombinedSemanticDeltaParts,
    ) -> Result<Option<WalAppend>> {
        if kind != WalRecordKind::PageDelta {
            return Ok(None);
        }

        let Some(last) = state.pending.back() else {
            return Ok(None);
        };
        let last_record = match WalRecord::decode(&last.encoded) {
            Ok(record) => record,
            Err(_) => return Ok(None),
        };
        if last_record.kind != kind || last_record.tx_id != candidate.tx_id {
            return Ok(None);
        }
        let last_payload =
            match decode_combined_semantic_delta(kind, last_record.tx_id, &last_record.payload) {
                Some(parts) => parts,
                _ => return Ok(None),
            };
        if last_payload.tx_id != candidate.tx_id
            || last_payload.rel_id != candidate.rel_id
            || last_payload.row_id != candidate.row_id
        {
            return Ok(None);
        }

        let merged_payload = merge_combined_semantic_delta_parts(last_payload, candidate.clone());
        let merged_payload_bytes = merged_payload.encode()?;
        let merged_encoded_len = WAL_HEADER_LEN
            .checked_add(merged_payload_bytes.len())
            .ok_or(Error::CorruptWal("record length overflow"))?;
        if merged_encoded_len > self.config.segment_bytes as usize
            || merged_encoded_len > self.config.wal_buffer_bytes
        {
            return Ok(None);
        }

        let old_encoded_len = last.encoded.len();
        let new_pending_bytes = state
            .pending_bytes
            .checked_sub(old_encoded_len)
            .and_then(|value| value.checked_add(merged_encoded_len))
            .ok_or(Error::CorruptWal("pending WAL bytes overflow"))?;
        if new_pending_bytes > self.config.wal_buffer_bytes {
            return Ok(None);
        }

        let start_lsn = last.append.start_lsn;
        if offset_for_lsn(start_lsn, self.config.segment_bytes)
            .saturating_add(merged_encoded_len as u64)
            > self.config.segment_bytes
        {
            return Ok(None);
        }

        let merged_record = WalRecord {
            lsn: start_lsn,
            prev_lsn: last_record.prev_lsn,
            tx_id: candidate.tx_id,
            kind,
            payload: merged_payload_bytes,
        };
        let merged_encoded = merged_record.encode()?;
        let merged_end_lsn = Lsn(start_lsn.0 + merged_encoded.len() as u64);

        let last = state
            .pending
            .back_mut()
            .ok_or(Error::CorruptWal("pending WAL queue unexpectedly empty"))?;
        last.append.end_lsn = merged_end_lsn;
        last.encoded = merged_encoded;
        state.pending_bytes = new_pending_bytes;
        state.reserved_lsn = merged_end_lsn;
        Ok(Some(last.append))
    }
}

#[derive(Clone, Debug)]
struct CombinedSemanticDeltaParts {
    tx_id: TxId,
    rel_id: crate::format::RelId,
    row_id: crate::format::RowId,
    deltas: Vec<(u16, i64)>,
    replacements: Vec<(u16, CombinedReplacementValue)>,
    batched_count: u32,
}

fn decode_combined_semantic_delta(
    kind: WalRecordKind,
    tx_id: TxId,
    payload: &[u8],
) -> Option<CombinedSemanticDeltaParts> {
    if kind != WalRecordKind::PageDelta {
        return None;
    }
    match WalPayload::decode(payload) {
        Ok(WalPayload::CombinedSemanticDelta {
            tx_id: candidate_tx_id,
            rel_id,
            row_id,
            deltas,
            replacements,
            batched_count,
        }) if candidate_tx_id == tx_id => Some(CombinedSemanticDeltaParts {
            tx_id: candidate_tx_id,
            rel_id,
            row_id,
            deltas,
            replacements,
            batched_count,
        }),
        _ => None,
    }
}

fn merge_combined_semantic_delta_parts(
    left: CombinedSemanticDeltaParts,
    right: CombinedSemanticDeltaParts,
) -> WalPayload {
    let mut replacements = left.replacements;
    for (col, value) in right.replacements {
        if let Some((_, slot)) = replacements
            .iter_mut()
            .rev()
            .find(|(existing_col, _)| *existing_col == col)
        {
            *slot = value;
        } else {
            replacements.push((col, value));
        }
    }

    let mut deltas = left.deltas;
    deltas.extend(right.deltas);

    WalPayload::CombinedSemanticDelta {
        tx_id: left.tx_id,
        rel_id: left.rel_id,
        row_id: left.row_id,
        deltas,
        replacements,
        batched_count: left.batched_count.saturating_add(right.batched_count),
    }
}
