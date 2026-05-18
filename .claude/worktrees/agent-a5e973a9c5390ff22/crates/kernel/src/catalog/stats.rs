use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::format::{Csn, RelId};
use crate::{Error, Result};

use super::codec::{BytesReader, BytesWriter, frame_snapshot, parse_header};
use super::ids::{ColumnId, IndexId, TableId};
use super::value::OwnedValue;

const MAGIC: u32 = u32::from_le_bytes(*b"RSTA");
const VERSION: u16 = 1;

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
        match decode_snapshot_file(&bytes) {
            Ok(snapshot) => Ok(Some(Arc::new(snapshot))),
            Err(_) => Ok(None),
        }
    }

    pub fn save(&self, snapshot: &StatsSnapshot) -> Result<()> {
        let bytes = encode_snapshot_file(snapshot)?;
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
        encode_table_stats(&mut out, table)?;
    }

    let mut columns: Vec<_> = snapshot.columns.iter().collect();
    columns.sort_by_key(|((table_id, column_id), _)| (table_id.0, column_id.0));
    out.u32(columns.len() as u32);
    for ((table_id, column_id), column) in columns {
        out.u64(table_id.0);
        out.u64(column_id.0);
        encode_column_stats(&mut out, column)?;
    }

    let mut indexes: Vec<_> = snapshot.indexes.iter().collect();
    indexes.sort_by_key(|(index_id, _)| index_id.0);
    out.u32(indexes.len() as u32);
    for (_, index) in indexes {
        encode_index_stats(&mut out, index)?;
    }

    Ok(out.finish())
}

pub fn decode_snapshot(bytes: &[u8]) -> Result<StatsSnapshot> {
    let mut reader = BytesReader::new(bytes);
    let epoch = StatsEpoch(reader.u64()?);
    let mut snapshot = StatsSnapshot::empty(epoch);

    let table_count = reader.u32()? as usize;
    for _ in 0..table_count {
        let table = decode_table_stats(&mut reader)?;
        snapshot.tables.insert(table.table_id, table);
    }

    let column_count = reader.u32()? as usize;
    for _ in 0..column_count {
        let table_id = TableId(reader.u64()?);
        let column_id = ColumnId(reader.u64()?);
        let column = decode_column_stats(&mut reader)?;
        snapshot.columns.insert((table_id, column_id), column);
    }

    let index_count = reader.u32()? as usize;
    for _ in 0..index_count {
        let index = decode_index_stats(&mut reader)?;
        snapshot.indexes.insert(index.index_id, index);
    }

    if reader.remaining() != 0 {
        return Err(Error::CatalogCorrupt("stats snapshot has trailing bytes"));
    }

    Ok(snapshot)
}

fn encode_snapshot_file(snapshot: &StatsSnapshot) -> Result<Vec<u8>> {
    let payload = encode_snapshot(snapshot)?;
    Ok(frame_snapshot(MAGIC, VERSION, &payload))
}

fn decode_snapshot_file(bytes: &[u8]) -> Result<StatsSnapshot> {
    let frame = parse_header(
        bytes,
        MAGIC,
        Error::CatalogCorrupt("stats snapshot file too small"),
        Error::CatalogCorrupt("stats snapshot magic mismatch"),
        Error::CatalogCorrupt("stats snapshot length overflow"),
        Error::CatalogCorrupt("stats snapshot length mismatch"),
        VERSION,
        Error::UnsupportedVersion,
    )?;
    decode_snapshot(frame.payload)
}

fn encode_table_stats(out: &mut BytesWriter, table: &TableStats) -> Result<()> {
    out.u64(table.table_id.0);
    out.u64(table.rel_id.0);
    out.u64(table.row_count);
    out.u64(table.live_row_count);
    out.u64(table.heap_pages);
    out.f64(table.avg_row_bytes);
    out.u64(table.analyzed_at_csn.0);
    out.u64(table.data_change_count);
    Ok(())
}

fn decode_table_stats(reader: &mut BytesReader<'_>) -> Result<TableStats> {
    Ok(TableStats {
        table_id: TableId(reader.u64()?),
        rel_id: RelId(reader.u64()?),
        row_count: reader.u64()?,
        live_row_count: reader.u64()?,
        heap_pages: reader.u64()?,
        avg_row_bytes: reader.f64()?,
        analyzed_at_csn: Csn(reader.u64()?),
        data_change_count: reader.u64()?,
    })
}

fn encode_column_stats(out: &mut BytesWriter, column: &ColumnStats) -> Result<()> {
    out.f64(column.null_frac);
    out.f64(column.ndv);
    out.f64(column.avg_width);
    write_opt_value(out, column.min.as_ref())?;
    write_opt_value(out, column.max.as_ref())?;
    out.u32(column.mcv.len() as u32);
    for item in &column.mcv {
        write_value(out, Some(&item.value))?;
        out.f64(item.frequency);
    }
    out.u32(column.histogram.len() as u32);
    for bucket in &column.histogram {
        write_opt_value(out, bucket.lower.as_ref())?;
        write_opt_value(out, bucket.upper.as_ref())?;
        out.f64(bucket.frequency);
    }
    Ok(())
}

fn decode_column_stats(reader: &mut BytesReader<'_>) -> Result<ColumnStats> {
    let null_frac = reader.f64()?;
    let ndv = reader.f64()?;
    let avg_width = reader.f64()?;
    let min = read_opt_value(reader)?;
    let max = read_opt_value(reader)?;
    let mcv_count = reader.u32()? as usize;
    let mut mcv = Vec::with_capacity(mcv_count);
    for _ in 0..mcv_count {
        mcv.push(MostCommonValue {
            value: read_value(reader)?.ok_or(Error::CatalogCorrupt("missing most common value"))?,
            frequency: reader.f64()?,
        });
    }
    let histogram_count = reader.u32()? as usize;
    let mut histogram = Vec::with_capacity(histogram_count);
    for _ in 0..histogram_count {
        histogram.push(HistogramBucket {
            lower: read_opt_value(reader)?,
            upper: read_opt_value(reader)?,
            frequency: reader.f64()?,
        });
    }
    Ok(ColumnStats {
        null_frac,
        ndv,
        avg_width,
        min,
        max,
        mcv,
        histogram,
    })
}

fn encode_index_stats(out: &mut BytesWriter, index: &IndexStats) -> Result<()> {
    out.u64(index.index_id.0);
    out.u64(index.entries);
    out.u64(index.leaf_pages);
    out.u16(index.height);
    out.u32(index.distinct_prefix_counts.len() as u32);
    for value in &index.distinct_prefix_counts {
        out.f64(*value);
    }
    out.f64(index.avg_key_bytes);
    out.f64(index.clustering_factor);
    Ok(())
}

fn decode_index_stats(reader: &mut BytesReader<'_>) -> Result<IndexStats> {
    let index_id = IndexId(reader.u64()?);
    let entries = reader.u64()?;
    let leaf_pages = reader.u64()?;
    let height = reader.u16()?;
    let prefix_count = reader.u32()? as usize;
    let mut distinct_prefix_counts = Vec::with_capacity(prefix_count);
    for _ in 0..prefix_count {
        distinct_prefix_counts.push(reader.f64()?);
    }
    Ok(IndexStats {
        index_id,
        entries,
        leaf_pages,
        height,
        distinct_prefix_counts,
        avg_key_bytes: reader.f64()?,
        clustering_factor: reader.f64()?,
    })
}

// Stats wire format helpers for `OwnedValue`. The primitive byte ops live
// in `super::codec`; these stats-specific helpers encode the tagged
// nullable-value form used by stats snapshots (tag 0..4, where tag 0 is
// inline-null rather than the opt-prefix form used by the catalog
// schema store).

// dedup-allowed: bool-strictness — stats reads must strictly accept only
// `0` and `1` for the `bool` prefix (on-disk invariant predating
// `BytesReader::bool`, which would accept any nonzero byte). The strict
// decoder below preserves the encoded shape.
fn read_strict_bool(reader: &mut BytesReader<'_>) -> Result<bool> {
    match reader.u8()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::CatalogCorrupt("invalid bool encoding")),
    }
}

fn write_opt_value(out: &mut BytesWriter, value: Option<&OwnedValue>) -> Result<()> {
    match value {
        Some(value) => {
            out.bool(true);
            write_value(out, Some(value))?;
        }
        None => out.bool(false),
    }
    Ok(())
}

fn write_value(out: &mut BytesWriter, value: Option<&OwnedValue>) -> Result<()> {
    match value {
        Some(OwnedValue::Null) | None => out.u8(0),
        Some(OwnedValue::Integer(v)) => {
            out.u8(1);
            out.u64(*v as u64);
        }
        Some(OwnedValue::Real(v)) => {
            out.u8(2);
            out.f64(*v);
        }
        Some(OwnedValue::Text(v)) => {
            out.u8(3);
            out.u32(v.len() as u32);
            out.bytes(v.as_bytes());
        }
        Some(OwnedValue::Blob(v)) => {
            out.u8(4);
            out.u32(v.len() as u32);
            out.bytes(v.as_ref());
        }
    }
    Ok(())
}

fn read_value(reader: &mut BytesReader<'_>) -> Result<Option<OwnedValue>> {
    Ok(match reader.u8()? {
        0 => None,
        1 => Some(OwnedValue::Integer(reader.u64()? as i64)),
        2 => Some(OwnedValue::Real(reader.f64()?)),
        3 => {
            let len = reader.u32()? as usize;
            let bytes = reader.take(len)?.to_vec();
            Some(OwnedValue::Text(Arc::from(
                String::from_utf8(bytes)
                    .map_err(|_| Error::CatalogCorrupt("invalid utf8 in stats snapshot"))?,
            )))
        }
        4 => {
            let len = reader.u32()? as usize;
            Some(OwnedValue::Blob(Arc::from(reader.take(len)?.to_vec())))
        }
        _ => return Err(Error::CatalogCorrupt("invalid value tag")),
    })
}

fn read_opt_value(reader: &mut BytesReader<'_>) -> Result<Option<OwnedValue>> {
    if read_strict_bool(reader)? {
        read_value(reader)
    } else {
        Ok(None)
    }
}
