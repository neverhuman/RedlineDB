//! NUMA-aware shard pinning. Phase 5 WS-B6. Feature-gated.
//!
//! With `--features numa` the buffer pool sizes its shard count to the
//! number of NUMA nodes the host exposes (clamped to >= 1) and exposes
//! a per-shard `pin_current_thread_to_node` helper that workers can
//! call to bind themselves to the node owning their shard, encouraging
//! first-touch allocation to land on the right socket.
//!
//! Without the feature, both helpers are no-op stubs that return 1 /
//! `Ok(())`, so the default build is byte-identical to pre-B6.

#[cfg(feature = "numa")]
pub use hwlocality::Topology;

/// Returns the number of NUMA nodes on the host, or 1 if unknown / feature off.
#[cfg(feature = "numa")]
pub fn numa_node_count() -> usize {
    use hwlocality::object::types::ObjectType;
    Topology::new()
        .map(|t| t.objects_with_type(ObjectType::NUMANode).count().max(1))
        .unwrap_or(1)
}

#[cfg(not(feature = "numa"))]
pub fn numa_node_count() -> usize {
    1
}

/// Pin the current OS thread to NUMA node `node_id`. No-op without feature.
#[cfg(feature = "numa")]
pub fn pin_current_thread_to_node(node_id: usize) -> Result<(), String> {
    use hwlocality::cpu::binding::CpuBindingFlags;
    use hwlocality::object::types::ObjectType;
    let topo = Topology::new().map_err(|e| format!("topology: {e}"))?;
    let nodes: Vec<_> = topo.objects_with_type(ObjectType::NUMANode).collect();
    if let Some(node) = nodes.get(node_id) {
        let cpuset = node
            .cpuset()
            .ok_or_else(|| "node has no cpuset".to_string())?;
        topo.bind_cpu(cpuset, CpuBindingFlags::THREAD)
            .map_err(|e| format!("bind: {e}"))?;
    }
    Ok(())
}

#[cfg(not(feature = "numa"))]
pub fn pin_current_thread_to_node(_node_id: usize) -> Result<(), String> {
    Ok(())
}
