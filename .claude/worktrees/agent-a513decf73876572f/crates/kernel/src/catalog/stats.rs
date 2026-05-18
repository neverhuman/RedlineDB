use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crc32fast::Hasher;

use crate::format::{Csn, RelId};
use crate::{Error, Result};

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
    let mut out = Writer::new();
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
    let mut reader = Reader::new(bytes);
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
    let mut out = Vec::with_capacity(20 + payload.len());
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    let crc = checksum(&payload);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

fn decode_snapshot_file(bytes: &[u8]) -> Result<StatsSnapshot> {
    if bytes.len() < 20 {
        return Err(Error::CatalogCorrupt("stats snapshot file too small"));
    }
    let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    if magic != MAGIC {
        return Err(Error::CatalogCorrupt("stats snapshot magic mismatch"));
    }
    let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
    if version != VERSION {
        return Err(Error::UnsupportedVersion(version));
    }
    let payload_len = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
    let crc = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
    let expected = 20_usize
        .checked_add(payload_len)
        .ok_or(Error::CatalogCorrupt("stats snapshot length overflow"))?;
    if bytes.len() != expected {
        return Err(Error::CatalogCorrupt("stats snapshot length mismatch"));
    }
    let payload = &bytes[20..];
    if checksum(payload) != crc {
        return Err(Error::InvalidChecksum);
    }
    decode_snapshot(payload)
}

fn encode_table_stats(out: &mut Writer, table: &TableStats) -> Result<()> {
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

fn decode_table_stats(reader: &mut Reader<'_>) -> Result<TableStats> {
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

fn encode_column_stats(out: &mut Writer, column: &ColumnStats) -> Result<()> {
    out.f64(column.null_frac);
    out.f64(column.ndv);
    out.f64(column.avg_width);
    out.opt_value(column.min.as_ref())?;
    out.opt_value(column.max.as_ref())?;
    out.u32(column.mcv.len() as u32);
    for item in &column.mcv {
        out.value(Some(&item.value))?;
        out.f64(item.frequency);
    }
    out.u32(column.histogram.len() as u32);
    for bucket in &column.histogram {
        out.opt_value(bucket.lower.as_ref())?;
        out.opt_value(bucket.upper.as_ref())?;
        out.f64(bucket.frequency);
    }
    Ok(())
}

fn decode_column_stats(reader: &mut Reader<'_>) -> Result<ColumnStats> {
    let null_frac = reader.f64()?;
    let ndv = reader.f64()?;
    let avg_width = reader.f64()?;
    let min = reader.opt_value()?;
    let max = reader.opt_value()?;
    let mcv_count = reader.u32()? as usize;
    let mut mcv = Vec::with_capacity(mcv_count);
    for _ in 0..mcv_count {
        mcv.push(MostCommonValue {
            value: reader
                .value()?
                .ok_or(Error::CatalogCorrupt("missing most common value"))?,
            frequency: reader.f64()?,
        });
    }
    let histogram_count = reader.u32()? as usize;
    let mut histogram = Vec::with_capacity(histogram_count);
    for _ in 0..histogram_count {
        histogram.push(HistogramBucket {
            lower: reader.opt_value()?,
            upper: reader.opt_value()?,
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

fn encode_index_stats(out: &mut Writer, index: &IndexStats) -> Result<()> {
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

fn decode_index_stats(reader: &mut Reader<'_>) -> Result<IndexStats> {
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

fn checksum(bytes: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    hasher.finalize()
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn f64(&mut self, value: f64) {
        self.u64(value.to_bits());
    }

    fn bool(&mut self, value: bool) {
        self.bytes.push(u8::from(value));
    }

    fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn opt_value(&mut self, value: Option<&OwnedValue>) -> Result<()> {
        match value {
            Some(value) => {
                self.bool(true);
                self.value(Some(value))?;
            }
            None => self.bool(false),
        }
        Ok(())
    }

    fn value(&mut self, value: Option<&OwnedValue>) -> Result<()> {
        match value {
            Some(OwnedValue::Null) | None => self.bytes.push(0),
            Some(OwnedValue::Integer(v)) => {
                self.bytes.push(1);
                self.u64(*v as u64);
            }
            Some(OwnedValue::Real(v)) => {
                self.bytes.push(2);
                self.f64(*v);
            }
            Some(OwnedValue::Text(v)) => {
                self.bytes.push(3);
                self.u32(v.len() as u32);
                self.bytes(v.as_bytes());
            }
            Some(OwnedValue::Blob(v)) => {
                self.bytes.push(4);
                self.u32(v.len() as u32);
                self.bytes(v.as_ref());
            }
        }
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.cursor)
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.u64()?))
    }

    fn bool(&mut self) -> Result<bool> {
        Ok(match self.take(1)?[0] {
            0 => false,
            1 => true,
            _ => return Err(Error::CatalogCorrupt("invalid bool encoding")),
        })
    }

    fn value(&mut self) -> Result<Option<OwnedValue>> {
        Ok(match self.take(1)?[0] {
            0 => None,
            1 => Some(OwnedValue::Integer(self.u64()? as i64)),
            2 => Some(OwnedValue::Real(self.f64()?)),
            3 => {
                let len = self.u32()? as usize;
                let bytes = self.take(len)?.to_vec();
                Some(OwnedValue::Text(Arc::from(
                    String::from_utf8(bytes)
                        .map_err(|_| Error::CatalogCorrupt("invalid utf8 in stats snapshot"))?,
                )))
            }
            4 => {
                let len = self.u32()? as usize;
                Some(OwnedValue::Blob(Arc::from(self.take(len)?.to_vec())))
            }
            _ => return Err(Error::CatalogCorrupt("invalid value tag")),
        })
    }

    fn opt_value(&mut self) -> Result<Option<OwnedValue>> {
        if self.bool()? { self.value() } else { Ok(None) }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .cursor
            .checked_add(len)
            .ok_or(Error::CatalogCorrupt("stats snapshot length overflow"))?;
        if end > self.bytes.len() {
            return Err(Error::BufferTooSmall {
                needed: end,
                actual: self.bytes.len(),
            });
        }
        let slice = &self.bytes[self.cursor..end];
        self.cursor = end;
        Ok(slice)
    }
}
