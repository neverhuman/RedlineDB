//! Phase 6 Morsel/Vector executor — scaffolding only.
//!
//! Defines the columnar batch primitives (`Morsel`, `ColumnBatch`,
//! `Bitmap`, `BytesArena`) and a tuple-to-morsel `MorselBuilder` adapter.
//! NO operator wiring yet; M2-M8 land separately per
//! `docs/phase6-morsel-vector.md`. The `dead_code` allow is intentional
//! for this scaffolding wave — the consumers arrive in M2 (scan) and M3
//! (filter kernels); without the allow, the entire module trips
//! `unused_*` until those land.
//!
//! W4-T: observe-only morsel-eligibility telemetry. Activated by
//! `REDLINE_MORSEL_TELEMETRY=1`. The classifier in
//! [`classify_select_plan_eligibility`] runs at `execute_select` entry
//! (`crates/sql/src/exec/select_top.rs`) and increments the static counters
//! below. NEVER changes execution path — purely diagnostic so future W4
//! routing decisions are evidence-driven.
#![allow(dead_code)]

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use smallvec::SmallVec;

pub mod arena;
pub mod bitmap;
pub mod builder;
pub mod column;
pub mod filter;
pub mod hash_agg;
pub mod route;
pub mod scan;

#[allow(unused_imports)]
pub use arena::BytesArena;
#[allow(unused_imports)]
pub use bitmap::Bitmap;
#[allow(unused_imports)]
pub use builder::{ColumnKind, MorselBuilder};
#[allow(unused_imports)]
pub use column::ColumnBatch;
#[allow(unused_imports)]
pub use filter::{
    filter_i64_eq, filter_i64_ge, filter_i64_gt, filter_i64_le, filter_i64_lt, filter_i64_ne,
};
#[allow(unused_imports)]
pub use hash_agg::{AggKind as MorselAggKind, AggSpec, GroupSpec, MorselHashAggregator};
#[allow(unused_imports)]
pub use scan::{MorselScan, RowRef, ScanSource};

/// Hard upper bound on rows per morsel. Matches the inline-storage budget
/// of `Bitmap` (16 × u64 = 1024 bits).
pub const MAX_BATCH_ROWS: usize = 1024;

// W4-T: morsel-eligibility classifier + counters. Observe-only; never
// changes execution. Increments fire only when `REDLINE_MORSEL_TELEMETRY=1`.

/// Classification of a SELECT plan's morsel-routing potential.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MorselEligibility {
    /// Single-table scan, no aggregates, no DISTINCT, no HAVING.
    PrimitiveScan,
    /// Numeric aggregate on a single table, ungrouped or single-key GROUP BY.
    PrimitiveAgg,
    /// Single-table scan with shape extensions deferred from initial routing
    /// (DISTINCT, multi-key GROUP BY, HAVING).
    DeferredShape,
    /// Joins / CTEs / subqueries / windows — stays on tuple path indefinitely.
    NotEligible,
}

pub static MORSEL_QUERIES_PRIMITIVE_SCAN: AtomicU64 = AtomicU64::new(0);
pub static MORSEL_QUERIES_PRIMITIVE_AGG: AtomicU64 = AtomicU64::new(0);
pub static MORSEL_QUERIES_DEFERRED_SHAPE: AtomicU64 = AtomicU64::new(0);
pub static MORSEL_QUERIES_NOT_ELIGIBLE: AtomicU64 = AtomicU64::new(0);

// W4-A1: routing decision counters. Incremented from
// `morsel::route::route_primitive_scan` when telemetry is enabled. The
// `_USED` counter is incremented only on actual routing wins; the
// `_FALLBACK_*` counters split by fallback reason so we can identify
// which constraint is firing most often in the corpus.
pub static MORSEL_ROUTE_USED: AtomicU64 = AtomicU64::new(0);
pub static MORSEL_ROUTE_FALLBACK_DISABLED: AtomicU64 = AtomicU64::new(0);
pub static MORSEL_ROUTE_FALLBACK_INELIGIBLE: AtomicU64 = AtomicU64::new(0);
pub static MORSEL_ROUTE_FALLBACK_PROJECTION: AtomicU64 = AtomicU64::new(0);
pub static MORSEL_ROUTE_FALLBACK_PREDICATE: AtomicU64 = AtomicU64::new(0);
pub static MORSEL_ROUTE_FALLBACK_DYNAMIC_KIND: AtomicU64 = AtomicU64::new(0);
pub static MORSEL_ROUTE_FALLBACK_SHAPE: AtomicU64 = AtomicU64::new(0);

/// True when `REDLINE_MORSEL_TELEMETRY=1` is set. Cached on first call.
pub fn morsel_telemetry_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var_os("REDLINE_MORSEL_TELEMETRY")
            .map(|v| v != "0" && !v.is_empty())
            .unwrap_or(false)
    })
}

/// W4-A1: parsed `REDLINE_MORSEL_ROUTE` env var. `None` means "stay on
/// tuple path" (the default); `Some(MorselRouteMode::PrimitiveScan)` opts
/// into the W4-A1 primitive-scan routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MorselRouteMode {
    /// Route eligible single-table all-primitive-column scans through the
    /// morsel filter + emit pipeline. Tuple path stays as the fallback.
    PrimitiveScan,
}

/// Cached `REDLINE_MORSEL_ROUTE` parse. Unrecognised values are rejected
/// with a stderr notice and treated as off.
pub fn morsel_route_mode() -> Option<MorselRouteMode> {
    static MODE: OnceLock<Option<MorselRouteMode>> = OnceLock::new();
    *MODE.get_or_init(|| {
        let raw = std::env::var("REDLINE_MORSEL_ROUTE").ok()?;
        match raw.trim().to_ascii_lowercase().as_str() {
            "primitive_scan" | "primitive-scan" | "all" | "1" | "on" => {
                Some(MorselRouteMode::PrimitiveScan)
            }
            "" | "0" | "off" => None,
            other => {
                eprintln!(
                    "redlinedb: REDLINEDB_MORSEL_ROUTE={other:?} not recognised; \
                     valid values: primitive_scan, all, on, 1, off, 0"
                );
                None
            }
        }
    })
}

/// True when W4 morsel observation/routing needs the SELECT classifier.
/// Default-OFF runs should not pay classifier/tap overhead.
pub fn morsel_observation_or_route_enabled() -> bool {
    morsel_telemetry_enabled() || morsel_route_mode().is_some()
}

/// Increment the matching counter when telemetry is enabled. No-op otherwise.
#[inline]
pub fn record_morsel_eligibility(kind: MorselEligibility) {
    if !morsel_telemetry_enabled() {
        return;
    }
    match kind {
        MorselEligibility::PrimitiveScan => {
            MORSEL_QUERIES_PRIMITIVE_SCAN.fetch_add(1, Ordering::Relaxed);
        }
        MorselEligibility::PrimitiveAgg => {
            MORSEL_QUERIES_PRIMITIVE_AGG.fetch_add(1, Ordering::Relaxed);
        }
        MorselEligibility::DeferredShape => {
            MORSEL_QUERIES_DEFERRED_SHAPE.fetch_add(1, Ordering::Relaxed);
        }
        MorselEligibility::NotEligible => {
            MORSEL_QUERIES_NOT_ELIGIBLE.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Snapshot of the four counters, for end-of-run dumps.
pub fn snapshot_morsel_eligibility() -> [(MorselEligibility, u64); 4] {
    [
        (
            MorselEligibility::PrimitiveScan,
            MORSEL_QUERIES_PRIMITIVE_SCAN.load(Ordering::Relaxed),
        ),
        (
            MorselEligibility::PrimitiveAgg,
            MORSEL_QUERIES_PRIMITIVE_AGG.load(Ordering::Relaxed),
        ),
        (
            MorselEligibility::DeferredShape,
            MORSEL_QUERIES_DEFERRED_SHAPE.load(Ordering::Relaxed),
        ),
        (
            MorselEligibility::NotEligible,
            MORSEL_QUERIES_NOT_ELIGIBLE.load(Ordering::Relaxed),
        ),
    ]
}

/// Classify a `SelectPlan` for morsel-routing eligibility. Pure; no execution.
pub fn classify_select_plan_eligibility(
    plan: &crate::statement::SelectPlan,
) -> MorselEligibility {
    use crate::statement::SelectSource;
    if !matches!(plan.source, SelectSource::Table(_)) {
        return MorselEligibility::NotEligible;
    }
    if plan.distinct || !plan.distinct_on.is_empty() {
        return MorselEligibility::DeferredShape;
    }
    if plan.having.is_some() {
        return MorselEligibility::DeferredShape;
    }
    if super::agg::select_requires_aggregation(plan) {
        return if plan.group_by.len() <= 1 {
            MorselEligibility::PrimitiveAgg
        } else {
            MorselEligibility::DeferredShape
        };
    }
    MorselEligibility::PrimitiveScan
}

#[cfg(test)]
mod w4_telemetry_tests {
    use super::*;

    #[test]
    fn record_does_not_panic_when_disabled() {
        record_morsel_eligibility(MorselEligibility::PrimitiveScan);
        record_morsel_eligibility(MorselEligibility::PrimitiveAgg);
        record_morsel_eligibility(MorselEligibility::DeferredShape);
        record_morsel_eligibility(MorselEligibility::NotEligible);
    }

    #[test]
    fn snapshot_returns_all_four_categories() {
        let snap = snapshot_morsel_eligibility();
        let kinds: Vec<MorselEligibility> = snap.iter().map(|(k, _)| *k).collect();
        assert!(kinds.contains(&MorselEligibility::PrimitiveScan));
        assert!(kinds.contains(&MorselEligibility::PrimitiveAgg));
        assert!(kinds.contains(&MorselEligibility::DeferredShape));
        assert!(kinds.contains(&MorselEligibility::NotEligible));
    }
}

#[derive(Debug)]
pub struct Morsel<'a> {
    pub columns: SmallVec<[ColumnBatch<'a>; 8]>,
    pub validity: Bitmap,
    pub len: u16,
}

impl<'a> Morsel<'a> {
    pub fn new() -> Self {
        Self {
            columns: SmallVec::new(),
            validity: Bitmap::new(0),
            len: 0,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len as usize
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
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
