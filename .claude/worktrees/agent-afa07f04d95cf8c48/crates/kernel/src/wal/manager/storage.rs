use std::path::Path;

use crate::format::bytes::read_u32;
use crate::format::{Lsn, TxId};
use crate::io::{FileHandle, FileSystem, StdFileSystem};
use crate::wal::{WAL_HEADER_LEN, WalRecord, WalRecordKind};
use crate::{Error, Result};

use super::*;

impl WalManager<StdFileSystem> {
    pub fn create(path: impl AsRef<Path>, config: WalConfig) -> Result<Self> {
        Self::create_with_fs(path, config, StdFileSystem)
    }

    pub fn open(path: impl AsRef<Path>, config: WalConfig) -> Result<Self> {
        Self::open_with_fs(path, config, StdFileSystem)
    }

    pub fn prune_segments_below_checkpoint_lsn(&mut self, checkpoint_lsn: Lsn) -> Result<usize> {
        let keep_segment = segment_for_lsn(checkpoint_lsn, self.config.segment_bytes);
        self.prune_segments_below(keep_segment)
    }

    pub fn prune_segments_below(&mut self, segment: u64) -> Result<usize> {
        if segment == 0 {
            return Ok(0);
        }

        // Lane E failpoint: armed before any WAL segment removal so harnesses
        // can crash between checkpoint completion and prune, observing whether
        // recovery still succeeds with stale segments on disk.
        crate::fail_point!("wal::prune");
        let mut removed = 0_usize;
        for candidate in self.segment_numbers()? {
            if candidate < segment && candidate < self.active_segment {
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

impl<Fs: FileSystem> WalManager<Fs> {
    pub fn create_with_fs(path: impl AsRef<Path>, config: WalConfig, fs: Fs) -> Result<Self> {
        validate_config(&config)?;
        let dir = path.as_ref().to_path_buf();
        fs.create_dir_all(&dir)?;
        let active_segment = 1;
        let active_offset = 0;
        let active_file = fs.open_rw_create(&segment_path(&dir, active_segment))?;
        Ok(Self {
            dir,
            fs,
            config,
            active_segment,
            active_offset,
            active_file,
            written_lsn: Lsn::ZERO,
            durable_lsn: Lsn::ZERO,
            prev_lsn: Lsn::ZERO,
            sync_counters: None,
        })
    }

    pub fn open_with_fs(path: impl AsRef<Path>, config: WalConfig, fs: Fs) -> Result<Self> {
        validate_config(&config)?;
        let dir = path.as_ref().to_path_buf();
        fs.create_dir_all(&dir)?;
        let mut scan = WalReader::new_with_fs(&dir, config.clone(), fs);
        let report = scan.scan_report()?;
        let records = report.records;
        let fs = scan.into_fs();

        let written_lsn = records
            .last()
            .map(|record| Lsn(record.lsn.0 + record.encoded_len() as u64))
            .unwrap_or(Lsn::ZERO);
        let prev_lsn = records.last().map(|record| record.lsn).unwrap_or(Lsn::ZERO);
        let active_segment = segment_for_lsn(written_lsn, config.segment_bytes);
        let active_offset = offset_for_lsn(written_lsn, config.segment_bytes);
        let active_file = fs.open_rw_create(&segment_path(&dir, active_segment))?;
        active_file.set_len(active_offset)?;

        Ok(Self {
            dir,
            fs,
            config,
            active_segment,
            active_offset,
            active_file,
            written_lsn,
            durable_lsn: Lsn::ZERO,
            prev_lsn,
            sync_counters: None,
        })
    }

    pub fn append(
        &mut self,
        kind: WalRecordKind,
        tx_id: TxId,
        payload: Vec<u8>,
    ) -> Result<WalAppend> {
        let encoded_len = WAL_HEADER_LEN
            .checked_add(payload.len())
            .ok_or(Error::CorruptWal("record length overflow"))?;
        let append = self.reserve_append(encoded_len as u64)?;
        let record = WalRecord {
            lsn: append.start_lsn,
            prev_lsn: self.prev_lsn,
            tx_id,
            kind,
            payload,
        };
        let encoded = record.encode()?;
        self.write_encoded(append, &encoded)?;
        Ok(append)
    }

    pub fn flush(&mut self) -> Result<Lsn> {
        // Lane E failpoint: armed before the fsync that establishes WAL
        // durability so the harness can simulate "fsync skipped" or kernel
        // crash mid-fsync.
        //
        // The closure form lets the bench-matrix runner inject the
        // `return` action with meaningful semantics: it short-circuits
        // the fsync entirely and reports `Ok(written_lsn)` to the
        // caller, simulating a kernel that *claimed* the WAL was
        // durable while skipping the actual fsync(2). That is the
        // wal-fsync-skipped scenario; the strict gate must catch it
        // by detecting acked rows that are missing post-recovery.
        // `panic`/`abort` actions still kill the thread before
        // `sync_data` runs, exactly like the original site.
        let written = self.written_lsn;
        let _ = written;
        crate::fail_point!("wal::flush", |_| { Ok(written) });
        self.active_file.sync_data()?;
        // Lane BH P1 #7: count fdatasync calls so the bench harness
        // can surface them on Redline rows. The bump only fires when
        // the manager is owned by a `WalCoordinator` (raw recovery
        // scans leave the counter `None`).
        if let Some(counters) = &self.sync_counters {
            counters.bump_fdatasync();
        }
        self.durable_lsn = self.written_lsn;
        Ok(self.durable_lsn)
    }

    pub fn written_lsn(&self) -> Lsn {
        self.written_lsn
    }

    pub fn durable_lsn(&self) -> Lsn {
        self.durable_lsn
    }

    pub fn wal_dir(&self) -> &Path {
        &self.dir
    }

    pub fn write_encoded(&mut self, append: WalAppend, encoded: &[u8]) -> Result<()> {
        // Lane E failpoint: armed before the WAL segment write so harnesses
        // can inject torn-write or panic faults at the precise moment the
        // record would land on disk.
        crate::fail_point!("wal::write_encoded");
        if encoded.len() > self.config.segment_bytes as usize {
            return Err(Error::CorruptWal("record larger than wal segment"));
        }

        let expected_lsn =
            Lsn((self.active_segment - 1) * self.config.segment_bytes + self.active_offset);
        if expected_lsn != append.start_lsn
            && self.active_offset > 0
            && self.active_offset + encoded.len() as u64 > self.config.segment_bytes
        {
            self.rotate_segment()?;
        }

        let expected_lsn =
            Lsn((self.active_segment - 1) * self.config.segment_bytes + self.active_offset);
        if expected_lsn != append.start_lsn {
            return Err(Error::CorruptWal(
                "record lsn does not match write position",
            ));
        }

        self.active_file.write_all_at(self.active_offset, encoded)?;
        // Lane BH P1 #7: count the pwrite-equivalent before bumping
        // the offset; the bench harness reads this through
        // `WalCoordinator::sync_counters_snapshot`.
        if let Some(counters) = &self.sync_counters {
            counters.bump_pwrite();
        }
        self.active_offset += encoded.len() as u64;
        self.written_lsn = append.end_lsn;
        self.prev_lsn = append.start_lsn;
        Ok(())
    }

    fn rotate_segment(&mut self) -> Result<()> {
        self.active_file.sync_data()?;
        // Lane BH P1 #7: rotation also performs an fdatasync to
        // make the trailing block durable before swapping segment
        // files; counted alongside the commit-path fdatasync.
        if let Some(counters) = &self.sync_counters {
            counters.bump_fdatasync();
        }
        self.active_segment += 1;
        self.active_offset = 0;
        self.active_file = self
            .fs
            .open_rw_create(&segment_path(&self.dir, self.active_segment))?;
        Ok(())
    }

    fn reserve_append(&mut self, encoded_len: u64) -> Result<WalAppend> {
        if encoded_len > self.config.segment_bytes {
            return Err(Error::CorruptWal("record larger than wal segment"));
        }

        if self.active_offset > 0 && self.active_offset + encoded_len > self.config.segment_bytes {
            self.rotate_segment()?;
        }

        let lsn = Lsn((self.active_segment - 1) * self.config.segment_bytes + self.active_offset);
        let end_lsn = Lsn(lsn.0 + encoded_len);
        Ok(WalAppend {
            start_lsn: lsn,
            end_lsn,
        })
    }

    fn segment_numbers(&self) -> Result<Vec<u64>> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }

        let mut segments = Vec::new();
        for name in self.fs.read_dir_names(&self.dir)? {
            if let Some(segment) = parse_segment_name(&name) {
                segments.push(segment);
            }
        }
        segments.sort_unstable();
        Ok(segments)
    }
}

impl WalReader<StdFileSystem> {
    pub fn new(path: impl AsRef<Path>, config: WalConfig) -> Self {
        Self::new_with_fs(path, config, StdFileSystem)
    }
}

impl<Fs: FileSystem> WalReader<Fs> {
    pub fn new_with_fs(path: impl AsRef<Path>, config: WalConfig, fs: Fs) -> Self {
        Self {
            dir: path.as_ref().to_path_buf(),
            fs,
            config,
        }
    }

    pub fn scan(&mut self) -> Result<Vec<WalRecord>> {
        Ok(self.scan_report()?.records)
    }

    pub fn scan_report(&mut self) -> Result<WalScanReport> {
        validate_config(&self.config)?;
        let segments = self.segment_numbers()?;
        let mut records = Vec::new();
        let mut stopped_at_tail = false;

        for (segment_index, segment) in segments.iter().enumerate() {
            let is_last_segment = segment_index + 1 == segments.len();
            let path = segment_path(&self.dir, *segment);
            let mut file = self.fs.open_rw_existing(&path)?;
            let file_len = file.len()?;
            let mut offset = 0_u64;

            while offset < file_len {
                if stopped_at_tail {
                    return Err(Error::CorruptWal("wal bytes after torn tail"));
                }

                let is_tail_candidate = is_last_segment;
                let remaining = file_len - offset;
                if remaining < WAL_HEADER_LEN as u64 {
                    if is_tail_candidate {
                        stopped_at_tail = true;
                        break;
                    }
                    return Err(Error::CorruptWal("partial record header before final tail"));
                }

                let mut header = vec![0; WAL_HEADER_LEN];
                file.read_exact_at(offset, &mut header)?;
                let payload_len = read_u32(&header, 12)? as u64;
                let record_len = match (WAL_HEADER_LEN as u64).checked_add(payload_len) {
                    Some(record_len) => record_len,
                    None if is_tail_candidate => {
                        stopped_at_tail = true;
                        break;
                    }
                    None => return Err(Error::CorruptWal("record length overflow")),
                };

                if record_len > self.config.segment_bytes {
                    if is_tail_candidate {
                        stopped_at_tail = true;
                        break;
                    }
                    return Err(Error::CorruptWal("record length exceeds segment size"));
                }

                if remaining < record_len {
                    if is_tail_candidate {
                        stopped_at_tail = true;
                        break;
                    }
                    return Err(Error::CorruptWal("partial record body before final tail"));
                }

                let mut encoded = vec![0; record_len as usize];
                encoded[..WAL_HEADER_LEN].copy_from_slice(&header);
                file.read_exact_at(
                    offset + WAL_HEADER_LEN as u64,
                    &mut encoded[WAL_HEADER_LEN..],
                )?;

                match WalRecord::decode(&encoded) {
                    Ok(record) => {
                        validate_record_position(
                            &record,
                            *segment,
                            offset,
                            self.config.segment_bytes,
                        )?;
                        records.push(record);
                        offset += record_len;
                    }
                    Err(_) if is_tail_candidate && offset + record_len >= file_len => {
                        stopped_at_tail = true;
                        break;
                    }
                    Err(err) => return Err(err),
                }
            }
        }

        let valid_end_lsn = records
            .last()
            .map(|record| Lsn(record.lsn.0 + record.encoded_len() as u64))
            .unwrap_or(Lsn::ZERO);
        Ok(WalScanReport {
            records,
            valid_end_lsn,
            torn_tail: stopped_at_tail,
        })
    }

    pub fn into_fs(self) -> Fs {
        self.fs
    }

    fn segment_numbers(&self) -> Result<Vec<u64>> {
        let mut segments = Vec::new();
        for name in self.fs.read_dir_names(&self.dir)? {
            if let Some(segment) = parse_segment_name(&name) {
                segments.push(segment);
            }
        }
        segments.sort_unstable();
        Ok(segments)
    }
}
