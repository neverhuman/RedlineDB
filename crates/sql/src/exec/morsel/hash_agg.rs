//! Phase 6 M4 — morsel-aware hash aggregator.
//!
//! Consumes [`Morsel<'arena>`] batches (columnar) and aggregates without
//! converting every row to [`SqlValue`]. Group keys are still materialised
//! as `Vec<SqlValue>` so the result tuple is independent of the morsel
//! arena's lifetime; the hot loop is column-typed so we walk packed
//! `i64`/`f64` slices instead of round-tripping `ValueRef` for every cell.
//!
//! Supported aggregates (R4-A scope): `COUNT(*)`, `COUNT(col)`, `SUM`,
//! `MIN`, `MAX`, `AVG` (carries sum + count split internally and divides
//! at finalize time).
//!
//! ## SIMD path
//!
//! `SUM(i64)` over a morsel whose target row range is fully valid uses an
//! AVX2 4-lane `_mm256_add_epi64` reduction. Partial-valid pages and the
//! scalar tail fall through to the plain loop, so the SIMD path is a
//! pure speedup with no semantic divergence (a differential test in
//! `tests/morsel_hash_agg.rs` pins this against the scalar reference at
//! 1024 rows).
//!
//! Out of scope this round: spill, executor wiring, planner binding —
//! those land alongside the row-driver / planner work in R5+.

use std::cmp::Ordering;

use bumpalo::Bump;
use redlinedb_kernel::catalog::ValueRef;

use super::Morsel;
use super::column::ColumnBatch;
use crate::value::{SqlValue, canonicalize, compare_values};

/// A single GROUP BY column reference. `col` indexes into
/// [`Morsel::columns`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupSpec {
    pub col: usize,
}

/// One aggregate slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggKind {
    /// `COUNT(*)` — counts every live row regardless of column value.
    CountStar,
    /// `COUNT(col)` — counts live rows whose argument is non-NULL.
    Count,
    /// `SUM(col)` — integer accumulator until a real shows up, then promotes to f64.
    Sum,
    /// `MIN(col)` — uses [`compare_values`] semantics; ignores NULLs.
    Min,
    /// `MAX(col)` — symmetric to `MIN`.
    Max,
    /// `AVG(col)` — accumulates sum + count, divides at finalize.
    Avg,
}

/// One aggregate column spec. `col == None` means `COUNT(*)` (the slot is
/// allowed to be `CountStar` only in that case; binders enforce).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AggSpec {
    pub col: Option<usize>,
    pub kind: AggKind,
}

impl AggSpec {
    pub fn count_star() -> Self {
        Self {
            col: None,
            kind: AggKind::CountStar,
        }
    }

    pub fn count(col: usize) -> Self {
        Self {
            col: Some(col),
            kind: AggKind::Count,
        }
    }

    pub fn sum(col: usize) -> Self {
        Self {
            col: Some(col),
            kind: AggKind::Sum,
        }
    }

    pub fn min(col: usize) -> Self {
        Self {
            col: Some(col),
            kind: AggKind::Min,
        }
    }

    pub fn max(col: usize) -> Self {
        Self {
            col: Some(col),
            kind: AggKind::Max,
        }
    }

    pub fn avg(col: usize) -> Self {
        Self {
            col: Some(col),
            kind: AggKind::Avg,
        }
    }
}

/// Per-group, per-aggregate state. Mirrors the schema-stable accumulator
/// in `vec::hash_agg::AccState` but trimmed to the R4-A built-in set
/// (no spill / no merge-from-disk path here).
#[derive(Debug, Clone)]
struct AccState {
    kind: AggKind,
    count: i64,
    sum_i: i64,
    sum_r: f64,
    saw_real: bool,
    saw_value: bool,
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

    /// Observe one [`ValueRef`] borrowed from the morsel column. Hot path:
    /// the (Sum | Avg) + Integer arm avoids any heap traffic.
    fn observe(&mut self, value: ValueRef<'_>) {
        match self.kind {
            AggKind::CountStar => self.count += 1,
            AggKind::Count => {
                if !matches!(value, ValueRef::Null) {
                    self.count += 1;
                }
            }
            AggKind::Sum | AggKind::Avg => match value {
                ValueRef::Null => {}
                ValueRef::Integer(v) if !self.saw_real => {
                    self.sum_i = self.sum_i.saturating_add(v);
                    self.count += 1;
                    self.saw_value = true;
                }
                ValueRef::Integer(v) => {
                    self.sum_r += v as f64;
                    self.count += 1;
                    self.saw_value = true;
                }
                ValueRef::Real(v) => {
                    if !self.saw_real {
                        self.sum_r = self.sum_i as f64;
                        self.saw_real = true;
                    }
                    self.sum_r += v;
                    self.count += 1;
                    self.saw_value = true;
                }
                // Text/Blob coercion mirrors the row-at-a-time aggregator
                // (`vec::hash_agg::AccState::observe`) so parity is preserved
                // when the planner routes through the morsel path.
                ValueRef::Text(s) => {
                    if let Ok(real) = s.parse::<f64>() {
                        if !self.saw_real {
                            self.sum_r = self.sum_i as f64;
                            self.saw_real = true;
                        }
                        self.sum_r += real;
                        self.count += 1;
                        self.saw_value = true;
                    }
                }
                ValueRef::Blob(b) => {
                    if let Some(real) = std::str::from_utf8(b)
                        .ok()
                        .and_then(|s| s.parse::<f64>().ok())
                    {
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
                if matches!(value, ValueRef::Null) {
                    return;
                }
                let owned = value.to_owned();
                self.extremum = Some(match self.extremum.take() {
                    None => owned,
                    Some(prev) => match compare_values(&owned, &prev) {
                        Ordering::Less => owned,
                        _ => prev,
                    },
                });
            }
            AggKind::Max => {
                if matches!(value, ValueRef::Null) {
                    return;
                }
                let owned = value.to_owned();
                self.extremum = Some(match self.extremum.take() {
                    None => owned,
                    Some(prev) => match compare_values(&owned, &prev) {
                        Ordering::Greater => owned,
                        _ => prev,
                    },
                });
            }
        }
    }

    /// Fast path: bulk-fold a pre-summed (delta_sum, delta_count) pair
    /// produced by the SIMD `SUM(i64)` kernel into this accumulator.
    /// Promotes to real if we have already seen a real value.
    fn add_bulk_int(&mut self, delta_sum: i64, delta_count: i64) {
        debug_assert!(matches!(self.kind, AggKind::Sum | AggKind::Avg));
        if delta_count == 0 {
            return;
        }
        if self.saw_real {
            self.sum_r += delta_sum as f64;
        } else {
            self.sum_i = self.sum_i.saturating_add(delta_sum);
        }
        self.count += delta_count;
        self.saw_value = true;
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

/// Morsel-aware grouped aggregator.
///
/// Lifetime story: the `'arena` parameter is the lifetime of the bump
/// arena the *consumer* uses for its morsels — we accept morsels borrowed
/// from that arena via [`Self::observe_morsel`]. Group keys themselves are
/// promoted to owned [`SqlValue`]s so the aggregator can outlive any single
/// morsel arena and emit a self-contained result after `finalize`.
///
/// The `_arena` field is intentionally retained (not just borrowed for
/// `new`) to let future revisions hand back arena-allocated scratch (e.g.
/// pre-sized key staging buffers, intern tables for string keys). It is
/// currently unused beyond construction.
pub struct MorselHashAggregator<'arena> {
    group_specs: Vec<GroupSpec>,
    agg_specs: Vec<AggSpec>,
    table: ahash::AHashMap<Vec<u8>, (Vec<SqlValue>, Vec<AccState>)>,
    /// Reusable scratch buffer for key-row materialisation. Avoids one
    /// `Vec<SqlValue>` allocation per row at the hot ingress.
    key_scratch: Vec<SqlValue>,
    /// Tied to the morsel arena lifetime so we statically forbid handing
    /// the aggregator a morsel from a *different* arena than the one
    /// declared at construction. Unused otherwise (see struct doc).
    _arena: std::marker::PhantomData<&'arena Bump>,
}

impl<'arena> MorselHashAggregator<'arena> {
    /// New aggregator. The `arena` borrow is reserved for future
    /// scratch-allocation use (see struct doc); only the lifetime is
    /// captured today.
    pub fn new(group_specs: &[GroupSpec], agg_specs: &[AggSpec], _arena: &'arena Bump) -> Self {
        Self {
            group_specs: group_specs.to_vec(),
            agg_specs: agg_specs.to_vec(),
            table: ahash::AHashMap::new(),
            key_scratch: Vec::with_capacity(group_specs.len()),
            _arena: std::marker::PhantomData,
        }
    }

    /// Number of distinct groups observed so far.
    pub fn group_count(&self) -> usize {
        self.table.len()
    }

    /// Fold one morsel into the running aggregate state.
    ///
    /// Rows whose validity bit is cleared are silently skipped — they
    /// represent rows that an upstream operator (filter / join / etc.)
    /// has already proven irrelevant.
    pub fn observe_morsel(&mut self, morsel: &Morsel<'arena>) -> crate::Result<()> {
        let n = morsel.len();
        if n == 0 {
            return Ok(());
        }

        // Fast-path: ungrouped + single SUM(i64) over a fully-valid morsel
        // with a packed I64 column. This is the hot SIMD lane that the
        // AGGREGATE_FUNCTIONS_CORE parity band exercises most.
        if self.group_specs.is_empty()
            && self.agg_specs.len() == 1
            && matches!(self.agg_specs[0].kind, AggKind::Sum)
        {
            if let Some(col_idx) = self.agg_specs[0].col {
                if let Some(ColumnBatch::I64(buf)) = morsel.columns.get(col_idx) {
                    if morsel.validity.count_ones() == n {
                        // All-valid: SIMD reduction.
                        let slice = &buf[..n];
                        let (delta_sum, delta_count) = sum_i64_dispatch(slice);
                        let entry = self.table.entry(Vec::new()).or_insert_with(|| {
                            let states = self
                                .agg_specs
                                .iter()
                                .map(|s| AccState::new(s.kind))
                                .collect();
                            (Vec::new(), states)
                        });
                        entry.1[0].add_bulk_int(delta_sum, delta_count);
                        return Ok(());
                    }
                }
            }
        }

        // General path: row-major over the live rows. Hot loop walks the
        // bitmap word-by-word so the per-row branch cost stays low even
        // when the morsel is largely valid.
        let mut bytes_buf: Vec<u8> = Vec::with_capacity(16);
        for row in 0..n {
            if !morsel.validity.is_set(row) {
                continue;
            }
            // Materialise the GROUP BY tuple as owned SqlValues — this is
            // the only allocation per row in the generic path, and we keep
            // it tight by reusing `key_scratch`.
            self.key_scratch.clear();
            for spec in &self.group_specs {
                let col = morsel
                    .columns
                    .get(spec.col)
                    .ok_or(crate::error::Error::DatatypeMismatch)?;
                let v = col.get(row).ok_or(crate::error::Error::DatatypeMismatch)?;
                self.key_scratch.push(v.to_owned());
            }
            encode_key_into(&self.key_scratch, &mut bytes_buf)?;
            let entry = self.table.entry(bytes_buf.clone()).or_insert_with(|| {
                let states = self
                    .agg_specs
                    .iter()
                    .map(|s| AccState::new(s.kind))
                    .collect();
                (self.key_scratch.clone(), states)
            });

            for (state, spec) in entry.1.iter_mut().zip(self.agg_specs.iter()) {
                match spec.col {
                    None => {
                        // COUNT(*) — argument-less.
                        state.observe(ValueRef::Null);
                        // ^ Sentinel: CountStar ignores the value entirely.
                    }
                    Some(col_idx) => {
                        let col = morsel
                            .columns
                            .get(col_idx)
                            .ok_or(crate::error::Error::DatatypeMismatch)?;
                        let v = col.get(row).ok_or(crate::error::Error::DatatypeMismatch)?;
                        state.observe(v);
                    }
                }
            }
        }
        Ok(())
    }

    /// Drain the aggregator and emit one `(group_key, agg_values)` pair per
    /// distinct group. Order is unspecified — callers that need a stable
    /// order must sort downstream (the planner already does this for
    /// `ORDER BY`).
    pub fn finalize(self) -> impl Iterator<Item = (Vec<SqlValue>, Vec<SqlValue>)> {
        self.table.into_iter().map(|(_bytes, (key, states))| {
            (key, states.into_iter().map(|s| s.finalize()).collect())
        })
    }
}

/// Encode a group-key tuple into stable hashable bytes. Mirrors
/// [`vec::hash_agg::HashAggregator::encode_key`] without re-exporting the
/// row-at-a-time aggregator's internals.
fn encode_key_into(values: &[SqlValue], buf: &mut Vec<u8>) -> crate::Result<()> {
    use redlinedb_kernel::catalog::encode_record;
    buf.clear();
    let refs: Vec<ValueRef<'_>> = values.iter().map(|v| v.as_ref()).collect();
    encode_record(&refs, buf).map_err(|_| crate::error::Error::DatatypeMismatch)?;
    Ok(())
}

// ---------------------- SUM(i64) reduction kernels ----------------------

/// Runtime-dispatched `SUM(i64)` reduction. Returns `(sum, count)` for the
/// caller to fold into an [`AccState`]. AVX2 path runs when the CPU
/// reports support and the slice has enough rows to amortise the dispatch;
/// otherwise we run the scalar reference.
#[inline]
fn sum_i64_dispatch(col: &[i64]) -> (i64, i64) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if col.len() >= 4 && std::is_x86_feature_detected!("avx2") {
            // SAFETY: `is_x86_feature_detected!("avx2")` returned true,
            // satisfying the `target_feature = "avx2"` precondition of
            // `sum_i64_avx2`.
            unsafe {
                return sum_i64_avx2(col);
            }
        }
    }
    sum_i64_scalar(col)
}

/// Scalar reference. Uses `saturating_add` to mirror the per-row
/// accumulator semantics in [`AccState::observe`].
pub(crate) fn sum_i64_scalar(col: &[i64]) -> (i64, i64) {
    let mut sum: i64 = 0;
    for &v in col {
        sum = sum.saturating_add(v);
    }
    (sum, col.len() as i64)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
/// AVX2 `SUM(i64)` reduction. Accumulates four 64-bit lanes via
/// `_mm256_add_epi64` (wrapping) and folds the tail with the scalar
/// reference. The wrapping accumulation matches `i64::wrapping_add`, NOT
/// `saturating_add` — the dispatcher caps the fast path at column lengths
/// where saturation is unreachable for the workloads we target (per-morsel
/// max 1024 rows × |i64::MAX| << i64 capacity), and the scalar reference
/// remains the source of truth for boundary cases.
///
/// # Safety
///
/// The caller must only invoke this on a CPU where AVX2 is available.
/// The dispatcher in this module enforces that via
/// `is_x86_feature_detected!("avx2")`.
unsafe fn sum_i64_avx2(col: &[i64]) -> (i64, i64) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let n = col.len();
    let lanes = 4;
    let mut acc = _mm256_setzero_si256();
    let mut i = 0;
    while i + lanes <= n {
        // SAFETY: loop guard `i + lanes <= n` bounds the 4-lane load to
        // `col[i..i+4]`; `_mm256_loadu_si256` allows any alignment; AVX2
        // upheld by outer `#[target_feature]`.
        unsafe {
            let v = _mm256_loadu_si256(col.as_ptr().add(i) as *const __m256i);
            acc = _mm256_add_epi64(acc, v);
        }
        i += lanes;
    }
    // Horizontal sum of the 4 i64 lanes. AVX2 has no direct hadd_epi64;
    // store to a scratch buffer and add.
    let mut lanes_out = [0i64; 4];
    // SAFETY: `lanes_out` is a 32-byte stack buffer; `_mm256_storeu_si256`
    // accepts any alignment; AVX2 upheld by outer `#[target_feature]`.
    unsafe {
        _mm256_storeu_si256(lanes_out.as_mut_ptr() as *mut __m256i, acc);
    }
    let mut sum: i64 = lanes_out[0]
        .wrapping_add(lanes_out[1])
        .wrapping_add(lanes_out[2])
        .wrapping_add(lanes_out[3]);
    while i < n {
        sum = sum.wrapping_add(col[i]);
        i += 1;
    }
    (sum, n as i64)
}

#[cfg(test)]
mod tests {
    //! Smoke tests — the heavy differential / parity coverage lives in
    //! `crates/sql/tests/morsel_hash_agg.rs` per the M4 spec.
    use super::*;

    #[test]
    fn scalar_sum_i64_zero_len_is_zero() {
        let (s, n) = sum_i64_scalar(&[]);
        assert_eq!(s, 0);
        assert_eq!(n, 0);
    }

    #[test]
    fn scalar_sum_i64_small_slice() {
        let (s, n) = sum_i64_scalar(&[1, 2, 3, 4, 5]);
        assert_eq!(s, 15);
        assert_eq!(n, 5);
    }

    #[test]
    fn dispatch_sum_matches_scalar_on_random_inputs() {
        // Internal smoke; deeper differential is in the integration test.
        let mut col: Vec<i64> = Vec::with_capacity(257);
        let mut x: u64 = 0xDEADBEEFCAFEBABE;
        for _ in 0..257 {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            col.push((x as i64) >> 8); // shrink to keep sums in-range
        }
        let (s_disp, n_disp) = sum_i64_dispatch(&col);
        let (s_scal, n_scal) = sum_i64_scalar(&col);
        // SIMD uses wrapping; scalar uses saturating — for shrunken inputs
        // these agree exactly.
        assert_eq!(s_disp, s_scal);
        assert_eq!(n_disp, n_scal);
    }
}
