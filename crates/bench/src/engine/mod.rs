mod redline;
mod sqlite;

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, anyhow};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::time::Duration;

use crate::config::{DurabilityKind, EngineKind, RunSpec};

pub use redline::RedlineEngine;
pub use sqlite::SqliteEngine;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CellValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct EngineSnapshot {
    pub data_bytes: u64,
    pub wal_bytes: u64,
    pub engine_stats: serde_json::Value,
    /// Lane BH P1 #7: durability syscall counters surfaced by the
    /// engine (currently only Redline populates these from the
    /// kernel's WAL coordinator). Left `None` for engines that
    /// can't report them so the JSON omits the keys instead of
    /// emitting bogus zeros.
    #[serde(default)]
    pub fsyncs_issued: Option<u64>,
    #[serde(default)]
    pub fdatasyncs_issued: Option<u64>,
    #[serde(default)]
    pub pwrites_issued: Option<u64>,
}

pub trait BenchEngine: Send + Sync {
    fn connect(&self, worker_id: usize) -> Result<Box<dyn BenchConn>>;
    fn setup_schema(&self) -> Result<()>;
    fn seed_kv(&self, rows: usize) -> Result<()>;
    fn checkpoint(&self) -> Result<()>;
    fn snapshot(&self) -> Result<EngineSnapshot>;
    fn checksum(&self) -> Result<crate::report::Checksum>;
}

pub trait BenchConn: Send {
    fn execute(&mut self, sql: &str, params: &[CellValue]) -> Result<u64>;
    fn query_row(&mut self, sql: &str, params: &[CellValue]) -> Result<Vec<CellValue>>;
    fn query_all(&mut self, sql: &str, params: &[CellValue]) -> Result<Vec<Vec<CellValue>>>;
    fn begin_immediate(&mut self) -> Result<()>;
    fn commit(&mut self) -> Result<()>;
    fn rollback(&mut self) -> Result<()>;
    fn set_busy_timeout(&mut self, timeout: Duration) -> Result<()>;
}

pub fn open(spec: &RunSpec, db_dir: &Path) -> Result<Box<dyn BenchEngine>> {
    match spec.engine {
        EngineKind::Redline => Ok(Box::new(RedlineEngine::open(spec, db_dir)?)),
        EngineKind::Sqlite => Ok(Box::new(SqliteEngine::open(spec, db_dir)?)),
    }
}

pub fn apply_durability(options: &mut redlinedb::OpenOptions, durability: DurabilityKind) {
    options.durability = match durability {
        DurabilityKind::Strict => redlinedb::Durability::Strict,
        DurabilityKind::Normal => redlinedb::Durability::Normal,
        DurabilityKind::Unsafe => redlinedb::Durability::UnsafeDev,
    };
}

pub(crate) fn kv_checksum(conn: &mut dyn BenchConn) -> Result<crate::report::Checksum> {
    let rows = conn.query_all("SELECT k, tenant, v, version FROM kv ORDER BY k", &[])?;
    let mut hasher = Sha256::new();
    let mut version_sum = 0_i64;
    let mut payload_bytes = 0_i64;
    let mut row_bytes_for_hash: Vec<Vec<u8>> = Vec::with_capacity(rows.len());
    let mut keys_for_xor: Vec<u64> = Vec::with_capacity(rows.len());

    for row in &rows {
        hasher.update(b"row\0");
        for cell in row {
            hash_cell(&mut hasher, cell);
        }
        if let Some(CellValue::Blob(bytes)) = row.get(2) {
            payload_bytes = payload_bytes.saturating_add(bytes.len() as i64);
        }
        if let Some(CellValue::Integer(version)) = row.get(3) {
            version_sum = version_sum.saturating_add(*version);
        }
        // Lane INT: build a deterministic per-row buffer for the dataset
        // checksum. Order-sensitive payload hash + order-independent key
        // XOR catch row-shape, key-set, and value drift independently.
        let key = match row.first() {
            Some(CellValue::Integer(k)) => *k as u64,
            _ => 0,
        };
        keys_for_xor.push(key);
        let mut row_buf = Vec::with_capacity(64);
        row_buf.extend_from_slice(&key.to_le_bytes());
        if let Some(CellValue::Blob(bytes)) = row.get(2) {
            row_buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            row_buf.extend_from_slice(bytes);
        }
        row_bytes_for_hash.push(row_buf);
    }

    let rows_count = rows.len() as i64;
    let mut index_consistency = BTreeMap::new();
    index_consistency.insert(
        "kv_tenant_idx".to_owned(),
        tenant_index_consistency(conn, rows_count)?,
    );

    let dataset = crate::checksum::DatasetChecksum {
        row_count: rows_count as u64,
        key_xor: crate::checksum::key_xor(keys_for_xor.iter().copied()),
        payload_hash: crate::checksum::payload_hash(
            row_bytes_for_hash.iter().map(|v| v.as_slice()),
        ),
    };

    Ok(crate::report::Checksum {
        rows: rows_count,
        version_sum,
        payload_bytes,
        content_hash: format!("{:x}", hasher.finalize()),
        index_consistency,
        dataset: Some(dataset),
    })
}

fn hash_cell(hasher: &mut Sha256, cell: &CellValue) {
    match cell {
        CellValue::Null => hasher.update(b"n\0"),
        CellValue::Integer(value) => {
            hasher.update(b"i");
            hasher.update(value.to_le_bytes());
        }
        CellValue::Real(value) => {
            hasher.update(b"r");
            hasher.update(value.to_bits().to_le_bytes());
        }
        CellValue::Text(value) => {
            hasher.update(b"t");
            hash_bytes(hasher, value.as_bytes());
        }
        CellValue::Blob(value) => {
            hasher.update(b"b");
            hash_bytes(hasher, value);
        }
    }
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn tenant_index_consistency(conn: &mut dyn BenchConn, rows_count: i64) -> Result<String> {
    let range_count = scalar_i64(
        conn,
        "SELECT COUNT(*) FROM kv WHERE tenant BETWEEN ?1 AND ?2",
        &[CellValue::Integer(0), CellValue::Integer(31)],
    )?;
    let mut bucket_sum = 0_i64;
    for tenant in 0..32 {
        bucket_sum = bucket_sum.saturating_add(scalar_i64(
            conn,
            "SELECT COUNT(*) FROM kv WHERE tenant = ?1",
            &[CellValue::Integer(tenant)],
        )?);
    }
    if range_count == rows_count && bucket_sum == rows_count {
        Ok(format!(
            "ok rows={rows_count} tenant_range={range_count} tenant_buckets={bucket_sum}"
        ))
    } else {
        Ok(format!(
            "mismatch rows={rows_count} tenant_range={range_count} tenant_buckets={bucket_sum}"
        ))
    }
}

fn scalar_i64(conn: &mut dyn BenchConn, sql: &str, params: &[CellValue]) -> Result<i64> {
    let row = conn.query_row(sql, params)?;
    match row.first() {
        Some(CellValue::Integer(value)) => Ok(*value),
        Some(CellValue::Null) | None => Ok(0),
        other => Err(anyhow!("expected integer scalar, got {other:?}")),
    }
}

/// Deterministic seed-derived blob used by per-engine `seed_kv`
/// helpers. The byte pattern is fixed across engines so cross-engine
/// checksum comparisons stay stable.
pub(super) fn seeded_blob(seed: usize) -> Vec<u8> {
    format!("value-{seed:08}").into_bytes()
}

/// Single-file byte count helper shared by the SQLite and (legacy
/// single-file) Redline adapters. Returns 0 when the path is missing
/// or unreadable so callers can sum it into reports without special
/// cases.
pub(super) fn file_len(path: &Path) -> u64 {
    std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

/// CLI argument / report token for an `EngineKind`. Used to forward
/// the engine selection to child processes and to label rows in
/// generated reports.
pub(crate) fn engine_name(engine: EngineKind) -> &'static str {
    match engine {
        EngineKind::Redline => "redline",
        EngineKind::Sqlite => "sqlite",
    }
}
