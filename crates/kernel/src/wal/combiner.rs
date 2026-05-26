//! Lane GC (phase 10): opt-in semantic WAL combiner.
//!
//! # Design intent
//!
//! Hot-counter workloads (think: leaderboards, ad-impression
//! counters, rate-limited token buckets) issue thousands of
//! `UPDATE counters SET v = v + ? WHERE id = ?` per second. Each
//! such mutation today produces a distinct WAL record; the WAL
//! writer's group-commit batches many of them into a single fsync,
//! but the on-disk write amplification is still proportional to the
//! number of transactions.
//!
//! The combiner is a *write-amplification* optimisation: when the
//! coordinator can prove (via the [`CombinableDelta`] shape) that two
//! still-in-buffer mutations target the same `(rel_id, row_id,
//! column)` and are commutative (additive) deltas, it folds them
//! into a single physical record before fsync. **Each individual
//! transaction's logical effect is still WAL-logged separately** —
//! the combiner operates on the per-tuple delta record only, never
//! on `Begin`/`Commit`/`Abort` boundaries — so recovery semantics
//! are unchanged.
//!
//! # Safety stance
//!
//! Per the phase-10 work plan: if the combiner is not safe-by-
//! construction, it ships disabled. This module therefore exposes a
//! deliberately small pure-data API (`CombinableDelta`, `WalCombiner`)
//! that the writer thread can probe without entering an unsafe state
//! machine. The actual fold step inside the coordinator's pending
//! queue is gated behind `WalConfig::semantic_combiner = true`.
//! Default behaviour is byte-for-byte identical to the pre-combiner
//! kernel because the flag remains off unless a caller opts in.
//!
//! # Why not in `manager.rs`?
//!
//! The combiner is independently testable, has no I/O, and will
//! grow once safe-by-construction proofs land. Keeping it in its
//! own file keeps `manager.rs` under the 2000-LOC active-source
//! ceiling enforced by `scripts/check_file_sizes.sh`.

/// A WAL-record-shape descriptor for an additive delta on a single
/// numeric column. Produced by the SQL layer (one day) at the
/// moment a `value = value + N` mutation is encoded.
///
/// The combiner treats two `CombinableDelta`s as mergeable iff their
/// `(rel_id, row_id, column)` triple matches and the saturating
/// addition of their `delta` fields does not overflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CombinableDelta {
    pub rel_id: u64,
    pub row_id: u64,
    pub column: u32,
    pub delta: i64,
}

/// Stateless probe-and-fold helper. Holding no state (and no
/// allocations) keeps the combiner cheap to invoke from the WAL
/// writer's hot path; future iterations may add a small per-target
/// hash-map to lift the O(N^2) scan to O(N).
#[derive(Clone, Copy, Debug, Default)]
pub struct WalCombiner;

impl WalCombiner {
    /// Lane GC: construct a fresh combiner. Stateless today, but
    /// the constructor is kept for forward compatibility with the
    /// hashed-target version.
    pub fn new() -> Self {
        Self
    }

    /// Returns `true` iff `a` and `b` operate on the same target.
    /// Distinct from [`Self::try_merge`] so callers can probe before
    /// allocating a merged record on the heap.
    pub fn targets_match(&self, a: &CombinableDelta, b: &CombinableDelta) -> bool {
        a.rel_id == b.rel_id && a.row_id == b.row_id && a.column == b.column
    }

    /// Attempt to fold two deltas into one. Returns `None` if the
    /// targets don't match or if `a.delta + b.delta` would overflow.
    /// Bit-exact: this never silently saturates; callers must skip
    /// the combine and let both records flow normally.
    pub fn try_merge(&self, a: &CombinableDelta, b: &CombinableDelta) -> Option<CombinableDelta> {
        if !self.targets_match(a, b) {
            return None;
        }
        let delta = a.delta.checked_add(b.delta)?;
        Some(CombinableDelta {
            rel_id: a.rel_id,
            row_id: a.row_id,
            column: a.column,
            delta,
        })
    }
}

/// Lane GC: invoked by the coordinator when
/// `WalConfig::semantic_combiner` is true and a candidate record is
/// about to enter the pending queue. This pure helper never mutates
/// the queue directly: it returns either the merged delta that the
/// coordinator should replace the pending tail with, or `Enqueue`
/// when the candidate must flow normally. Callers must check the
/// config flag *before* invoking.
///
/// Documented contract for the future safe version:
/// 1. Only operates on records where both sides have opted-in via
///    `Txn::set_combinable(true)`.
/// 2. Folds happen exclusively in-buffer: once a record's bytes
///    leave the pending queue (i.e. `write_encoded` was called) it
///    is no longer eligible.
/// 3. The folded record carries a *combined* tx_id list so commit
///    visibility honours every constituent transaction.
pub fn maybe_combine_pending(
    combiner: &WalCombiner,
    candidate: &CombinableDelta,
    pending_back: Option<&CombinableDelta>,
) -> CombineOutcome {
    match pending_back.and_then(|pending| combiner.try_merge(pending, candidate)) {
        Some(merged) => CombineOutcome::Folded(merged),
        None => CombineOutcome::Enqueue,
    }
}

/// Lane GC: outcome of [`maybe_combine_pending`]. Reserved API for
/// the eventual implementation; the variants document the expected
/// state transitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombineOutcome {
    /// Candidate folded into the pending tail; pending queue
    /// should replace that tail with the returned merged delta.
    Folded(CombinableDelta),
    /// No fold — candidate must be enqueued normally.
    Enqueue,
}

/// WS-C6: cross-lane flush combiner helper. Consumes a stream of
/// `(lane_idx, target_lsn)` flush requests and emits the per-lane
/// union — the maximum LSN any waiter on that lane requested. Used
/// by the cross-lane coalescer worker to collapse a burst of N
/// concurrent flush requests into at most `lane_count` per-lane
/// fsync barriers. Free function so the hot path performs zero
/// allocations beyond the caller's scratch slice. Returns the count
/// of distinct lanes touched.
pub fn cross_lane_union<I>(requests: I, per_lane_target: &mut [u64]) -> usize
where
    I: IntoIterator<Item = (usize, u64)>,
{
    for slot in per_lane_target.iter_mut() {
        *slot = 0;
    }
    let mut touched = 0_usize;
    for (lane_idx, target) in requests {
        if let Some(slot) = per_lane_target.get_mut(lane_idx) {
            if *slot == 0 {
                touched += 1;
            }
            if target > *slot {
                *slot = target;
            }
        }
    }
    touched
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_matching_targets() {
        let combiner = WalCombiner::new();
        let merged = combiner
            .try_merge(
                &CombinableDelta {
                    rel_id: 1,
                    row_id: 2,
                    column: 3,
                    delta: 5,
                },
                &CombinableDelta {
                    rel_id: 1,
                    row_id: 2,
                    column: 3,
                    delta: 7,
                },
            )
            .expect("matching targets must merge");
        assert_eq!(merged.delta, 12);
    }

    #[test]
    fn refuses_overflow() {
        let combiner = WalCombiner::new();
        assert!(
            combiner
                .try_merge(
                    &CombinableDelta {
                        rel_id: 1,
                        row_id: 2,
                        column: 3,
                        delta: i64::MAX,
                    },
                    &CombinableDelta {
                        rel_id: 1,
                        row_id: 2,
                        column: 3,
                        delta: 1,
                    },
                )
                .is_none()
        );
    }

    #[test]
    fn cross_lane_union_picks_max_per_lane() {
        let mut scratch = [0_u64; 4];
        let touched = cross_lane_union(
            [(0, 100), (1, 50), (0, 200), (3, 10), (1, 30), (9, 7)],
            &mut scratch,
        );
        assert_eq!(touched, 3);
        assert_eq!(scratch, [200, 50, 0, 10]);
    }

    #[test]
    fn cross_lane_union_empty_input() {
        let mut scratch = [0_u64; 2];
        let touched = cross_lane_union(std::iter::empty(), &mut scratch);
        assert_eq!(touched, 0);
        assert_eq!(scratch, [0, 0]);
    }

    #[test]
    fn refuses_distinct_rows() {
        let combiner = WalCombiner::new();
        assert!(
            combiner
                .try_merge(
                    &CombinableDelta {
                        rel_id: 1,
                        row_id: 2,
                        column: 3,
                        delta: 1,
                    },
                    &CombinableDelta {
                        rel_id: 1,
                        row_id: 99,
                        column: 3,
                        delta: 1,
                    },
                )
                .is_none()
        );
    }
}
