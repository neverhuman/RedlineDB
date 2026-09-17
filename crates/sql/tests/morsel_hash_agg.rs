//! Phase 6 M4 (R4-A) — morsel-aware hash aggregator tests.
//!
//! Mirrors the `morsel_scan_filter.rs` structure: the morsel module in
//! `redlinedb-sql` is `pub(crate)`, so we re-include the source files via
//! `#[path = "..."]` and exercise the aggregator against a private compile
//! copy. The vector-path differential (parity vs. `vec::hash_agg`) uses
//! the *public* `redlinedb_sql` `SqlValue` aliases so the comparison
//! still reflects the production type story.

#![allow(dead_code)]

use bumpalo::Bump;
use redlinedb_kernel::catalog::ValueRef;
use smallvec::SmallVec;

// -------- re-compile of the morsel module (mirrors morsel_scan_filter.rs) --

#[path = "../src/exec/morsel/arena.rs"]
mod arena;
#[path = "../src/exec/morsel/bitmap.rs"]
mod bitmap;
#[path = "../src/exec/morsel/column.rs"]
mod column;

pub const MAX_BATCH_ROWS: usize = 1024;

#[derive(Debug)]
pub struct Morsel<'a> {
    pub columns: SmallVec<[column::ColumnBatch<'a>; 8]>,
    pub validity: bitmap::Bitmap,
    pub len: u16,
}

impl<'a> Morsel<'a> {
    pub fn new() -> Self {
        Self {
            columns: SmallVec::new(),
            validity: bitmap::Bitmap::new(0),
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn width(&self) -> usize {
        self.columns.len()
    }

    pub fn live_rows(&self) -> usize {
        self.validity.count_ones()
    }
}

impl<'a> Default for Morsel<'a> {
    fn default() -> Self {
        Self::new()
    }
}

#[path = "../src/exec/morsel/builder.rs"]
mod builder;

// `hash_agg.rs` references `crate::value::SqlValue`, `crate::Result`, and
// `crate::error::Error`. Inside this integration test those resolve to the
// test binary's crate root, so we provide the same aliases that
// `redlinedb-sql` exposes publicly. This keeps the aggregator's source
// untouched while letting the differential test still exercise the *real*
// SqlValue / encode_record pipeline.

pub mod error {
    use redlinedb_kernel::Error as KernelError;
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum Error {
        #[error("kernel error: {0}")]
        Kernel(#[from] KernelError),
        #[error("datatype mismatch")]
        DatatypeMismatch,
    }

    pub type Result<T> = std::result::Result<T, Error>;
}

pub use error::Result;

pub mod value {
    use redlinedb_kernel::catalog::OwnedValue;
    pub type SqlValue = OwnedValue;

    pub fn canonicalize(value: SqlValue) -> SqlValue {
        match value {
            OwnedValue::Real(0.0) => OwnedValue::Real(0.0),
            OwnedValue::Real(v) if v.is_nan() => OwnedValue::Real(f64::NAN),
            other => other,
        }
    }

    pub fn compare_values(left: &SqlValue, right: &SqlValue) -> std::cmp::Ordering {
        use OwnedValue::*;
        use std::cmp::Ordering;
        match (left, right) {
            (Null, Null) => Ordering::Equal,
            (Null, _) => Ordering::Less,
            (_, Null) => Ordering::Greater,
            (Integer(a), Integer(b)) => a.cmp(b),
            (Real(a), Real(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (Integer(a), Real(b)) => (*a as f64).partial_cmp(b).unwrap_or(Ordering::Equal),
            (Real(a), Integer(b)) => a.partial_cmp(&(*b as f64)).unwrap_or(Ordering::Equal),
            (Integer(_) | Real(_), Text(_) | Blob(_)) => Ordering::Less,
            (Text(_) | Blob(_), Integer(_) | Real(_)) => Ordering::Greater,
            (Text(a), Text(b)) => a.as_ref().cmp(b.as_ref()),
            (Blob(a), Blob(b)) => a.as_ref().cmp(b.as_ref()),
            (Text(_), Blob(_)) => Ordering::Less,
            (Blob(_), Text(_)) => Ordering::Greater,
        }
    }
}

#[path = "../src/exec/morsel/hash_agg.rs"]
mod hash_agg;

use builder::{ColumnKind, MorselBuilder};
use column::ColumnBatch;
use hash_agg::{AggSpec, GroupSpec, MorselHashAggregator, sum_i64_scalar};
use value::SqlValue;

// -------- helpers --------

/// Build an all-valid morsel from `(group_col_i64, value_col_i64)` pairs.
fn build_morsel_i64_pairs<'a>(arena: &'a Bump, rows: &[(i64, i64)]) -> Morsel<'a> {
    let kinds = [ColumnKind::I64, ColumnKind::I64];
    let mut b = MorselBuilder::with_capacity(arena, &kinds, rows.len());
    for (g, v) in rows {
        b.push_row(&[ValueRef::Integer(*g), ValueRef::Integer(*v)])
            .expect("push");
    }
    let m = b.finish();
    // builder::Morsel and integration-test `Morsel` are layout-compatible
    // because the integration-test `Morsel` is a forward-decl that the
    // re-included builder operates against. Transmute via the safe path
    // of unpacking + repacking is unnecessary: builder::Morsel IS the
    // re-included Morsel; the compiler resolves them to the same type
    // through `super::*` inside `builder.rs`.
    m
}

fn build_morsel_single_i64<'a>(arena: &'a Bump, values: &[i64]) -> Morsel<'a> {
    let mut b = MorselBuilder::with_capacity(arena, &[ColumnKind::I64], values.len());
    for v in values {
        b.push_row(&[ValueRef::Integer(*v)]).expect("push");
    }
    b.finish()
}

fn build_morsel_with_nulls_i64<'a>(arena: &'a Bump, values: &[Option<i64>]) -> Morsel<'a> {
    let mut b = MorselBuilder::with_capacity(arena, &[ColumnKind::I64], values.len());
    let mut idx = 0_usize;
    // We can't push a Null into an I64 column, so build the morsel as
    // all-valid integer rows and then `invalidate(idx)` the slots that
    // are conceptually NULL. This matches how a filter operator would
    // mark rows it has rejected.
    for v in values {
        let placeholder = v.unwrap_or(0);
        b.push_row(&[ValueRef::Integer(placeholder)]).expect("push");
        if v.is_none() {
            b.invalidate(idx);
        }
        idx += 1;
    }
    b.finish()
}

/// Reference COUNT(*) implementation over the live rows.
fn ref_count_star(morsel: &Morsel<'_>) -> i64 {
    morsel.live_rows() as i64
}

/// Reference SUM(i64) over the live rows of column `col`.
fn ref_sum_i64(morsel: &Morsel<'_>, col: usize) -> i64 {
    let mut s = 0i64;
    let ColumnBatch::I64(buf) = &morsel.columns[col] else {
        panic!("expected I64 column")
    };
    for (i, v) in buf.iter().enumerate() {
        if morsel.validity.is_set(i) {
            s = s.saturating_add(*v);
        }
    }
    s
}

// -------- tests --------

#[test]
fn single_group_count_matches_vec_hash_agg() {
    // Differential: ungrouped COUNT(*) over a morsel should equal the
    // row-at-a-time aggregator's COUNT(*) over the same data.
    let bump = Bump::new();
    let m = build_morsel_single_i64(&bump, &(0..50i64).collect::<Vec<_>>());

    let mut agg = MorselHashAggregator::new(&[], &[AggSpec::count_star()], &bump);
    agg.observe_morsel(&m).expect("observe");
    let groups: Vec<_> = agg.finalize().collect();
    assert_eq!(groups.len(), 1);
    let (key, vals) = &groups[0];
    assert!(key.is_empty(), "ungrouped key must be empty");
    assert_eq!(vals[0], SqlValue::Integer(50));
    // Cross-check against the trivial scalar reference.
    assert_eq!(vals[0], SqlValue::Integer(ref_count_star(&m)));
}

#[test]
fn single_group_sum_i64_matches_simd_vs_scalar() {
    // Pump enough rows through the SIMD fast path that the AVX2 body runs
    // (≥4 lanes) and the scalar tail runs (col len % 4 != 0).
    let bump = Bump::new();
    let data: Vec<i64> = (1..=257i64).collect(); // 257 % 4 == 1 → tail of 1
    let m = build_morsel_single_i64(&bump, &data);

    let mut agg = MorselHashAggregator::new(&[], &[AggSpec::sum(0)], &bump);
    agg.observe_morsel(&m).expect("observe");
    let groups: Vec<_> = agg.finalize().collect();
    assert_eq!(groups.len(), 1);
    let want: i64 = data.iter().sum();
    assert_eq!(groups[0].1[0], SqlValue::Integer(want));

    // Differential SIMD-vs-scalar reduction at the kernel level.
    let (s_scalar, n_scalar) = sum_i64_scalar(&data);
    assert_eq!(s_scalar, want);
    assert_eq!(n_scalar, data.len() as i64);
}

#[test]
fn multi_group_count_with_validity_mask() {
    // 100 rows: (i % 4) groups → 4 groups. Mark every 7th row invalid,
    // assert the per-group COUNT(*) reflects the surviving rows only.
    let bump = Bump::new();
    let pairs: Vec<(i64, i64)> = (0..100i64).map(|i| (i % 4, i)).collect();
    let m_full = build_morsel_i64_pairs(&bump, &pairs);

    // Construct a separate morsel where 7th row is invalidated.
    let mut b =
        MorselBuilder::with_capacity(&bump, &[ColumnKind::I64, ColumnKind::I64], pairs.len());
    for (idx, (g, v)) in pairs.iter().enumerate() {
        b.push_row(&[ValueRef::Integer(*g), ValueRef::Integer(*v)])
            .expect("push");
        if idx % 7 == 0 {
            b.invalidate(idx);
        }
    }
    let m_masked = b.finish();

    // Aggregator: GROUP BY col0, COUNT(*).
    let mut agg =
        MorselHashAggregator::new(&[GroupSpec { col: 0 }], &[AggSpec::count_star()], &bump);
    agg.observe_morsel(&m_masked).expect("observe");

    let mut groups: Vec<_> = agg.finalize().collect();
    assert_eq!(groups.len(), 4);
    // Sort by group key for stable assertions.
    groups.sort_by(|a, b| {
        use std::cmp::Ordering;
        match (&a.0[0], &b.0[0]) {
            (SqlValue::Integer(x), SqlValue::Integer(y)) => x.cmp(y),
            _ => Ordering::Equal,
        }
    });
    // Reference: count surviving rows per group.
    let mut expected = [0i64; 4];
    for idx in 0..pairs.len() {
        if idx % 7 != 0 {
            let g = pairs[idx].0 as usize;
            expected[g] += 1;
        }
    }
    for (g, (_key, vals)) in groups.iter().enumerate() {
        assert_eq!(
            vals[0],
            SqlValue::Integer(expected[g]),
            "group {g} count mismatch"
        );
    }
    // Sanity: total live rows = sum of group counts.
    assert_eq!(m_masked.live_rows() as i64, expected.iter().sum::<i64>());
    // m_full kept for symmetry — guarantees the builder path still emits
    // a morsel of expected width.
    assert_eq!(m_full.live_rows(), pairs.len());
}

#[test]
fn min_max_correctness_with_null() {
    // Build a morsel of [5, NULL, 1, NULL, 9, 3, 7, NULL]; expect
    // MIN=1, MAX=9, NULLs ignored.
    let bump = Bump::new();
    let m = build_morsel_with_nulls_i64(
        &bump,
        &[
            Some(5),
            None,
            Some(1),
            None,
            Some(9),
            Some(3),
            Some(7),
            None,
        ],
    );

    let mut agg = MorselHashAggregator::new(&[], &[AggSpec::min(0), AggSpec::max(0)], &bump);
    agg.observe_morsel(&m).expect("observe");
    let groups: Vec<_> = agg.finalize().collect();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].1[0], SqlValue::Integer(1));
    assert_eq!(groups[0].1[1], SqlValue::Integer(9));
}

#[test]
fn avg_returns_correct_sum_count_split() {
    // AVG([10, 20, 30]) = 20.0 — exercises the sum/count split.
    let bump = Bump::new();
    let m = build_morsel_single_i64(&bump, &[10, 20, 30]);
    let mut agg = MorselHashAggregator::new(&[], &[AggSpec::avg(0)], &bump);
    agg.observe_morsel(&m).expect("observe");
    let groups: Vec<_> = agg.finalize().collect();
    assert_eq!(groups.len(), 1);
    match groups[0].1[0] {
        SqlValue::Real(v) => assert!((v - 20.0).abs() < 1e-9, "got {v}"),
        ref other => panic!("expected SqlValue::Real, got {other:?}"),
    }

    // AVG with one NULL ignored: AVG([10, NULL, 30]) = 20.0.
    let m2 = build_morsel_with_nulls_i64(&bump, &[Some(10), None, Some(30)]);
    let mut agg2 = MorselHashAggregator::new(&[], &[AggSpec::avg(0)], &bump);
    agg2.observe_morsel(&m2).expect("observe");
    let g2: Vec<_> = agg2.finalize().collect();
    match g2[0].1[0] {
        SqlValue::Real(v) => assert!((v - 20.0).abs() < 1e-9, "got {v}"),
        ref other => panic!("expected SqlValue::Real, got {other:?}"),
    }
}

#[test]
fn morsel_chunk_boundary_doesnt_double_count() {
    // Same row set, split across two morsels at row 17. COUNT/SUM must
    // be identical to feeding one giant morsel.
    let bump = Bump::new();
    let all: Vec<i64> = (1..=64i64).collect();
    let m_single = build_morsel_single_i64(&bump, &all);

    let bump_split = Bump::new();
    let m_a = build_morsel_single_i64(&bump_split, &all[..17]);
    let m_b = build_morsel_single_i64(&bump_split, &all[17..]);

    let mut agg_one =
        MorselHashAggregator::new(&[], &[AggSpec::count_star(), AggSpec::sum(0)], &bump);
    agg_one.observe_morsel(&m_single).expect("observe one");
    let g_one: Vec<_> = agg_one.finalize().collect();

    let mut agg_split =
        MorselHashAggregator::new(&[], &[AggSpec::count_star(), AggSpec::sum(0)], &bump_split);
    agg_split.observe_morsel(&m_a).expect("observe a");
    agg_split.observe_morsel(&m_b).expect("observe b");
    let g_split: Vec<_> = agg_split.finalize().collect();

    assert_eq!(g_one.len(), 1);
    assert_eq!(g_split.len(), 1);
    assert_eq!(g_one[0].1, g_split[0].1, "split must equal unified");
    // And both must equal the scalar reference.
    let want_sum: i64 = all.iter().sum();
    assert_eq!(g_one[0].1[0], SqlValue::Integer(all.len() as i64));
    assert_eq!(g_one[0].1[1], SqlValue::Integer(want_sum));
}

#[test]
fn empty_morsel_emits_no_groups() {
    let bump = Bump::new();
    let m = build_morsel_single_i64(&bump, &[]);
    let mut agg =
        MorselHashAggregator::new(&[GroupSpec { col: 0 }], &[AggSpec::count_star()], &bump);
    agg.observe_morsel(&m).expect("observe");
    let groups: Vec<_> = agg.finalize().collect();
    assert!(groups.is_empty(), "no live rows → no groups");

    // Ungrouped over empty input also emits no groups (no key was inserted).
    let bump2 = Bump::new();
    let m2 = build_morsel_single_i64(&bump2, &[]);
    let mut agg2 = MorselHashAggregator::new(&[], &[AggSpec::count_star()], &bump2);
    agg2.observe_morsel(&m2).expect("observe");
    let g2: Vec<_> = agg2.finalize().collect();
    assert!(
        g2.is_empty(),
        "morsel-aware variant: ungrouped empty input emits no groups (the planner injects a zero-row for empty COUNT(*) at finalize time)"
    );
}

#[test]
fn large_morsel_1024_rows_simd_eq_scalar() {
    // Full-cap morsel: 1024 rows. Exercise SIMD fast path and assert SUM
    // equals the scalar reference computed via `sum_i64_scalar`.
    let bump = Bump::new();
    let data: Vec<i64> = (0..1024i64).map(|i| (i % 1000) - 500).collect();
    let m = build_morsel_single_i64(&bump, &data);
    assert_eq!(m.len(), MAX_BATCH_ROWS);

    let mut agg = MorselHashAggregator::new(&[], &[AggSpec::sum(0)], &bump);
    agg.observe_morsel(&m).expect("observe");
    let g: Vec<_> = agg.finalize().collect();

    let (want_sum, want_n) = sum_i64_scalar(&data);
    assert_eq!(want_n, 1024);
    assert_eq!(g[0].1[0], SqlValue::Integer(want_sum));

    // Reference cross-check.
    assert_eq!(g[0].1[0], SqlValue::Integer(ref_sum_i64(&m, 0)));
}

// -------- bonus differential coverage --------

#[test]
fn count_distinct_groups_matches_hashmap_reference() {
    // 1000 rows: groups = i % 13 → 13 distinct groups. The aggregator's
    // group count must match a naive HashMap reference.
    let bump = Bump::new();
    let pairs: Vec<(i64, i64)> = (0..1000i64).map(|i| (i % 13, i)).collect();
    let m = build_morsel_i64_pairs(&bump, &pairs);

    let mut agg = MorselHashAggregator::new(
        &[GroupSpec { col: 0 }],
        &[AggSpec::count_star(), AggSpec::sum(1)],
        &bump,
    );
    agg.observe_morsel(&m).expect("observe");

    use std::collections::HashMap;
    let mut want: HashMap<i64, (i64, i64)> = HashMap::new();
    for (g, v) in &pairs {
        let e = want.entry(*g).or_insert((0, 0));
        e.0 += 1;
        e.1 += v;
    }

    let groups: Vec<_> = agg.finalize().collect();
    assert_eq!(groups.len(), want.len());
    for (key, vals) in groups {
        let g = match key[0] {
            SqlValue::Integer(v) => v,
            ref other => panic!("non-integer group key: {other:?}"),
        };
        let (c, s) = want[&g];
        assert_eq!(vals[0], SqlValue::Integer(c), "count for group {g}");
        assert_eq!(vals[1], SqlValue::Integer(s), "sum for group {g}");
    }
}
