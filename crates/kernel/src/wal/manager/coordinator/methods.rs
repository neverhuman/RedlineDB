use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::{Condvar, Mutex};
use std::thread;

use crate::format::{Csn, Lsn, TxId};
use crate::wal::WAL_HEADER_LEN;
use crate::wal::{WalPayload, WalRecordKind};
use crate::{Error, Result};

use super::helpers::{check_wal_failure, enqueue_reserved_record};
use super::writer::wal_writer_loop;
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
