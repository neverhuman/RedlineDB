use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use rusqlite::types::{Value, ValueRef};
use rusqlite::{Connection, OpenFlags, params_from_iter};

use crate::config::{DurabilityKind, RunSpec};
use crate::engine::{BenchConn, BenchEngine, CellValue, EngineSnapshot};

pub struct SqliteEngine {
    path: PathBuf,
    init: Mutex<()>,
    cache_kib: isize,
    durability: DurabilityKind,
}

impl SqliteEngine {
    pub fn open(spec: &RunSpec, db_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(db_dir)?;
        Ok(Self {
            path: db_dir.join("bench.sqlite3"),
            init: Mutex::new(()),
            cache_kib: -((spec.cache_bytes / 1024) as isize),
            durability: spec.durability,
        })
    }

    fn open_conn(&self) -> Result<Connection> {
        let flags = OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_URI;
        let conn = Connection::open_with_flags(&self.path, flags)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        apply_pragmas(&conn, self.cache_kib, self.durability)?;
        Ok(conn)
    }

    /// Capture validated PRAGMA settings as `BTreeMap<String, String>`.
    /// Failures are best-effort — we record `<error>` for any pragma that
    /// the driver refuses to read so the manifest still surfaces what we
    /// attempted.
    pub fn pragmas(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let conn = match self.open_conn() {
            Ok(conn) => conn,
            Err(_) => return out,
        };
        for name in [
            "journal_mode",
            "synchronous",
            "cache_size",
            "busy_timeout",
            "foreign_keys",
            "page_size",
        ] {
            let value =
                read_pragma_value(&conn, name).unwrap_or_else(|err| format!("<error: {err}>"));
            out.insert(name.to_owned(), value);
        }
        out
    }
}

fn read_pragma_value(conn: &Connection, name: &str) -> Result<String> {
    let sql = format!("PRAGMA {name}");
    let value: String = conn.query_row(&sql, [], |row| {
        // PRAGMA results are heterogeneous — read into ValueRef so we can
        // print integers and text uniformly.
        Ok(match row.get_ref(0)? {
            ValueRef::Null => String::new(),
            ValueRef::Integer(v) => v.to_string(),
            ValueRef::Real(v) => v.to_string(),
            ValueRef::Text(v) => String::from_utf8_lossy(v).into_owned(),
            ValueRef::Blob(v) => format!("blob:{}", v.len()),
        })
    })?;
    Ok(value)
}

impl BenchEngine for SqliteEngine {
    fn connect(&self, _worker_id: usize) -> Result<Box<dyn BenchConn>> {
        Ok(Box::new(SqliteConn {
            conn: self.open_conn()?,
        }))
    }

    fn setup_schema(&self) -> Result<()> {
        let _guard = self
            .init
            .lock()
            .map_err(|_| anyhow!("sqlite init mutex poisoned"))?;
        let conn = self.open_conn()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS kv(k INTEGER PRIMARY KEY, tenant INTEGER, v BLOB, version INTEGER);
             CREATE INDEX IF NOT EXISTS kv_tenant_idx ON kv(tenant);",
        )?;
        Ok(())
    }

    fn seed_kv(&self, rows: usize) -> Result<()> {
        let conn = self.open_conn()?;
        conn.execute("DELETE FROM kv", [])?;
        conn.execute_batch("BEGIN IMMEDIATE")?;
        {
            let mut stmt =
                conn.prepare("INSERT INTO kv(k, tenant, v, version) VALUES (?1, ?2, ?3, ?4)")?;
            for i in 0..rows {
                stmt.execute((i as i64, (i % 32) as i64, seeded_blob(i), 1_i64))?;
            }
        }
        conn.execute_batch("COMMIT")?;
        Ok(())
    }

    fn checkpoint(&self) -> Result<()> {
        let conn = self.open_conn()?;
        conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE)")?;
        Ok(())
    }

    fn snapshot(&self) -> Result<EngineSnapshot> {
        // NEEDS_REVIEW: SQLite VFS metrics not yet captured — the snapshot
        // surfaces only journal/synchronous PRAGMAs and on-disk byte counts.
        let conn = self.open_conn()?;
        let journal_mode: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
        let synchronous: i64 = conn.query_row("PRAGMA synchronous", [], |row| row.get(0))?;
        Ok(EngineSnapshot {
            data_bytes: file_len(&self.path),
            wal_bytes: file_len(&self.path.with_extension("sqlite3-wal")),
            engine_stats: serde_json::json!({
                "journal_mode": journal_mode,
                "synchronous": synchronous,
            }),
            // Lane BH P1 #7: SQLite does not expose internal sync
            // counters; bench harness leaves these `None` so the
            // process-level path (strace summary on Linux) is the
            // authoritative source for SQLite rows.
            fsyncs_issued: None,
            fdatasyncs_issued: None,
            pwrites_issued: None,
        })
    }

    fn checksum(&self) -> Result<crate::report::Checksum> {
        let mut conn = SqliteConn {
            conn: self.open_conn()?,
        };
        crate::engine::kv_checksum(&mut conn)
    }
}

struct SqliteConn {
    conn: Connection,
}

impl BenchConn for SqliteConn {
    fn execute(&mut self, sql: &str, params: &[CellValue]) -> Result<u64> {
        let values = to_sqlite_values(params);
        let changed = self.conn.execute(sql, params_from_iter(values))?;
        Ok(changed as u64)
    }

    fn query_row(&mut self, sql: &str, params: &[CellValue]) -> Result<Vec<CellValue>> {
        let rows = self.query_all(sql, params)?;
        Ok(rows.into_iter().next().unwrap_or_default())
    }

    fn query_all(&mut self, sql: &str, params: &[CellValue]) -> Result<Vec<Vec<CellValue>>> {
        let values = to_sqlite_values(params);
        let mut stmt = self.conn.prepare(sql)?;
        let count = stmt.column_count();
        let mut rows = stmt.query(params_from_iter(values))?;
        let mut out_rows = Vec::new();
        while let Some(row) = rows.next()? {
            let mut out = Vec::with_capacity(count);
            for idx in 0..count {
                out.push(match row.get_ref(idx)? {
                    ValueRef::Null => CellValue::Null,
                    ValueRef::Integer(v) => CellValue::Integer(v),
                    ValueRef::Real(v) => CellValue::Real(v),
                    ValueRef::Text(v) => CellValue::Text(String::from_utf8_lossy(v).into_owned()),
                    ValueRef::Blob(v) => CellValue::Blob(v.to_vec()),
                });
            }
            out_rows.push(out);
        }
        Ok(out_rows)
    }

    fn begin_immediate(&mut self) -> Result<()> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        Ok(())
    }

    fn commit(&mut self) -> Result<()> {
        self.conn.execute_batch("COMMIT")?;
        Ok(())
    }

    fn rollback(&mut self) -> Result<()> {
        self.conn.execute_batch("ROLLBACK")?;
        Ok(())
    }

    fn set_busy_timeout(&mut self, timeout: Duration) -> Result<()> {
        self.conn.busy_timeout(timeout)?;
        Ok(())
    }
}

fn apply_pragmas(conn: &Connection, cache_kib: isize, durability: DurabilityKind) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "wal_autocheckpoint", 0_i64)?;
    conn.pragma_update(None, "cache_size", cache_kib)?;
    let synchronous = match durability {
        DurabilityKind::Strict => "FULL",
        DurabilityKind::Normal => "NORMAL",
        DurabilityKind::Unsafe => "OFF",
    };
    conn.pragma_update(None, "synchronous", synchronous)?;

    let mode: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        bail!("sqlite refused WAL mode, got {mode}");
    }
    let current_sync: i64 = conn.query_row("PRAGMA synchronous", [], |row| row.get(0))?;
    let expected_sync = match durability {
        DurabilityKind::Strict => 2,
        DurabilityKind::Normal => 1,
        DurabilityKind::Unsafe => 0,
    };
    if current_sync != expected_sync {
        bail!("sqlite refused synchronous={synchronous}, got code {current_sync}");
    }
    Ok(())
}

fn to_sqlite_values(params: &[CellValue]) -> Vec<Value> {
    params
        .iter()
        .map(|value| match value {
            CellValue::Null => Value::Null,
            CellValue::Integer(v) => Value::Integer(*v),
            CellValue::Real(v) => Value::Real(*v),
            CellValue::Text(v) => Value::Text(v.clone()),
            CellValue::Blob(v) => Value::Blob(v.clone()),
        })
        .collect()
}

fn seeded_blob(seed: usize) -> Vec<u8> {
    format!("value-{seed:08}").into_bytes()
}

fn file_len(path: &Path) -> u64 {
    std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}
