//! Lane VE: hash-based grouped aggregation.
//!
//! Replaces the O(n^2) "linear scan + find" group-build pass with a real
//! hash table keyed by the canonical encoding of the GROUP BY tuple.
//! Supports COUNT/SUM/AVG/MIN/MAX. When the group table grows past
//! `work_mem_bytes` the partial state is spilled to disk and the consumer
//! finalises after a second merge pass.

use std::cmp::Ordering;
use std::collections::hash_map::Entry;
use std::path::PathBuf;
use std::sync::Arc;

use redlinedb_kernel::catalog::{ValueRef, encode_record};

use super::spill::{SpillFile, SpillWriter};
use crate::error::{Error, Result};
use crate::exec::expr::row_width;
use crate::value::{SqlValue, canonicalize, compare_values};

/// One aggregate column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggKind {
    CountStar,
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

/// Per-group accumulator state for a single aggregate.
#[derive(Debug, Clone)]
struct AccState {
    kind: AggKind,
    count: i64,
    /// SUM accumulator: tracked as integer until a real shows up.
    sum_i: i64,
    sum_r: f64,
    saw_real: bool,
    saw_value: bool,
    /// MIN/MAX accumulator.
    extremum: Option<SqlValue>,
}

impl AccState {
    fn new(kind: AggKind) -> Self {
        Self {
            kind,
            count: 0,
            sum_i: 0,
            sum_r: 0.0,
            saw_real: false,
            saw_value: false,
            extremum: None,
        }
    }

    fn observe(&mut self, value: &SqlValue) {
        match self.kind {
            AggKind::CountStar => self.count += 1,
            AggKind::Count => {
                if !matches!(value, SqlValue::Null) {
                    self.count += 1;
                }
            }
            AggKind::Sum | AggKind::Avg => match value {
                SqlValue::Null => {}
                SqlValue::Integer(v) if !self.saw_real => {
                    self.sum_i = self.sum_i.saturating_add(*v);
                    self.count += 1;
                    self.saw_value = true;
                }
                SqlValue::Integer(v) => {
                    self.sum_r += *v as f64;
                    self.count += 1;
                    self.saw_value = true;
                }
                SqlValue::Real(v) => {
                    if !self.saw_real {
                        self.sum_r = self.sum_i as f64;
                        self.saw_real = true;
                    }
                    self.sum_r += *v;
                    self.count += 1;
                    self.saw_value = true;
                }
                _ => {
                    let coerced = match value {
                        SqlValue::Text(s) => s.parse::<f64>().ok(),
                        SqlValue::Blob(b) => std::str::from_utf8(b)
                            .ok()
                            .and_then(|s| s.parse::<f64>().ok()),
                        _ => None,
                    };
                    if let Some(real) = coerced {
                        if !self.saw_real {
                            self.sum_r = self.sum_i as f64;
                            self.saw_real = true;
                        }
                        self.sum_r += real;
                        self.count += 1;
                        self.saw_value = true;
                    }
                }
            },
            AggKind::Min => {
                if matches!(value, SqlValue::Null) {
                    return;
                }
                self.extremum = Some(match self.extremum.take() {
                    None => value.clone(),
                    Some(prev) => match compare_values(value, &prev) {
                        Ordering::Less => value.clone(),
                        _ => prev,
                    },
                });
            }
            AggKind::Max => {
                if matches!(value, SqlValue::Null) {
                    return;
                }
                self.extremum = Some(match self.extremum.take() {
                    None => value.clone(),
                    Some(prev) => match compare_values(value, &prev) {
                        Ordering::Greater => value.clone(),
                        _ => prev,
                    },
                });
            }
        }
    }

    fn merge(&mut self, other: AccState) {
        debug_assert_eq!(self.kind, other.kind);
        match self.kind {
            AggKind::CountStar | AggKind::Count => self.count += other.count,
            AggKind::Sum | AggKind::Avg => {
                if other.saw_real {
                    if !self.saw_real {
                        self.sum_r = self.sum_i as f64;
                        self.saw_real = true;
                    }
                    self.sum_r += other.sum_r;
                } else {
                    self.sum_i = self.sum_i.saturating_add(other.sum_i);
                }
                if other.saw_real && !self.saw_real {
                    self.saw_real = true;
                }
                if self.saw_real {
                    if !other.saw_real {
                        self.sum_r += other.sum_i as f64;
                    }
                } else {
                    // Already added.
                }
                self.count += other.count;
                self.saw_value |= other.saw_value;
            }
            AggKind::Min => {
                if let Some(v) = other.extremum {
                    self.observe(&v);
                }
            }
            AggKind::Max => {
                if let Some(v) = other.extremum {
                    self.observe(&v);
                }
            }
        }
    }

    fn finalize(self) -> SqlValue {
        match self.kind {
            AggKind::CountStar | AggKind::Count => SqlValue::Integer(self.count),
            AggKind::Sum => {
                if !self.saw_value {
                    SqlValue::Null
                } else if self.saw_real {
                    canonicalize(SqlValue::Real(self.sum_r))
                } else {
                    SqlValue::Integer(self.sum_i)
                }
            }
            AggKind::Avg => {
                if self.count == 0 {
                    SqlValue::Null
                } else {
                    let sum = if self.saw_real {
                        self.sum_r
                    } else {
                        self.sum_i as f64
                    };
                    SqlValue::Real(sum / self.count as f64)
                }
            }
            AggKind::Min | AggKind::Max => self.extremum.unwrap_or(SqlValue::Null),
        }
    }
}

/// Hash group-by aggregator. Spills partial groups when the hash table
/// exceeds the budget; the spill format is one (key, partial-state) record
/// per row that the next pass merges.
pub struct HashAggregator {
    aggs: Arc<[AggKind]>,
    work_mem_bytes: usize,
    max_spill_bytes: usize,
    spill_root: PathBuf,
    table: ahash::AHashMap<Vec<u8>, (Vec<SqlValue>, Vec<AccState>)>,
    table_bytes: usize,
    spill: Option<SpillFile>,
    spill_writer: Option<SpillWriter>,
    spilled_bytes: u64,
}

impl HashAggregator {
    pub fn new(
        aggs: Vec<AggKind>,
        work_mem_bytes: usize,
        max_spill_bytes: usize,
        spill_root: PathBuf,
    ) -> Self {
        Self {
            aggs: Arc::from(aggs.into_boxed_slice()),
            work_mem_bytes,
            max_spill_bytes,
            spill_root,
            table: ahash::AHashMap::new(),
            table_bytes: 0,
            spill: None,
            spill_writer: None,
            spilled_bytes: 0,
        }
    }

    pub fn spilled_bytes(&self) -> u64 {
        self.spilled_bytes
    }

    pub fn group_count(&self) -> usize {
        self.table.len()
    }

    /// Encode a group key tuple to a stable byte form for hashing.
    fn encode_key(values: &[SqlValue]) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(16);
        let refs: Vec<ValueRef<'_>> = values.iter().map(|v| v.as_ref()).collect();
        encode_record(&refs, &mut buf).map_err(|_| Error::DatatypeMismatch)?;
        Ok(buf)
    }

    /// Observe a row: `key` is the GROUP BY tuple, `arg_values` aligns with
    /// the aggregator list (one value per aggregate slot).
    pub fn observe(&mut self, key: Vec<SqlValue>, arg_values: &[SqlValue]) -> Result<()> {
        debug_assert_eq!(arg_values.len(), self.aggs.len());
        let bytes = Self::encode_key(&key)?;
        let entry = match self.table.entry(bytes) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                self.table_bytes = self
                    .table_bytes
                    .saturating_add(entry.key().len() + 8 * self.aggs.len());
                let states = self.aggs.iter().map(|k| AccState::new(*k)).collect();
                entry.insert((key, states))
            }
        };
        for (state, value) in entry.1.iter_mut().zip(arg_values.iter()) {
            state.observe(value);
        }

        if self.table_bytes > self.work_mem_bytes {
            self.spill_table()?;
        }
        Ok(())
    }

    fn spill_table(&mut self) -> Result<()> {
        if self.table.is_empty() {
            return Ok(());
        }
        if self.spill.is_none() {
            let file = SpillFile::create_in(&self.spill_root, "hash-agg")?;
            let writer = file.writer()?;
            self.spill = Some(file);
            self.spill_writer = Some(writer);
        }
        let writer = self
            .spill_writer
            .as_mut()
            .expect("spill writer initialised");
        // Drain table to disk; finalize-time merge re-aggregates.
        let drained: Vec<_> = self.table.drain().collect();
        for (_bytes, (key, states)) in drained {
            // Pack group as: <ngroup_keys> | keys... | ngrops aggs words...
            // We re-encode using the spill writer's row format: each "row" is
            // [k0, k1, ..., COUNTSTAR_count, COUNT_count, ... per agg state].
            let mut row: Vec<SqlValue> = Vec::with_capacity(key.len() + states.len() * 4);
            row.extend(key);
            for state in &states {
                // Encode AccState as 4 SqlValues so we can rehydrate after
                // merge: count, sum_i, sum_r (or NULL), extremum-or-NULL.
                row.push(SqlValue::Integer(state.count));
                row.push(SqlValue::Integer(state.sum_i));
                row.push(if state.saw_real {
                    SqlValue::Real(state.sum_r)
                } else {
                    SqlValue::Null
                });
                row.push(state.extremum.clone().unwrap_or(SqlValue::Null));
            }
            writer.write_row(&row)?;
        }
        writer.flush()?;
        self.spilled_bytes = writer.bytes_written();
        if self.spilled_bytes > self.max_spill_bytes as u64 {
            return Err(Error::ConstraintViolation(
                "query spill limit exceeded during hash aggregate".to_owned(),
            ));
        }
        self.table_bytes = 0;
        Ok(())
    }

    /// Finalise: merge spill (if any) with the in-memory table and yield
    /// `(key, [agg-result])` rows.
    pub fn finalize(mut self) -> Result<Vec<(Vec<SqlValue>, Vec<SqlValue>)>> {
        if self.spill.is_some() {
            // Force-flush any leftover groups so the merge sees them all.
            self.spill_table()?;
            self.merge_from_spill()?;
        }
        let mut out: Vec<(Vec<SqlValue>, Vec<SqlValue>)> = Vec::with_capacity(self.table.len());
        for (_bytes, (key, states)) in self.table.into_iter() {
            let finalized: Vec<SqlValue> = states.into_iter().map(|s| s.finalize()).collect();
            out.push((key, finalized));
        }
        Ok(out)
    }

    fn merge_from_spill(&mut self) -> Result<()> {
        let Some(file) = self.spill.as_ref() else {
            return Ok(());
        };
        let mut reader = file.reader()?;
        let key_count: usize = self
            .table
            .values()
            .next()
            .map(|(k, _)| k.len())
            .unwrap_or(0);
        // Need a proper key-arity: read first row to derive it if the table
        // was empty when we started merging.
        let key_count = if key_count == 0 {
            // Inspect the first spilled row: total cols = key_count + 4*aggs.
            let Some(first) = reader.read_row()? else {
                return Ok(());
            };
            let derived_key_count = first.len().saturating_sub(4 * self.aggs.len());
            self.merge_one_row(derived_key_count, &first)?;
            derived_key_count
        } else {
            key_count
        };
        while let Some(row) = reader.read_row()? {
            self.merge_one_row(key_count, &row)?;
        }
        Ok(())
    }

    fn merge_one_row(&mut self, key_count: usize, row: &[SqlValue]) -> Result<()> {
        let key = row[..key_count].to_vec();
        let bytes = Self::encode_key(&key)?;
        let entry = self.table.entry(bytes).or_insert_with(|| {
            let states = self.aggs.iter().map(|k| AccState::new(*k)).collect();
            (key.clone(), states)
        });
        for (i, kind) in self.aggs.iter().enumerate() {
            let base = key_count + 4 * i;
            let count = match row[base] {
                SqlValue::Integer(v) => v,
                _ => 0,
            };
            let sum_i = match row[base + 1] {
                SqlValue::Integer(v) => v,
                _ => 0,
            };
            let (sum_r, saw_real) = match row[base + 2] {
                SqlValue::Real(v) => (v, true),
                _ => (0.0, false),
            };
            let extremum = match &row[base + 3] {
                SqlValue::Null => None,
                other => Some(other.clone()),
            };
            let mut other = AccState::new(*kind);
            other.count = count;
            other.sum_i = sum_i;
            other.sum_r = sum_r;
            other.saw_real = saw_real;
            other.saw_value = count > 0;
            other.extremum = extremum;
            entry.1[i].merge(other);
        }
        Ok(())
    }
}

/// Convenience: row-width estimator, exposed for tests / planner sizing.
pub fn estimate_group_bytes(key: &[SqlValue], aggs_len: usize) -> usize {
    row_width(key) + aggs_len * 32
}

/// Encode a group-by tuple into stable bytes suitable for hash lookup.
///
/// Public so the scalar grouped-aggregate path in `exec.rs` can drop its
/// O(n²) linear-find group build and use a real `HashMap`. Mirrors
/// [`HashAggregator::encode_key`] without exposing the type.
pub fn encode_group_key_bytes(values: &[SqlValue]) -> Result<Vec<u8>> {
    HashAggregator::encode_key_public(values)
}

/// Phase 4.5: caller-buffer variant of [`encode_group_key_bytes`].
/// Clears `buf` and writes the encoded key in-place, returning `Ok(())`
/// on success. Callers in tight loops (window partitioning,
/// scalar GROUP BY hashing) can reuse a single scratch buffer per
/// statement and avoid allocating a fresh `Vec<u8>` per row. The
/// existing `encode_group_key_bytes` is preserved (and now delegates
/// to this helper) so the wider call surface is unchanged.
pub fn encode_group_key_bytes_into(values: &[SqlValue], buf: &mut Vec<u8>) -> Result<()> {
    buf.clear();
    let refs: Vec<ValueRef<'_>> = values.iter().map(|v| v.as_ref()).collect();
    encode_record(&refs, buf).map_err(|_| Error::DatatypeMismatch)?;
    Ok(())
}

impl HashAggregator {
    /// Public alias for [`HashAggregator::encode_key`] used by the scalar
    /// grouped-aggregate path. Keeps the canonical encoding in one place.
    pub fn encode_key_public(values: &[SqlValue]) -> Result<Vec<u8>> {
        Self::encode_key(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn key1(v: i64) -> Vec<SqlValue> {
        vec![SqlValue::Integer(v)]
    }

    #[test]
    fn count_and_sum_correctness() {
        let root = tempdir().expect("tempdir");
        let mut agg = HashAggregator::new(
            vec![AggKind::CountStar, AggKind::Sum],
            16 * 1024,
            1024 * 1024,
            root.path().to_path_buf(),
        );
        for v in [1, 2, 3, 4, 5] {
            let group = if v % 2 == 0 { 0 } else { 1 };
            agg.observe(key1(group), &[SqlValue::Null, SqlValue::Integer(v)])
                .expect("observe");
        }
        let mut rows = agg.finalize().expect("finalize");
        rows.sort_by(|a, b| compare_values(&a.0[0], &b.0[0]));
        assert_eq!(rows.len(), 2);
        // Group 0 (even): rows {2, 4}; group 1 (odd): {1, 3, 5}.
        assert_eq!(rows[0].1[0], SqlValue::Integer(2));
        assert_eq!(rows[0].1[1], SqlValue::Integer(6));
        assert_eq!(rows[1].1[0], SqlValue::Integer(3));
        assert_eq!(rows[1].1[1], SqlValue::Integer(9));
    }

    #[test]
    fn avg_correctness() {
        let root = tempdir().expect("tempdir");
        let mut agg = HashAggregator::new(
            vec![AggKind::Avg],
            16 * 1024,
            1024 * 1024,
            root.path().to_path_buf(),
        );
        for v in [10, 20, 30] {
            agg.observe(key1(0), &[SqlValue::Integer(v)]).expect("o");
        }
        let rows = agg.finalize().expect("finalize");
        match rows[0].1[0] {
            SqlValue::Real(v) => assert!((v - 20.0).abs() < 1e-9),
            _ => panic!("expected real"),
        }
    }

    #[test]
    fn min_max_correctness() {
        let root = tempdir().expect("tempdir");
        let mut agg = HashAggregator::new(
            vec![AggKind::Min, AggKind::Max],
            16 * 1024,
            1024 * 1024,
            root.path().to_path_buf(),
        );
        for v in [5, 1, 9, 3, 7] {
            agg.observe(key1(0), &[SqlValue::Integer(v), SqlValue::Integer(v)])
                .expect("o");
        }
        let rows = agg.finalize().expect("finalize");
        assert_eq!(rows[0].1[0], SqlValue::Integer(1));
        assert_eq!(rows[0].1[1], SqlValue::Integer(9));
    }

    #[test]
    fn count_skips_nulls() {
        let root = tempdir().expect("tempdir");
        let mut agg = HashAggregator::new(
            vec![AggKind::Count],
            16 * 1024,
            1024 * 1024,
            root.path().to_path_buf(),
        );
        agg.observe(key1(0), &[SqlValue::Integer(1)]).expect("o");
        agg.observe(key1(0), &[SqlValue::Null]).expect("o");
        agg.observe(key1(0), &[SqlValue::Integer(3)]).expect("o");
        let rows = agg.finalize().expect("finalize");
        assert_eq!(rows[0].1[0], SqlValue::Integer(2));
    }

    #[test]
    fn repeated_group_hits_do_not_create_spill_pressure() {
        let root = tempdir().expect("tempdir");
        let mut agg = HashAggregator::new(
            vec![AggKind::CountStar],
            1024,
            1024 * 1024,
            root.path().to_path_buf(),
        );
        for _ in 0..200 {
            agg.observe(key1(0), &[SqlValue::Null]).expect("o");
        }
        assert_eq!(agg.group_count(), 1);
        assert_eq!(agg.spilled_bytes(), 0);
        let rows = agg.finalize().expect("finalize");
        assert_eq!(rows[0].1[0], SqlValue::Integer(200));
    }

    #[test]
    fn spill_triggered_by_tiny_budget_yields_correct_aggregate() {
        let root = tempdir().expect("tempdir");
        let mut agg = HashAggregator::new(
            vec![AggKind::Sum],
            8,
            1024 * 1024,
            root.path().to_path_buf(),
        );
        for v in 0..200 {
            let group = (v % 4) as i64;
            agg.observe(key1(group), &[SqlValue::Integer(v as i64)])
                .expect("o");
        }
        let spilled_before = agg.spilled_bytes();
        let mut rows = agg.finalize().expect("finalize");
        assert!(spilled_before > 0, "expected spill");
        rows.sort_by(|a, b| compare_values(&a.0[0], &b.0[0]));
        // Group g sums: g, g+4, g+8, ... up to 196 — sum = sum_{i=0..50}(g+4i)
        // = 50*g + 4*(0+1+...+49) = 50*g + 4*1225 = 50*g + 4900.
        for (i, expected_g) in [0i64, 1, 2, 3].iter().enumerate() {
            let want = 50 * *expected_g + 4 * (49 * 50 / 2);
            assert_eq!(rows[i].1[0], SqlValue::Integer(want));
        }
    }
}
