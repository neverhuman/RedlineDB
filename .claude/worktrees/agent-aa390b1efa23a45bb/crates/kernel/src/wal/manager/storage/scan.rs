use std::path::Path;

use crate::format::Lsn;
use crate::format::bytes::read_u32;
use crate::io::{FileHandle, FileSystem, StdFileSystem};
use crate::wal::{WAL_HEADER_LEN, WalRecord};
use crate::{Error, Result};

use super::*;

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
