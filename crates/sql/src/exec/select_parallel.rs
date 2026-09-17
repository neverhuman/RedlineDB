use crate::statement::SelectPlan;

// ============================================================
// WS-C3 R2: parallel covering-scan gate
// ============================================================

/// Outcome of the WS-C3 R2 parallel covering-scan gate. The variants
/// document *why* the gate chose one path over the other; the
/// `Dispatch` variant is the only one that would actually fan work
/// out across the rayon pool. Today no production covering pipeline
/// reaches `Dispatch` because the path walks index leaves rather
/// than heap pages — see the module note at the gate site for
/// the rationale (kept here so tests can assert the predicate's
/// per-condition behaviour without rebuilding the gate).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParallelCoveringDecision {
    /// All gate conditions hold AND a downstream consumer
    /// (HashAggregator / SpillSort) was detected — the executor
    /// would dispatch the parallel scan if the underlying engine
    /// were appropriate.
    Dispatch { worker_count: usize },
    /// `LIMIT` clause present — covering scans with a hard cap stay
    /// serial so the early-stop semantics survive.
    FallbackLimitPresent,
    /// `OUTER_ROW_STACK` is non-empty — a correlated outer row is
    /// in scope and a worker thread would lose access to it
    /// (the stack is thread-local).
    FallbackOuterRowStack,
    /// No rayon pool installed on the per-thread slot — operators
    /// must take the serial path until WS-C7 installs one.
    FallbackNoPool,
    /// Downstream consumer is neither HashAggregator nor SpillSort
    /// — the parallel result-ordering relaxation does not apply.
    FallbackDownstreamNotAggregator,
}

impl ParallelCoveringDecision {
    pub fn would_dispatch(self) -> bool {
        matches!(self, Self::Dispatch { .. })
    }
}

/// WS-C3 R2 gate predicate. Returns the decision the gate would
/// make for `plan` under the current thread-local context (rayon
/// pool slot + correlated-row stack). The actual heap-side
/// dispatch lives in `PageBackedHeap::parallel_scan_page_range`;
/// today the covering path serves results directly from the
/// index leaf chain, so even a `Dispatch` decision is honoured
/// by walking the existing serial cursor — the wiring is in
/// place for the future, the perf delta is what R1-D shipped.
pub(crate) fn decide_parallel_covering_scan(
    plan: &SelectPlan,
    limit: usize,
) -> ParallelCoveringDecision {
    if plan.limit.is_some() || limit != usize::MAX {
        return ParallelCoveringDecision::FallbackLimitPresent;
    }
    if !super::outer_row_stack_is_empty() {
        return ParallelCoveringDecision::FallbackOuterRowStack;
    }
    let pool = match super::current_rayon_pool() {
        Some(pool) => pool,
        None => return ParallelCoveringDecision::FallbackNoPool,
    };
    if !plan_downstream_is_aggregator_or_spill_sort(plan) {
        return ParallelCoveringDecision::FallbackDownstreamNotAggregator;
    }
    ParallelCoveringDecision::Dispatch {
        worker_count: pool.current_num_threads().max(1),
    }
}

/// Returns `true` when `plan`'s downstream operator (after the
/// covering scan emits rows) is a `HashAggregator` or `SpillSort`
/// — both tolerate unordered input which is what the parallel
/// scan produces. Today this is approximated by the presence of
/// `GROUP BY` / aggregate projections (→ HashAggregator) or a
/// non-empty `ORDER BY` (→ SpillSort).
fn plan_downstream_is_aggregator_or_spill_sort(plan: &SelectPlan) -> bool {
    if !plan.group_by.is_empty() {
        return true;
    }
    if super::agg::select_requires_aggregation(plan) {
        return true;
    }
    if !plan.order_by.is_empty() {
        return true;
    }
    false
}

thread_local! {
    /// Last `ParallelCoveringDecision` emitted on this thread, set by
    /// [`record_parallel_covering_decision`]. The SQL gate's tests read
    /// it to verify per-condition branching without intercepting the
    /// kernel scan call. Cleared lazily — readers should call
    /// [`take_last_parallel_covering_decision`] so a follow-up SELECT
    /// does not observe stale state.
    static LAST_PARALLEL_DECISION: std::cell::Cell<Option<ParallelCoveringDecision>> =
        const { std::cell::Cell::new(None) };
}

pub(crate) fn record_parallel_covering_decision(decision: ParallelCoveringDecision) {
    LAST_PARALLEL_DECISION.with(|cell| cell.set(Some(decision)));
}

/// WS-C3 R2 test hook: read and clear the most recent decision the
/// parallel covering-scan gate made on this thread. Returns `None`
/// when no covering-eligible SELECT has run since the last read.
pub fn take_last_parallel_covering_decision() -> Option<ParallelCoveringDecision> {
    LAST_PARALLEL_DECISION.with(|cell| cell.take())
}
