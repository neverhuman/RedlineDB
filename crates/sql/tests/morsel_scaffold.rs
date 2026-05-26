//! Phase 6 Candidate 1 (Morsel/Vector) scaffolding tests.
//!
//! The morsel module is `pub(crate)` inside `redlinedb-sql`, so this
//! integration test re-includes the source files via `#[path = "..."]`
//! and exercises the primitives against a private compilation copy.
//! This mirrors how `parity_oracle/harness.rs` is shared across tests
//! without requiring it to be a public re-export.
//!
//! Scope: pure scaffolding. No operator wiring, no SIMD, no executor
//! integration — those land in M2-M8 per `docs/phase6-morsel-vector.md`.
#![allow(dead_code)]

use smallvec::SmallVec;

#[path = "../src/exec/morsel/arena.rs"]
mod arena;
#[path = "../src/exec/morsel/bitmap.rs"]
mod bitmap;
#[path = "../src/exec/morsel/column.rs"]
mod column;

// `builder.rs` references `super::{MAX_BATCH_ROWS, Morsel}` and
// `super::arena::BytesArena` etc. When included via `#[path]` here,
// `super` resolves to this test root, so we hoist the morsel-root items
// (mirroring `crates/sql/src/exec/morsel/mod.rs`) to module scope so the
// builder source compiles unchanged.
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

use bumpalo::Bump;
use redlinedb_kernel::catalog::ValueRef;
use smallvec::smallvec;

use arena::BytesArena;
use bitmap::Bitmap;
use builder::{ColumnKind, MorselBuilder};
use column::ColumnBatch;

// -------- Bitmap tests --------

#[test]
fn bitmap_set_clear_count_roundtrip() {
    let mut b = Bitmap::new(200);
    assert_eq!(b.count_ones(), 0);
    for i in (0..200).step_by(7) {
        b.set(i);
    }
    let expected = (0..200).step_by(7).count();
    assert_eq!(b.count_ones(), expected);
    for i in (0..200).step_by(7) {
        assert!(b.is_set(i));
        b.clear(i);
    }
    assert_eq!(b.count_ones(), 0);
}

#[test]
fn bitmap_and_or_match_reference() {
    let mut a = Bitmap::new(257);
    let mut c = Bitmap::new(257);
    for i in 0..257 {
        if i % 3 == 0 {
            a.set(i);
        }
        if i % 5 == 0 {
            c.set(i);
        }
    }
    let mut and_b = a.clone();
    and_b.and(&c);
    let expected_and = (0..257).filter(|i| i % 3 == 0 && i % 5 == 0).count();
    assert_eq!(and_b.count_ones(), expected_and);

    let mut or_b = a.clone();
    or_b.or(&c);
    let expected_or = (0..257).filter(|i| i % 3 == 0 || i % 5 == 0).count();
    assert_eq!(or_b.count_ones(), expected_or);
}

#[test]
fn bitmap_full_capacity_round_trips() {
    let mut b = Bitmap::new(MAX_BATCH_ROWS);
    for i in 0..MAX_BATCH_ROWS {
        b.set(i);
    }
    assert_eq!(b.count_ones(), MAX_BATCH_ROWS);
    for i in 0..MAX_BATCH_ROWS {
        assert!(b.is_set(i));
    }
}

// -------- BytesArena tests --------

#[test]
fn arena_push_get_byte_identical() {
    let bump = Bump::new();
    let mut a = BytesArena::new(&bump, 8);
    let payloads: Vec<&[u8]> = vec![
        b"alpha",
        b"",
        b"beta gamma",
        b"\x00\x01\x02",
        b"long-ish payload with spaces",
    ];
    for p in &payloads {
        a.push(p);
    }
    assert_eq!(a.len(), payloads.len());
    for (i, expected) in payloads.iter().enumerate() {
        assert_eq!(a.get(i).unwrap(), *expected);
    }
    assert!(a.get(payloads.len()).is_none());
}

#[test]
fn arena_offsets_match_lengths() {
    let bump = Bump::new();
    let mut a = BytesArena::new(&bump, 4);
    let payloads: Vec<&[u8]> = vec![b"aa", b"bbb", b"c", b"dddd"];
    for p in &payloads {
        a.push(p);
    }
    let mut cumulative = 0u32;
    for (i, p) in payloads.iter().enumerate() {
        assert_eq!(a.offsets[i], cumulative);
        cumulative += p.len() as u32;
        assert_eq!(a.offsets[i + 1], cumulative);
    }
}

// -------- ColumnBatch tests --------

#[test]
fn column_batch_get_dispatch() {
    let bump = Bump::new();
    let i = ColumnBatch::I64(smallvec![10, 20, 30]);
    assert_eq!(i.len(), 3);
    assert!(matches!(i.get(1), Some(ValueRef::Integer(20))));

    let f = ColumnBatch::F64(smallvec![1.5, 2.5]);
    assert!(matches!(f.get(0), Some(ValueRef::Real(v)) if v == 1.5));

    let mut arena = BytesArena::new(&bump, 1);
    arena.push(b"hello");
    let t = ColumnBatch::Text(arena);
    assert!(matches!(t.get(0), Some(ValueRef::Text("hello"))));

    let n: ColumnBatch<'_> = ColumnBatch::Null { len: 4 };
    assert_eq!(n.len(), 4);
    assert!(matches!(n.get(0), Some(ValueRef::Null)));
}

// -------- Builder + Morsel tests --------

#[test]
fn builder_push_finish_yields_correct_morsel() {
    let bump = Bump::new();
    let kinds = [ColumnKind::I64, ColumnKind::Text];
    let mut b = MorselBuilder::with_capacity(&bump, &kinds, 4);
    assert_eq!(b.column_count(), 2);
    assert_eq!(b.capacity(), 4);

    b.push_row(&[ValueRef::Integer(1), ValueRef::Text("a")])
        .unwrap();
    b.push_row(&[ValueRef::Integer(2), ValueRef::Text("bb")])
        .unwrap();
    b.push_row(&[ValueRef::Integer(3), ValueRef::Text("ccc")])
        .unwrap();

    let m: Morsel<'_> = b.finish();
    assert_eq!(m.len(), 3);
    assert_eq!(m.width(), 2);
    assert_eq!(m.live_rows(), 3);

    let c0 = &m.columns[0];
    assert!(matches!(c0.get(2), Some(ValueRef::Integer(3))));
    let c1 = &m.columns[1];
    assert!(matches!(c1.get(0), Some(ValueRef::Text("a"))));
    assert!(matches!(c1.get(2), Some(ValueRef::Text("ccc"))));
}

#[test]
fn builder_rejects_shape_mismatch_and_overflow() {
    let bump = Bump::new();
    let kinds = [ColumnKind::I64];
    let mut b = MorselBuilder::with_capacity(&bump, &kinds, 1);
    assert!(
        b.push_row(&[ValueRef::Integer(1), ValueRef::Integer(2)])
            .is_err()
    );
    assert!(b.push_row(&[ValueRef::Text("nope")]).is_err());
    b.push_row(&[ValueRef::Integer(7)]).unwrap();
    assert!(b.push_row(&[ValueRef::Integer(8)]).is_err());
}

#[test]
fn builder_invalidate_drops_live_count() {
    let bump = Bump::new();
    let kinds = [ColumnKind::I64];
    let mut b = MorselBuilder::with_capacity(&bump, &kinds, 8);
    for v in 0..5i64 {
        b.push_row(&[ValueRef::Integer(v)]).unwrap();
    }
    b.invalidate(1);
    b.invalidate(3);
    let m = b.finish();
    assert_eq!(m.len(), 5);
    assert_eq!(m.live_rows(), 3);
}

#[test]
fn builder_full_1024_row_morsel_layout() {
    let bump = Bump::new();
    let kinds = [ColumnKind::I64, ColumnKind::I64, ColumnKind::Text];
    let mut b = MorselBuilder::with_capacity(&bump, &kinds, MAX_BATCH_ROWS);
    let mut owned: Vec<String> = Vec::with_capacity(MAX_BATCH_ROWS);
    for i in 0..MAX_BATCH_ROWS {
        owned.push(format!("row-{}", i));
    }
    for i in 0..MAX_BATCH_ROWS {
        let row = [
            ValueRef::Integer(i as i64),
            ValueRef::Integer((i as i64) * 2),
            ValueRef::Text(owned[i].as_str()),
        ];
        b.push_row(&row).unwrap();
    }
    let m = b.finish();
    assert_eq!(m.len(), MAX_BATCH_ROWS);
    assert_eq!(m.width(), 3);
    assert_eq!(m.live_rows(), MAX_BATCH_ROWS);
    assert_eq!(m.columns[0].len(), MAX_BATCH_ROWS);
    assert_eq!(m.columns[1].len(), MAX_BATCH_ROWS);
    assert_eq!(m.columns[2].len(), MAX_BATCH_ROWS);

    assert!(matches!(m.columns[0].get(0), Some(ValueRef::Integer(0))));
    assert!(matches!(
        m.columns[0].get(1023),
        Some(ValueRef::Integer(1023))
    ));
    assert!(matches!(
        m.columns[1].get(500),
        Some(ValueRef::Integer(1000))
    ));
    assert!(matches!(
        m.columns[2].get(42),
        Some(ValueRef::Text(s)) if s == "row-42"
    ));
}

#[test]
fn empty_morsel_is_well_formed() {
    let m: Morsel<'_> = Morsel::new();
    assert_eq!(m.len(), 0);
    assert_eq!(m.width(), 0);
    assert!(m.is_empty());
    assert_eq!(m.live_rows(), 0);
}
