use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::config::RunSpec;
use crate::engine::{BenchConn, BenchEngine, CellValue, EngineSnapshot, apply_durability};
use anyhow::{Result, anyhow};

pub struct RedlineEngine {
    path: PathBuf,
    db: Arc<Mutex<redlinedb::Database>>,
}

impl RedlineEngine {
    pub fn open(spec: &RunSpec, db_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(db_dir)?;
        let path = db_dir.join("bench.redline");
        let mut options = redlinedb::OpenOptions::default();
        // Wave 7 follow-up: `OpenOptions::default()` has `create: true`,
        // which routes the facade through `Database::create` → re-init of
        // the page file → wiped data. The bench's recover-matrix and
        // failpoint child both reopen an existing `bench.redline` to verify
        // recovery; force the open path when that directory already exists.
        if path.exists() {
            options.create = false;
        }
        options.memory.cache_bytes = spec.cache_bytes;
        apply_durability(&mut options, spec.durability);
        let db = redlinedb::Database::open_with_options(&path, options)?;
        Ok(Self {
            path,
            db: Arc::new(Mutex::new(db)),
        })
    }

    fn open_conn(&self) -> Result<redlinedb::Connection> {
        self.db
            .lock()
            .map_err(|_| anyhow!("redline database mutex poisoned"))?
            .connect()
            .map_err(Into::into)
    }
}

impl BenchEngine for RedlineEngine {
    fn connect(&self, _worker_id: usize) -> Result<Box<dyn BenchConn>> {
        Ok(Box::new(RedlineConn {
            conn: self.open_conn()?,
        }))
    }

    fn setup_schema(&self) -> Result<()> {
        let mut conn = self.open_conn()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS kv(k INTEGER PRIMARY KEY, tenant INTEGER, v BLOB, version INTEGER)",
            (),
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS kv_tenant_idx ON kv(tenant)", ())?;
        Ok(())
    }

    fn seed_kv(&self, rows: usize) -> Result<()> {
        let mut conn = self.open_conn()?;
        conn.execute("DELETE FROM kv", ())?;
        conn.begin(redlinedb::BeginMode::Immediate)?;
        {
            let mut stmt =
                conn.prepare("INSERT INTO kv(k, tenant, v, version) VALUES (?, ?, ?, ?)")?;
            for i in 0..rows {
                stmt.reset()?;
                stmt.clear_bindings();
                stmt.bind_i64(1, i as i64)?;
                stmt.bind_i64(2, (i % 32) as i64)?;
                stmt.bind_blob(3, seeded_blob(i))?;
                stmt.bind_i64(4, 1)?;
                while matches!(stmt.step()?, redlinedb::Step::Row(_)) {}
            }
        }
        let _ = conn.commit()?;
        Ok(())
    }

    fn checkpoint(&self) -> Result<()> {
        self.db
            .lock()
            .map_err(|_| anyhow!("redline database mutex poisoned"))?
            .checkpoint()?;
        Ok(())
    }

    fn snapshot(&self) -> Result<EngineSnapshot> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow!("redline database mutex poisoned"))?;
        let stats = db.benchmark_stats()?;
        // Lane BH P1 #7: capture the WAL syscall counters before
        // wrapping the rest of the stats blob into JSON so we can
        // tag them onto `EngineSnapshot` directly. The bench's
        // `RunRecord.process_metrics` then merges these onto the
        // host-collected metrics so Redline rows surface real
        // fsync/pwrite tallies.
        let fdatasyncs = stats.wal.fdatasyncs_issued;
        let fsyncs = stats.wal.fsyncs_issued;
        let pwrites = stats.wal.pwrites_issued;
        // Lane BH P1 #7: a `.redline` is a directory containing a
        // catalog snapshot, page file, multiple WAL segments,
        // control files, an owner.lock — `file_len(self.path)` only
        // saw one of those. Walk the whole tree so `data_bytes`
        // matches what an operator would see in `du -sh`.
        Ok(EngineSnapshot {
            data_bytes: dir_total_bytes(&self.path),
            wal_bytes: dir_wal_bytes(self.path.parent().unwrap_or(Path::new(".")), "wal", ".wal"),
            engine_stats: serde_json::to_value(stats)?,
            fsyncs_issued: Some(fsyncs),
            fdatasyncs_issued: Some(fdatasyncs),
            pwrites_issued: Some(pwrites),
        })
    }

    fn checksum(&self) -> Result<crate::report::Checksum> {
        let mut conn = RedlineConn {
            conn: self.open_conn()?,
        };
        crate::engine::kv_checksum(&mut conn)
    }
}

struct RedlineConn {
    conn: redlinedb::Connection,
}

impl BenchConn for RedlineConn {
    fn execute(&mut self, sql: &str, params: &[CellValue]) -> Result<u64> {
        let summary = self.conn.execute(sql, values(params))?;
        Ok(summary.rows_affected)
    }

    fn query_row(&mut self, sql: &str, params: &[CellValue]) -> Result<Vec<CellValue>> {
        let rows = self.query_all(sql, params)?;
        Ok(rows.into_iter().next().unwrap_or_default())
    }

    fn query_all(&mut self, sql: &str, params: &[CellValue]) -> Result<Vec<Vec<CellValue>>> {
        let mut stmt = self.conn.prepare(sql)?;
        stmt.bind_all(values(params))?;
        let count = stmt.column_count();
        let mut rows = Vec::new();
        while let redlinedb::Step::Row(row) = stmt.step()? {
            let mut out = Vec::with_capacity(count);
            for idx in 0..count {
                out.push(match row.get_ref(idx)? {
                    redlinedb::ValueRef::Null => CellValue::Null,
                    redlinedb::ValueRef::Integer(v) => CellValue::Integer(v),
                    redlinedb::ValueRef::Real(v) => CellValue::Real(v),
                    redlinedb::ValueRef::Text(v) => CellValue::Text(v.to_owned()),
                    redlinedb::ValueRef::Blob(v) => CellValue::Blob(v.to_vec()),
                });
            }
            rows.push(out);
        }
        Ok(rows)
    }

    fn begin_immediate(&mut self) -> Result<()> {
        self.conn.begin(redlinedb::BeginMode::Immediate)?;
        Ok(())
    }

    fn commit(&mut self) -> Result<()> {
        let _ = self.conn.commit()?;
        Ok(())
    }

    fn rollback(&mut self) -> Result<()> {
        self.conn.rollback()?;
        Ok(())
    }

    fn set_busy_timeout(&mut self, timeout: Duration) -> Result<()> {
        self.conn.set_busy_timeout(timeout);
        Ok(())
    }
}

fn values(params: &[CellValue]) -> Vec<redlinedb::Value> {
    params
        .iter()
        .map(|value| match value {
            CellValue::Null => redlinedb::Value::Null,
            CellValue::Integer(v) => redlinedb::Value::Integer(*v),
            CellValue::Real(v) => redlinedb::Value::Real(*v),
            CellValue::Text(v) => redlinedb::Value::Text(v.clone().into()),
            CellValue::Blob(v) => redlinedb::Value::Blob(v.clone().into()),
        })
        .collect()
}

fn seeded_blob(seed: usize) -> Vec<u8> {
    format!("value-{seed:08}").into_bytes()
}

#[allow(dead_code)]
fn file_len(path: &Path) -> u64 {
    std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

/// Recursively sum the size of every regular file under `path`.
///
/// Redline opens a `.redline` *directory* containing the catalog
/// snapshot, page file, multiple WAL segments, control files, and
/// an owner lock. Reporting only the page file under-counted the
/// actual on-disk footprint by orders of magnitude on long runs;
/// `walkdir` follows the directory tree (skipping unreadable
/// entries) and sums every file size.
pub(crate) fn dir_total_bytes(path: &Path) -> u64 {
    if let Ok(meta) = std::fs::metadata(path)
        && meta.is_file()
    {
        // Caller passed a leaf file (e.g. legacy single-file flavor);
        // preserve the previous behavior so existing tests keep
        // their numeric expectations.
        return meta.len();
    }
    let mut total = 0_u64;
    for entry in walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let meta = match entry.metadata() {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if meta.is_file() {
            total = total.saturating_add(meta.len());
        }
    }
    total
}

fn dir_wal_bytes(dir: &Path, name: &str, ext: &str) -> u64 {
    let wal_dir = dir.join(name);
    std::fs::read_dir(&wal_dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.flatten())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|value| value == &ext[1..])
        })
        .map(|entry| entry.metadata().map(|meta| meta.len()).unwrap_or(0))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::dir_total_bytes;
    use std::fs;
    use tempfile::tempdir;

    /// Lane BH P1 #7: synthesize a `.redline` directory with the
    /// expected fan-out (catalog snapshot, page file, two wal
    /// segments, control file, owner.lock) and assert
    /// `dir_total_bytes` aggregates every byte. Regression for the
    /// pre-Lane-BH path that read only `bench.redline` and
    /// systematically under-counted on-disk usage.
    #[test]
    fn redline_data_bytes_recursive() {
        let tmp = tempdir().expect("tempdir");
        let root = tmp.path().join("bench.redline");
        fs::create_dir_all(root.join("wal")).expect("mkdir wal");
        fs::write(root.join("data.redline"), vec![0u8; 1024]).expect("data");
        fs::write(root.join("catalog.snapshot"), vec![0u8; 256]).expect("catalog");
        fs::write(root.join("control.000"), vec![0u8; 64]).expect("control0");
        fs::write(root.join("control.001"), vec![0u8; 64]).expect("control1");
        fs::write(root.join("owner.lock"), b"pid").expect("lock");
        fs::write(root.join("wal").join("000000000001.wal"), vec![0u8; 4096]).expect("wal seg 1");
        fs::write(root.join("wal").join("000000000002.wal"), vec![0u8; 2048]).expect("wal seg 2");
        let total = dir_total_bytes(&root);
        // 1024 + 256 + 64 + 64 + 3 + 4096 + 2048 = 7555
        assert_eq!(total, 1024 + 256 + 64 + 64 + 3 + 4096 + 2048);
        // Sanity: includes the wal segments specifically — used to
        // be missed when only the page file was probed.
        assert!(total > 4096, "wal segments must be included");
    }
}
