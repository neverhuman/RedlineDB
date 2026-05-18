use super::*;

use std::cmp::Ordering;
use std::collections::HashMap;

pub(crate) fn build_table_stats(
    conn: &Connection,
    table: &TableDef,
    rows: &[Vec<SqlValue>],
) -> Result<TableStats> {
    let current = conn.stats_snapshot();
    let preserved = current.tables.get(&table.table_id).cloned();
    let row_count = rows.len() as u64;
    let avg_row_bytes = if rows.is_empty() {
        0.0
    } else {
        rows.iter().map(|row| row_width(row)).sum::<usize>() as f64 / rows.len() as f64
    };
    Ok(TableStats {
        table_id: table.table_id,
        rel_id: table.relation_id,
        row_count,
        live_row_count: row_count,
        heap_pages: if row_count == 0 {
            0
        } else {
            row_count.div_ceil(64).max(1)
        },
        avg_row_bytes,
        analyzed_at_csn: preserved
            .as_ref()
            .map(|stats| stats.analyzed_at_csn)
            .unwrap_or(redlinedb_kernel::format::Csn::ZERO),
        data_change_count: preserved.map(|stats| stats.data_change_count).unwrap_or(0),
    })
}

pub(crate) fn build_column_stats(
    cfg: &crate::connection::StatsConfig,
    rows: &[Vec<SqlValue>],
    ordinal: usize,
) -> ColumnStats {
    if rows.is_empty() {
        return ColumnStats {
            null_frac: 1.0,
            ndv: 0.0,
            avg_width: 0.0,
            min: None,
            max: None,
            mcv: Vec::new(),
            histogram: Vec::new(),
        };
    }
    let mut nulls = 0usize;
    let mut widths = 0usize;
    let mut min: Option<SqlValue> = None;
    let mut max: Option<SqlValue> = None;
    let mut counts: HashMap<String, (usize, SqlValue)> = HashMap::new();
    let mut non_null_values = Vec::new();
    for row in rows {
        let value = row.get(ordinal).cloned().unwrap_or(SqlValue::Null);
        if matches!(value, SqlValue::Null) {
            nulls += 1;
            continue;
        }
        widths += row_width_value(&value);
        if min
            .as_ref()
            .map(|current| compare_values(&value, current) == Ordering::Less)
            .unwrap_or(true)
        {
            min = Some(value.clone());
        }
        if max
            .as_ref()
            .map(|current| compare_values(&value, current) == Ordering::Greater)
            .unwrap_or(true)
        {
            max = Some(value.clone());
        }
        non_null_values.push(value.clone());
        let key = stats_value_key(&value);
        let entry = counts.entry(key).or_insert((0, value));
        entry.0 += 1;
    }

    non_null_values.sort_by(compare_values);
    let ndv = counts.len() as f64;
    let mut mcv: Vec<_> = counts
        .into_iter()
        .map(|(_, (count, value))| MostCommonValue {
            value,
            frequency: count as f64 / rows.len() as f64,
        })
        .collect();
    mcv.sort_by(|left, right| {
        right
            .frequency
            .partial_cmp(&left.frequency)
            .unwrap_or(Ordering::Equal)
            .then_with(|| compare_values(&left.value, &right.value))
    });
    mcv.truncate(cfg.mcv_capacity);

    let histogram = build_histogram(cfg, &non_null_values, rows.len());
    ColumnStats {
        null_frac: nulls as f64 / rows.len() as f64,
        ndv,
        avg_width: if non_null_values.is_empty() {
            0.0
        } else {
            widths as f64 / non_null_values.len() as f64
        },
        min,
        max,
        mcv,
        histogram,
    }
}

pub(crate) fn build_index_stats(
    _cfg: &crate::connection::StatsConfig,
    rows: &[Vec<SqlValue>],
    index: &redlinedb_kernel::catalog::IndexDef,
) -> IndexStats {
    let mut distinct_prefix_counts = Vec::new();
    for prefix_len in 1..=index.keys.len() {
        let mut seen = std::collections::BTreeSet::new();
        for row in rows {
            let mut key = String::new();
            for key_def in index.keys.iter().take(prefix_len) {
                let value = row
                    .get(key_def.ordinal as usize)
                    .cloned()
                    .unwrap_or(SqlValue::Null);
                key.push_str(&stats_value_key(&value));
                key.push('|');
            }
            seen.insert(key);
        }
        distinct_prefix_counts.push(seen.len() as f64);
    }
    let avg_key_bytes = if rows.is_empty() {
        0.0
    } else {
        let total = rows
            .iter()
            .map(|row| {
                index
                    .keys
                    .iter()
                    .map(|key_def| {
                        row.get(key_def.ordinal as usize)
                            .map(row_width_value)
                            .unwrap_or(0)
                    })
                    .sum::<usize>()
            })
            .sum::<usize>();
        total as f64 / rows.len() as f64
    };
    IndexStats {
        index_id: index.index_id,
        entries: rows.len() as u64,
        leaf_pages: if rows.is_empty() {
            0
        } else {
            (rows.len() as u64).div_ceil(64).max(1)
        },
        height: if rows.is_empty() { 0 } else { 1 },
        distinct_prefix_counts,
        avg_key_bytes,
        clustering_factor: if rows.is_empty() { 0.0 } else { 1.0 },
    }
}

pub(crate) fn sample_rows(
    cfg: &crate::connection::StatsConfig,
    rows: &[Vec<SqlValue>],
) -> Vec<Vec<SqlValue>> {
    if rows.len() <= cfg.exact_analyze_row_threshold {
        return rows.to_vec();
    }
    let mut sample = rows
        .iter()
        .cloned()
        .map(|row| (stable_row_score(&row), row))
        .collect::<Vec<_>>();
    sample.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| compare_rows(&left.1, &right.1))
    });
    sample.truncate(cfg.sample_rows.min(sample.len()));
    sample.into_iter().map(|(_, row)| row).collect()
}

pub(crate) fn stable_row_score(row: &[SqlValue]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for value in row {
        stats_value_key(value).hash(&mut hasher);
    }
    hasher.finish()
}

pub(crate) fn build_histogram(
    cfg: &crate::connection::StatsConfig,
    values: &[SqlValue],
    total_rows: usize,
) -> Vec<HistogramBucket> {
    if values.is_empty() || cfg.histogram_buckets == 0 {
        return Vec::new();
    }
    let bucket_count = cfg.histogram_buckets.min(values.len()).max(1);
    let mut buckets = Vec::with_capacity(bucket_count);
    let chunk = values.len().div_ceil(bucket_count);
    let mut start = 0usize;
    while start < values.len() {
        let end = (start + chunk).min(values.len());
        buckets.push(HistogramBucket {
            lower: Some(values[start].clone()),
            upper: Some(values[end - 1].clone()),
            frequency: (end - start) as f64 / total_rows as f64,
        });
        start = end;
    }
    buckets
}

pub(crate) fn stats_value_key(value: &SqlValue) -> String {
    match value {
        SqlValue::Null => "n".to_owned(),
        SqlValue::Integer(v) => format!("i:{v}"),
        SqlValue::Real(v) => format!("r:{:016x}", v.to_bits()),
        SqlValue::Text(v) => format!("t:{v}"),
        SqlValue::Blob(v) => {
            let mut out = String::from("b:");
            for byte in v.iter() {
                use std::fmt::Write;
                let _ = write!(&mut out, "{byte:02x}");
            }
            out
        }
    }
}
