use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::format::{Csn, RelId};
use crate::{Error, Result};

use super::codec::{BytesReader, BytesWriter};
use super::ids::{ColumnId, IndexId, TableId};
use super::value::OwnedValue;

#[path = "stats/wire.rs"]
mod wire;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct StatsEpoch(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub struct MostCommonValue {
    pub value: OwnedValue,
    pub frequency: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistogramBucket {
    pub lower: Option<OwnedValue>,
    pub upper: Option<OwnedValue>,
    pub frequency: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableStats {
    pub table_id: TableId,
    pub rel_id: RelId,
    pub row_count: u64,
    pub live_row_count: u64,
    pub heap_pages: u64,
    pub avg_row_bytes: f64,
    pub analyzed_at_csn: Csn,
    pub data_change_count: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnStats {
    pub null_frac: f64,
    pub ndv: f64,
    pub avg_width: f64,
    pub min: Option<OwnedValue>,
    pub max: Option<OwnedValue>,
    pub mcv: Vec<MostCommonValue>,
    pub histogram: Vec<HistogramBucket>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndexStats {
    pub index_id: IndexId,
    pub entries: u64,
    pub leaf_pages: u64,
    pub height: u16,
    pub distinct_prefix_counts: Vec<f64>,
    pub avg_key_bytes: f64,
    pub clustering_factor: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatsSnapshot {
    pub epoch: StatsEpoch,
    pub tables: HashMap<TableId, TableStats>,
    pub columns: HashMap<(TableId, ColumnId), ColumnStats>,
    pub indexes: HashMap<IndexId, IndexStats>,
}

impl StatsSnapshot {
    pub fn empty(epoch: StatsEpoch) -> Self {
        Self {
            epoch,
            tables: HashMap::new(),
            columns: HashMap::new(),
            indexes: HashMap::new(),
        }
    }
}

impl Default for StatsSnapshot {
    fn default() -> Self {
        Self::empty(StatsEpoch(0))
    }
}

#[derive(Debug, Clone)]
pub struct StatsStore {
    path: PathBuf,
}

impl StatsStore {
    pub fn new(base: impl AsRef<Path>) -> Self {
        Self {
            path: base.as_ref().join("stats.redline"),
        }
    }

    pub fn load(&self) -> Result<Option<Arc<StatsSnapshot>>> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        match wire::decode_snapshot_file(&bytes) {
            Ok(snapshot) => Ok(Some(Arc::new(snapshot))),
            Err(_) => Ok(None),
        }
    }

    pub fn save(&self, snapshot: &StatsSnapshot) -> Result<()> {
        let bytes = wire::encode_snapshot_file(snapshot)?;
        let tmp = self.path.with_extension("tmp");
        {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        fs::rename(tmp, &self.path)?;
        Ok(())
    }
}

pub fn encode_snapshot(snapshot: &StatsSnapshot) -> Result<Vec<u8>> {
    let mut out = BytesWriter::new();
    out.u64(snapshot.epoch.0);

    let mut tables: Vec<_> = snapshot.tables.iter().collect();
    tables.sort_by_key(|(table_id, _)| table_id.0);
    out.u32(tables.len() as u32);
    for (_, table) in tables {
        wire::encode_table_stats(&mut out, table)?;
    }

    let mut columns: Vec<_> = snapshot.columns.iter().collect();
    columns.sort_by_key(|((table_id, column_id), _)| (table_id.0, column_id.0));
    out.u32(columns.len() as u32);
    for ((table_id, column_id), column) in columns {
        out.u64(table_id.0);
        out.u64(column_id.0);
        wire::encode_column_stats(&mut out, column)?;
    }

    let mut indexes: Vec<_> = snapshot.indexes.iter().collect();
    indexes.sort_by_key(|(index_id, _)| index_id.0);
    out.u32(indexes.len() as u32);
    for (_, index) in indexes {
        wire::encode_index_stats(&mut out, index)?;
    }

    Ok(out.finish())
}

pub fn decode_snapshot(bytes: &[u8]) -> Result<StatsSnapshot> {
    let mut reader = BytesReader::new(bytes);
    let epoch = StatsEpoch(reader.u64()?);
    let mut snapshot = StatsSnapshot::empty(epoch);

    let table_count = reader.u32()? as usize;
    for _ in 0..table_count {
        let table = wire::decode_table_stats(&mut reader)?;
        snapshot.tables.insert(table.table_id, table);
    }

    let column_count = reader.u32()? as usize;
    for _ in 0..column_count {
        let table_id = TableId(reader.u64()?);
        let column_id = ColumnId(reader.u64()?);
        let column = wire::decode_column_stats(&mut reader)?;
        snapshot.columns.insert((table_id, column_id), column);
    }

    let index_count = reader.u32()? as usize;
    for _ in 0..index_count {
        let index = wire::decode_index_stats(&mut reader)?;
        snapshot.indexes.insert(index.index_id, index);
    }

    if reader.remaining() != 0 {
        return Err(Error::CatalogCorrupt("stats snapshot has trailing bytes"));
    }

    Ok(snapshot)
}
