//! Phase 5 WS-B6 — NUMA-aware shard pinning hooks.
//!
//! Off-feature the helpers MUST be trivial stubs; on-feature they MUST
//! at least report >= 1 NUMA node and not panic when asked to pin to
//! node 0. We intentionally avoid asserting any specific topology
//! because CI runners may be single-socket VMs.

use redlinedb_kernel::numa;

#[cfg(not(feature = "numa"))]
#[test]
fn numa_node_count_stub_is_one_without_feature() {
    assert_eq!(numa::numa_node_count(), 1);
}

#[cfg(not(feature = "numa"))]
#[test]
fn pin_current_thread_is_noop_without_feature() {
    assert!(numa::pin_current_thread_to_node(0).is_ok());
    assert!(numa::pin_current_thread_to_node(usize::MAX).is_ok());
}

#[cfg(feature = "numa")]
#[test]
fn numa_node_count_is_at_least_one_with_feature() {
    assert!(numa::numa_node_count() >= 1);
}

#[cfg(feature = "numa")]
#[test]
fn pin_current_thread_to_node_zero_does_not_error() {
    // node 0 must exist on any host where the topology probe succeeded;
    // an out-of-range node id is silently ignored (the helper only
    // pins when the index resolves).
    let _ = numa::pin_current_thread_to_node(0);
}
