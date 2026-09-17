use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use crate::format::Lsn;
use crate::io::{FileSystem, StdFileSystem};
use crate::telemetry::Phase11Counters;
use crate::wal::{WAL_HEADER_LEN, WalRecord};
use crate::{Error, Result};

use super::counters::WalSyncCounters;
use super::*;

const WAL_SEGMENT_EXT: &str = ".wal";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WalAppend {
    pub start_lsn: Lsn,
    pub end_lsn: Lsn,
}

#[derive(Debug)]
pub struct WalManager<Fs: FileSystem = StdFileSystem> {
    pub(crate) dir: PathBuf,
    pub(crate) fs: Fs,
    pub(crate) config: WalConfig,
    pub(crate) active_segment: u64,
    pub(crate) active_offset: u64,
    pub(crate) active_file: Fs::File,
    pub(crate) written_lsn: Lsn,
    pub(crate) durable_lsn: Lsn,
    pub(crate) prev_lsn: Lsn,
    /// Lane BH P1 #7: optional pointer to the coordinator's shared
    /// sync counters; left `None` for raw `WalManager` use (e.g.
    /// recovery scans) and populated when `WalCoordinator::new`
    /// hands ownership of the manager to the writer thread.
    pub(crate) sync_counters: Option<Arc<WalSyncCounters>>,
}

#[derive(Debug)]
pub struct WalCoordinator {
    pub(crate) shared: Arc<WalCoordinatorShared>,
    pub(crate) writer: Mutex<Option<JoinHandle<()>>>,
    pub(crate) config: WalConfig,
    pub(crate) dir: PathBuf,
    pub(crate) volatile: bool,
    /// Lane BH P1 #7: shared counters bumped by the writer thread.
    pub(crate) sync_counters: Arc<WalSyncCounters>,
}

#[derive(Debug)]
pub(crate) struct WalCoordinatorShared {
    pub(crate) state: Mutex<WalCoordinatorState>,
    pub(crate) cvar: Condvar,
    /// Wave 1A-F: optional Phase 11 telemetry sink, installed
    /// post-construction by [`WalCoordinator::set_phase11_counters`].
    /// The writer thread reads this directly (no state-mutex hop) so
    /// it can bump `wal_batch_size_buckets` per fdatasync.
    pub(crate) phase11: std::sync::RwLock<Option<Arc<Phase11Counters>>>,
}

#[derive(Debug)]
pub(crate) struct WalCoordinatorState {
    pub(crate) reserved_lsn: Lsn,
    pub(crate) written_lsn: Lsn,
    pub(crate) prev_lsn: Lsn,
    pub(crate) durable_lsn: Lsn,
    pub(crate) pending: VecDeque<QueuedWalRecord>,
    pub(crate) pending_bytes: usize,
    /// Durable wake predicate for the writer. Semantic-combiner candidates
    /// deliberately remain false until a flush, buffer-pressure write, or a
    /// non-combinable record releases the pending batch.
    pub(crate) write_requested: bool,
    pub(crate) flush_requested_lsn: Lsn,
    pub(crate) shutdown: bool,
    pub(crate) failure: Option<&'static str>,
}

#[derive(Debug)]
pub(crate) struct QueuedWalRecord {
    pub(crate) append: WalAppend,
    pub(crate) encoded: Vec<u8>,
}

#[derive(Debug)]
pub struct WalReader<Fs: FileSystem = StdFileSystem> {
    pub(crate) dir: PathBuf,
    pub(crate) fs: Fs,
    pub(crate) config: WalConfig,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalScanReport {
    pub records: Vec<WalRecord>,
    pub valid_end_lsn: Lsn,
    pub torn_tail: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WalOpenScanSummary {
    pub(crate) valid_end_lsn: Lsn,
    pub(crate) last_record_lsn: Lsn,
}

impl WalScanReport {
    pub(crate) fn open_summary(&self) -> WalOpenScanSummary {
        WalOpenScanSummary {
            valid_end_lsn: self.valid_end_lsn,
            last_record_lsn: self
                .records
                .last()
                .map(|record| record.lsn)
                .unwrap_or(Lsn::ZERO),
        }
    }
}

pub(super) fn validate_config(config: &WalConfig) -> Result<()> {
    if config.segment_bytes < WAL_HEADER_LEN as u64 {
        return Err(Error::CorruptWal("wal segment size too small"));
    }
    if config.wal_buffer_bytes < WAL_HEADER_LEN {
        return Err(Error::CorruptWal("wal buffer too small"));
    }
    if config.wal_write_batch_bytes == 0 {
        return Err(Error::CorruptWal("wal write batch must be nonzero"));
    }
    Ok(())
}

pub(super) fn validate_record_position(
    record: &WalRecord,
    segment: u64,
    offset: u64,
    segment_bytes: u64,
) -> Result<()> {
    let expected = (segment - 1)
        .checked_mul(segment_bytes)
        .and_then(|base| base.checked_add(offset))
        .ok_or(Error::CorruptWal("lsn overflow"))?;
    if record.lsn.0 != expected {
        return Err(Error::CorruptWal(
            "record lsn does not match segment position",
        ));
    }
    Ok(())
}

pub(super) fn segment_for_lsn(lsn: Lsn, segment_bytes: u64) -> u64 {
    lsn.0 / segment_bytes + 1
}

pub(super) fn offset_for_lsn(lsn: Lsn, segment_bytes: u64) -> u64 {
    lsn.0 % segment_bytes
}

pub(super) fn segment_path(dir: &Path, segment: u64) -> PathBuf {
    dir.join(format!("{segment:020}{WAL_SEGMENT_EXT}"))
}

pub(super) fn segment_numbers_on_disk(dir: &Path) -> Result<Vec<u64>> {
    let mut segments = Vec::new();
    if !dir.exists() {
        return Ok(segments);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str()
            && let Some(segment) = parse_segment_name(name)
        {
            segments.push(segment);
        }
    }
    segments.sort_unstable();
    Ok(segments)
}

pub(super) fn parse_segment_name(name: &str) -> Option<u64> {
    let number = name.strip_suffix(WAL_SEGMENT_EXT)?;
    if number.len() != 20 || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    number.parse().ok()
}
