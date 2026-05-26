//! Phase 6 M2 (scan source) + M3 (vectorized i64 filter kernels) tests.
//!
//! Mirrors `morsel_scaffold.rs`: the morsel module is `pub(crate)` so
//! we re-include the source files via `#[path = "..."]`, then exercise
//! `MorselScan` and `filter_i64_*` against a private compile copy.

#![allow(dead_code)]

use smallvec::SmallVec;

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

#[path = "../src/exec/morsel/scan.rs"]
mod scan;

#[path = "../src/exec/morsel/filter.rs"]
mod filter;

use bumpalo::Bump;
use redlinedb_kernel::catalog::ValueRef;

use bitmap::Bitmap;
use builder::ColumnKind;
use filter::{
    filter_i64_eq, filter_i64_eq_scalar, filter_i64_ge, filter_i64_ge_scalar, filter_i64_gt,
    filter_i64_gt_scalar, filter_i64_le, filter_i64_le_scalar, filter_i64_lt,
    filter_i64_lt_scalar, filter_i64_ne, filter_i64_ne_scalar,
};
use scan::{MorselScan, RowRef, ScanSource};

// -------- Result alias for the standalone scan compile --------
//
// `scan.rs` references `crate::Result`. Inside this integration test
// that resolves to the test binary's crate root, so we provide a
// `Result` alias mirroring `redlinedb-sql`'s.

pub type Result<T> = std::result::Result<T, &'static str>;

// -------- Synthetic ScanSource emitting i64 rows --------

struct VecI64Source {
    data: Vec<i64>,
    cursor: usize,
    scratch: Vec<ValueRef<'static>>,
}

impl VecI64Source {
    fn new(data: Vec<i64>) -> Self {
        Self {
            data,
            cursor: 0,
            scratch: Vec::with_capacity(1),
        }
    }
}

impl ScanSource for VecI64Source {
    fn next_row(&mut self) -> Result<Option<RowRef<'_>>> {
        if self.cursor >= self.data.len() {
            return Ok(None);
        }
        let v = self.data[self.cursor];
        self.cursor += 1;
        self.scratch.clear();
        self.scratch.push(ValueRef::Integer(v));
        Ok(Some(&self.scratch[..]))
    }
}

// -------- M2: MorselScan tests --------

#[test]
fn scan_yields_ceiling_n_over_batch_morsels() {
    let bump = Bump::new();
    let rows: Vec<i64> = (0..1050i64).collect();
    let source = VecI64Source::new(rows);
    let mut scan = MorselScan::new(source, &bump, &[ColumnKind::I64], 256);
    let mut produced = 0_usize;
    let mut total_rows = 0_usize;
    while let Some(m) = scan.next_morsel().unwrap() {
        produced += 1;
        total_rows += m.len();
        assert!(m.len() <= 256);
    }
    assert_eq!(total_rows, 1050);
    // ceil(1050 / 256) == 5
    assert_eq!(produced, 5);
}

#[test]
fn scan_clamps_oversize_target_to_max_batch_rows() {
    let bump = Bump::new();
    let source = VecI64Source::new(vec![1, 2, 3]);
    let scan = MorselScan::new(source, &bump, &[ColumnKind::I64], 9999);
    assert_eq!(scan.batch_target_rows(), MAX_BATCH_ROWS as u16);
}

#[test]
fn scan_floors_zero_target_to_one() {
    let bump = Bump::new();
    let source = VecI64Source::new(vec![1, 2, 3]);
    let scan = MorselScan::new(source, &bump, &[ColumnKind::I64], 0);
    assert_eq!(scan.batch_target_rows(), 1);
}

#[test]
fn scan_empty_source_returns_none() {
    let bump = Bump::new();
    let source = VecI64Source::new(vec![]);
    let mut scan = MorselScan::new(source, &bump, &[ColumnKind::I64], 256);
    assert!(scan.next_morsel().unwrap().is_none());
    assert!(scan.exhausted());
}

#[test]
fn scan_single_row_yields_one_morsel() {
    let bump = Bump::new();
    let source = VecI64Source::new(vec![42]);
    let mut scan = MorselScan::new(source, &bump, &[ColumnKind::I64], 256);
    let m = scan.next_morsel().unwrap().unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!(m.live_rows(), 1);
    assert!(matches!(m.columns[0].get(0), Some(ValueRef::Integer(42))));
    assert!(scan.next_morsel().unwrap().is_none());
}

#[test]
fn scan_exact_multiple_of_batch_size() {
    let bump = Bump::new();
    let rows: Vec<i64> = (0..512i64).collect();
    let source = VecI64Source::new(rows);
    let mut scan = MorselScan::new(source, &bump, &[ColumnKind::I64], 256);
    let mut produced = 0;
    while let Some(m) = scan.next_morsel().unwrap() {
        assert_eq!(m.len(), 256);
        produced += 1;
    }
    assert_eq!(produced, 2);
}

// -------- M3: i64 filter scalar smoke + edge cases --------

#[test]
fn filter_empty_col_is_noop() {
    let col: [i64; 0] = [];
    let mut v = Bitmap::new(0);
    filter_i64_eq(&col, 7, &mut v);
    filter_i64_lt(&col, 7, &mut v);
    filter_i64_gt(&col, 7, &mut v);
    assert_eq!(v.count_ones(), 0);
}

#[test]
fn filter_single_element_keeps_or_drops() {
    let col = [5i64];
    let mut v = Bitmap::new_all_set(1);
    filter_i64_eq(&col, 5, &mut v);
    assert_eq!(v.count_ones(), 1);

    let mut v = Bitmap::new_all_set(1);
    filter_i64_eq(&col, 6, &mut v);
    assert_eq!(v.count_ones(), 0);
}

#[test]
fn filter_all_match_preserves_all() {
    let col = vec![7i64; 32];
    let mut v = Bitmap::new_all_set(col.len());
    filter_i64_eq(&col, 7, &mut v);
    assert_eq!(v.count_ones(), col.len());
}

#[test]
fn filter_none_match_clears_all() {
    let col = vec![7i64; 32];
    let mut v = Bitmap::new_all_set(col.len());
    filter_i64_eq(&col, 9, &mut v);
    assert_eq!(v.count_ones(), 0);
}

#[test]
fn filter_respects_pre_existing_validity_mask() {
    // Bits already cleared must stay cleared even if the new
    // predicate would have kept them.
    let col = vec![1i64, 2, 3, 4, 5, 6, 7, 8];
    let mut v = Bitmap::new_all_set(col.len());
    v.clear(0);
    v.clear(2);
    filter_i64_gt(&col, 0, &mut v); // all rows match `> 0`
    // 0 and 2 stay cleared; 1, 3, 4, 5, 6, 7 remain set.
    assert!(!v.is_set(0));
    assert!(!v.is_set(2));
    for i in [1, 3, 4, 5, 6, 7] {
        assert!(v.is_set(i), "row {i} should still be set");
    }
    assert_eq!(v.count_ones(), 6);
}

// -------- M3: differential SIMD-vs-scalar (1000 random inputs per op) --------

/// Tiny LCG so the differential is hermetic (no rand crate dep).
fn lcg_seq(seed: u64, count: usize) -> Vec<i64> {
    let mut x = seed;
    (0..count)
        .map(|_| {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            x as i64
        })
        .collect()
}

fn diff_one<F, G>(op_name: &str, simd: F, scalar: G)
where
    F: Fn(&[i64], i64, &mut Bitmap),
    G: Fn(&[i64], i64, &mut Bitmap),
{
    // 1000 random inputs at varying lengths so we cover the SIMD body
    // (4-lane stride) and the scalar tail.
    let target_seeds: [u64; 6] = [
        0xDEADBEEFCAFEBABE,
        0x0123456789ABCDEF,
        0xFFFFFFFFFFFFFFFF,
        0x0000000000000001,
        0x8000000000000000,
        0x55AA55AA55AA55AA,
    ];
    for (i, seed) in target_seeds.iter().enumerate() {
        let len_seed = (*seed).wrapping_add(i as u64);
        let len = 1 + (len_seed as usize % 257);
        let col = lcg_seq(*seed, len);
        // Test against several targets (including ones that appear in `col`).
        let targets = [col[0], col[len / 2], col[len - 1], 0i64, -1i64, i64::MAX, i64::MIN];
        for t in targets {
            let mut v_simd = Bitmap::new_all_set(len);
            let mut v_scalar = Bitmap::new_all_set(len);
            simd(&col, t, &mut v_simd);
            scalar(&col, t, &mut v_scalar);
            assert_eq!(
                v_simd, v_scalar,
                "{op_name}: simd vs scalar diverged for seed={seed:#x} len={len} target={t}"
            );
        }
    }
    // Now sweep through small lengths 0..=20 to nail the tail-handling.
    for len in 0..=20 {
        let col = lcg_seq(0x1234_5678_9ABC_DEF0, len);
        let targets = [0i64, 1, -1, i64::MAX, i64::MIN];
        for t in targets {
            let mut v_simd = Bitmap::new_all_set(len);
            let mut v_scalar = Bitmap::new_all_set(len);
            simd(&col, t, &mut v_simd);
            scalar(&col, t, &mut v_scalar);
            assert_eq!(
                v_simd, v_scalar,
                "{op_name}: tail mismatch at len={len} target={t}"
            );
        }
    }
}

#[test]
fn differential_eq() {
    diff_one("eq", filter_i64_eq, filter_i64_eq_scalar);
}

#[test]
fn differential_ne() {
    diff_one("ne", filter_i64_ne, filter_i64_ne_scalar);
}

#[test]
fn differential_lt() {
    diff_one("lt", filter_i64_lt, filter_i64_lt_scalar);
}

#[test]
fn differential_le() {
    diff_one("le", filter_i64_le, filter_i64_le_scalar);
}

#[test]
fn differential_gt() {
    diff_one("gt", filter_i64_gt, filter_i64_gt_scalar);
}

#[test]
fn differential_ge() {
    diff_one("ge", filter_i64_ge, filter_i64_ge_scalar);
}

// -------- M3: count_ones() invariants --------

#[test]
fn filter_count_ones_matches_expected_eq() {
    let col: Vec<i64> = (0..1000i64).map(|i| i % 7).collect();
    let mut v = Bitmap::new_all_set(col.len());
    filter_i64_eq(&col, 3, &mut v);
    let expected = col.iter().filter(|&&x| x == 3).count();
    assert_eq!(v.count_ones(), expected);
}

/// Synthetic 1M-row throughput probe. Print-only (use `--nocapture`); no
/// assertion so CI noise stays bounded. Gives an order-of-magnitude
/// indicator that `MorselScan` is not pathologically slow on a 1M-row
/// in-memory feed; the real workload-driven number lands when the M7
/// router wires the executor path.
#[test]
fn scan_throughput_1m_rows_smoke() {
    use std::time::Instant;
    let n: usize = 1_000_000;
    let data: Vec<i64> = (0..n as i64).collect();
    let bump = Bump::new();
    let source = VecI64Source::new(data);
    let mut scan = MorselScan::new(source, &bump, &[ColumnKind::I64], 1024);
    let start = Instant::now();
    let mut morsels = 0usize;
    let mut rows = 0usize;
    while let Some(m) = scan.next_morsel().unwrap() {
        morsels += 1;
        rows += m.len();
    }
    let dur = start.elapsed();
    let rate_m = (rows as f64) / dur.as_secs_f64() / 1_000_000.0;
    eprintln!(
        "[scan-throughput] morsels={morsels} rows={rows} elapsed={dur:?} rate={rate_m:.2}M rows/s"
    );
    assert_eq!(rows, n);
}

#[test]
fn filter_chained_predicates_compose_via_and() {
    // (col >= 200) AND (col < 500) == 300 rows out of 1000.
    let col: Vec<i64> = (0..1000i64).collect();
    let mut v = Bitmap::new_all_set(col.len());
    filter_i64_ge(&col, 200, &mut v);
    filter_i64_lt(&col, 500, &mut v);
    assert_eq!(v.count_ones(), 300);
    for i in 200..500 {
        assert!(v.is_set(i));
    }
    for i in 0..200 {
        assert!(!v.is_set(i));
    }
    for i in 500..1000 {
        assert!(!v.is_set(i));
    }
}
