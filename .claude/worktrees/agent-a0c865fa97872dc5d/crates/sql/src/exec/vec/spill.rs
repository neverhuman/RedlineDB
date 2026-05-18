//! Lane VE: spill-file format and lifecycle.
//!
//! Sortable / aggregatable operators that exceed `work_mem_bytes` write
//! sorted "runs" or partial groups to a scratch file. Each run is a
//! sequence of length-prefixed rows, framed in 64KB blocks for predictable
//! I/O. The file is deleted on drop so spill never leaks across query
//! failures.

use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use redlinedb_kernel::catalog::{RecordRef, RecordScratch, ValueRef, encode_record};

use crate::error::{Error, Result};
use crate::value::SqlValue;

/// Block size in bytes. Each row is length-prefixed; rows that span block
/// boundaries are written contiguously (the block size is advisory for
/// flush cadence, not a hard frame).
pub const SPILL_BLOCK_BYTES: usize = 64 * 1024;

static SPILL_FILE_COUNTER: AtomicU64 = AtomicU64::new(1);

pub(crate) fn spill_path(root: &Path, label: &str) -> PathBuf {
    let stamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d,
        Err(_) => Duration::default(),
    }
    .as_nanos();
    let seq = SPILL_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    root.join(format!("redline-ve-{label}-{stamp}-{seq}.spill"))
}

/// A single spill file, deleted on drop.
#[derive(Debug)]
pub struct SpillFile {
    path: PathBuf,
}

impl SpillFile {
    pub fn create(label: &str) -> Result<Self> {
        Self::create_in(std::env::temp_dir(), label)
    }

    pub fn create_in(root: impl AsRef<Path>, label: &str) -> Result<Self> {
        let root = root.as_ref();
        fs::create_dir_all(root)
            .map_err(|err| Error::ConstraintViolation(format!("spill root create: {err}")))?;
        let path = spill_path(root, label);
        // Touch the file so subsequent opens never race.
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .map_err(|err| Error::ConstraintViolation(format!("spill create: {err}")))?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn writer(&self) -> Result<SpillWriter> {
        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.path)
            .map_err(|err| Error::ConstraintViolation(format!("spill open writer: {err}")))?;
        Ok(SpillWriter {
            inner: BufWriter::with_capacity(SPILL_BLOCK_BYTES, file),
            bytes_written: 0,
        })
    }

    pub fn reader(&self) -> Result<SpillReader> {
        let file = File::open(&self.path)
            .map_err(|err| Error::ConstraintViolation(format!("spill open reader: {err}")))?;
        Ok(SpillReader {
            inner: BufReader::with_capacity(SPILL_BLOCK_BYTES, file),
        })
    }
}

impl Drop for SpillFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Writes length-prefixed encoded rows, flushing on each block.
pub struct SpillWriter {
    inner: BufWriter<File>,
    bytes_written: u64,
}

impl SpillWriter {
    pub fn write_row(&mut self, row: &[SqlValue]) -> Result<()> {
        let mut buf = Vec::with_capacity(64);
        let refs = row.iter().map(|value| value.as_ref()).collect::<Vec<_>>();
        encode_record(&refs, &mut buf).map_err(|_| Error::DatatypeMismatch)?;
        let len = buf.len() as u32;
        self.inner
            .write_all(&len.to_le_bytes())
            .map_err(|err| Error::ConstraintViolation(format!("spill write len: {err}")))?;
        self.inner
            .write_all(&buf)
            .map_err(|err| Error::ConstraintViolation(format!("spill write row: {err}")))?;
        self.bytes_written += 4 + buf.len() as u64;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        self.inner
            .flush()
            .map_err(|err| Error::ConstraintViolation(format!("spill flush: {err}")))
    }

    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }
}

/// Reads back length-prefixed encoded rows.
pub struct SpillReader {
    inner: BufReader<File>,
}

impl SpillReader {
    /// Read a single row. Returns `Ok(None)` at EOF.
    pub fn read_row(&mut self) -> Result<Option<Vec<SqlValue>>> {
        let mut len_bytes = [0u8; 4];
        match self.inner.read_exact(&mut len_bytes) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(err) => {
                return Err(Error::ConstraintViolation(format!("spill read len: {err}")));
            }
        }
        let len = u32::from_le_bytes(len_bytes) as usize;
        let mut payload = vec![0u8; len];
        self.inner
            .read_exact(&mut payload)
            .map_err(|err| Error::ConstraintViolation(format!("spill read row: {err}")))?;
        let record = RecordRef::new(&payload).map_err(|_| Error::DatatypeMismatch)?;
        let mut scratch = RecordScratch::default();
        record
            .decode_into(&mut scratch)
            .map_err(|_| Error::DatatypeMismatch)?;
        let column_count = record.column_count().map_err(|_| Error::DatatypeMismatch)?;
        let mut values = Vec::with_capacity(column_count);
        for idx in 0..column_count {
            let v = record
                .value_at(&scratch, idx)
                .map_err(|_| Error::DatatypeMismatch)?;
            values.push(owned(v));
        }
        Ok(Some(values))
    }
}

fn owned(value: ValueRef<'_>) -> SqlValue {
    value.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn round_trip_single_row() {
        let root = tempdir().expect("tempdir");
        let file = SpillFile::create_in(root.path(), "test-rt").expect("create");
        let mut writer = file.writer().expect("writer");
        writer
            .write_row(&[SqlValue::Integer(42), SqlValue::Text(Arc::from("hello"))])
            .expect("write");
        writer.flush().expect("flush");
        let mut reader = file.reader().expect("reader");
        let row = reader.read_row().expect("read").expect("some");
        assert_eq!(
            row,
            vec![SqlValue::Integer(42), SqlValue::Text(Arc::from("hello"))]
        );
        assert!(reader.read_row().expect("eof").is_none());
    }

    #[test]
    fn round_trip_many_rows() {
        let root = tempdir().expect("tempdir");
        let file = SpillFile::create_in(root.path(), "test-many").expect("create");
        let mut writer = file.writer().expect("writer");
        for i in 0..5_000 {
            writer
                .write_row(&[SqlValue::Integer(i), SqlValue::Real(i as f64 / 7.0)])
                .expect("write");
        }
        writer.flush().expect("flush");
        let mut reader = file.reader().expect("reader");
        for i in 0..5_000 {
            let row = reader.read_row().expect("read").expect("some");
            assert_eq!(row[0], SqlValue::Integer(i));
        }
        assert!(reader.read_row().expect("eof").is_none());
    }

    #[test]
    fn spill_file_is_removed_on_drop() {
        let path = {
            let root = tempdir().expect("tempdir");
            let file = SpillFile::create_in(root.path(), "test-drop").expect("create");
            file.path().clone()
        };
        assert!(!path.exists(), "spill file should be removed on drop");
    }
}
