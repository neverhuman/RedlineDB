//! W4-A1: morsel-routed PrimitiveScan entry point (commit-1 plumbing).
//!
//! This module is the bridge between `select_top.rs::build_select_runtime`
//! and the morsel scaffolding in `super`. Commit-1 ships the skeleton only:
//! [`route_primitive_scan`] reads the env-var route mode, runs the W4-T
//! eligibility classifier, and records a telemetry counter — then *always*
//! returns a `Decline` so the caller stays on the tuple path. Subsequent
//! commits (W4-A2 classifier wiring, W4-A3 filter + emit) replace the
//! `Decline` with a real morsel-routed result when the shape is supported.
//!
//! Why ship the skeleton separately:
//! - Lets the integration point in `select_top.rs` land with zero
//!   behavioural change so we can A/B against the same binary.
//! - Splits the eligibility/predicate/filter work into reviewable chunks.
//! - Gives the W4-T telemetry a place to record routing decisions even
//!   before the routing itself fires.
//!
//! Off by default. Opt in via `REDLINE_MORSEL_ROUTE=primitive_scan`
//! (also `all`, `1`, `on`) — see [`super::morsel_route_mode`].

use std::sync::atomic::Ordering;

use super::{
    MORSEL_ROUTE_FALLBACK_DISABLED, MORSEL_ROUTE_FALLBACK_INELIGIBLE,
    MORSEL_ROUTE_FALLBACK_SHAPE, MorselEligibility, MorselRouteMode,
    classify_select_plan_eligibility, morsel_route_mode, morsel_telemetry_enabled,
};

/// Telemetry tap for routing decisions. No-op when telemetry is disabled.
#[inline]
fn record_decline(counter: &std::sync::atomic::AtomicU64) {
    if morsel_telemetry_enabled() {
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

/// W4-A1 entry point. Returns `RouteDecision::Decline(_)` whenever the
/// morsel path can't (or won't) handle this plan; the caller continues on
/// the tuple path.
///
/// Commit-1 contract: this function ALWAYS returns `Decline`. Subsequent
/// commits replace the final `Decline` with a `Routed` variant when
/// classification + predicate translation succeed.
pub fn route_primitive_scan(
    plan: &crate::statement::SelectPlan,
) -> Result<RouteDecision, crate::error::Error> {
    // 1. Route mode gate. When the env var isn't set we bail immediately so
    //    the only cost in default builds is one OnceLock read + branch.
    let mode = match morsel_route_mode() {
        Some(mode) => mode,
        None => {
            record_decline(&MORSEL_ROUTE_FALLBACK_DISABLED);
            return Ok(RouteDecision::Decline(DeclineReason::Disabled));
        }
    };

    // 2. Eligibility gate. The W4-T classifier already knows how to spot
    //    PrimitiveScan-shaped plans. Anything else stays on tuple path.
    let eligibility = classify_select_plan_eligibility(plan);
    if !matches!(eligibility, MorselEligibility::PrimitiveScan) {
        record_decline(&MORSEL_ROUTE_FALLBACK_INELIGIBLE);
        return Ok(RouteDecision::Decline(DeclineReason::Ineligible));
    }

    // 3. Shape-level rejections that the classifier doesn't catch. ORDER BY
    //    and OFFSET are deferred until W4-A2 so morsel routing can't
    //    interfere with ordering semantics. (LIMIT + no ORDER BY is OK
    //    because StaticRows respects the limit at the runtime layer.)
    if !plan.order_by.is_empty() || plan.offset.is_some() {
        record_decline(&MORSEL_ROUTE_FALLBACK_SHAPE);
        return Ok(RouteDecision::Decline(DeclineReason::Shape));
    }

    // 4. Commit-1 stops here. Subsequent commits add: column-kind derivation,
    //    WHERE predicate translation, rowid harvest, morsel-batch loop, and
    //    final tuple emission. For now we report that we WOULD have routed,
    //    then decline so the tuple path produces the answer.
    let _ = mode; // silence unused warning until commit-2 wires the mode.
    record_decline(&MORSEL_ROUTE_FALLBACK_SHAPE);
    Ok(RouteDecision::Decline(DeclineReason::NotYetImplemented))
}

/// Outcome of [`route_primitive_scan`]. `Routed` is reserved for the future
/// commits that ship actual morsel-routed execution.
#[derive(Debug)]
pub enum RouteDecision {
    /// Caller continues on the tuple path. Reason is recorded for telemetry.
    Decline(DeclineReason),
    /// Routed via morsel; the caller should use the returned rows as if
    /// they came from the existing `StaticRows` fast path. Not produced
    /// by commit-1; reserved for W4-A2+.
    #[allow(dead_code)]
    Routed(Vec<Vec<crate::value::SqlValue>>),
}

/// Why a routing attempt declined to morsel-route. Mapped 1:1 with the
/// `MORSEL_ROUTE_FALLBACK_*` counters in `super` (counter names kept for
/// continuity with the documented telemetry surface).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclineReason {
    /// `REDLINE_MORSEL_ROUTE` env var not set (the default).
    Disabled,
    /// W4-T classifier says the plan isn't PrimitiveScan-shaped.
    Ineligible,
    /// ORDER BY / OFFSET / LIMIT shape deferred from W4-A1.
    Shape,
    /// Projection contains non-bare or non-primitive columns.
    #[allow(dead_code)] // wired in W4-A2.
    Projection,
    /// WHERE predicate uses an op/value combination the filter kernels
    /// don't support yet.
    #[allow(dead_code)] // wired in W4-A2.
    Predicate,
    /// Mid-scan: the actual row had a value of a kind incompatible with
    /// the column's affinity-derived kind (e.g. INTEGER affinity column
    /// holds a TEXT value).
    #[allow(dead_code)] // wired in W4-A3.
    DynamicKind,
    /// Skeleton-stage stub: future commits will implement the routing.
    NotYetImplemented,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decline_reason_is_pure_data() {
        // Cheap smoke: the enum carries no payload beyond the discriminant.
        let r = DeclineReason::Disabled;
        assert_eq!(r, DeclineReason::Disabled);
        assert_ne!(r, DeclineReason::Ineligible);
    }
}
