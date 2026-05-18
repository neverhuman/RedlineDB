use std::sync::Arc;

use crate::format::{Csn, RelId};
use crate::{Error, Result};

use super::super::codec::{BytesReader, BytesWriter, frame_snapshot, parse_header};
use super::super::ids::{ColumnId, IndexId, TableId};
use super::super::value::OwnedValue;
use super::{HistogramBucket, IndexStats, MostCommonValue, StatsSnapshot, TableStats};

pub(super) fn encode_snapshot_file(snapshot: &StatsSnapshot) -> Result<Vec<u8>> {
    let payload = super::encode_snapshot(snapshot)?;
    Ok(frame_snapshot(MAGIC, VERSION, &payload))
}

pub(super) fn decode_snapshot_file(bytes: &[u8]) -> Result<StatsSnapshot> {
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
    super::decode_snapshot(frame.payload)
}

const MAGIC: u32 = u32::from_le_bytes(*b"RSTA");
const VERSION: u16 = 1;

pub(super) fn encode_table_stats(out: &mut BytesWriter, table: &TableStats) -> Result<()> {
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

pub(super) fn decode_table_stats(reader: &mut BytesReader<'_>) -> Result<TableStats> {
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

pub(super) fn encode_column_stats(
    out: &mut BytesWriter,
    column: &super::ColumnStats,
) -> Result<()> {
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

pub(super) fn decode_column_stats(reader: &mut BytesReader<'_>) -> Result<super::ColumnStats> {
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
    Ok(super::ColumnStats {
        null_frac,
        ndv,
        avg_width,
        min,
        max,
        mcv,
        histogram,
    })
}

pub(super) fn encode_index_stats(out: &mut BytesWriter, index: &IndexStats) -> Result<()> {
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

pub(super) fn decode_index_stats(reader: &mut BytesReader<'_>) -> Result<IndexStats> {
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
