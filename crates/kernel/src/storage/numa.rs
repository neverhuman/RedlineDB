//! NUMA-aware shard pinning. Phase 5 WS-B6. Feature-gated.
//!
//! With `--features numa` the buffer pool sizes its shard count to the
//! number of NUMA nodes the host exposes (clamped to >= 1) and exposes
//! a per-shard `pin_current_thread_to_node` helper that workers can
//! call to bind themselves to the node owning their shard, encouraging
//! first-touch allocation to land on the right socket.
//!
//! Linux topology comes from sysfs and thread affinity is applied with
//! Rustix. Nodes are ordered by numeric sysfs node ID and intersected with
//! the calling thread's current affinity mask. Other targets retain the
//! one-node/no-op fallback.
//!
//! Without the feature, both helpers are no-op stubs that return 1 /
//! `Ok(())`, so the default build is byte-identical to pre-B6.

#[cfg(all(feature = "numa", target_os = "linux"))]
use rustix::thread::{CpuSet, sched_getaffinity, sched_setaffinity};
#[cfg(all(feature = "numa", target_os = "linux"))]
use std::collections::BTreeSet;
#[cfg(all(feature = "numa", target_os = "linux"))]
use std::ffi::OsStr;
#[cfg(all(feature = "numa", target_os = "linux"))]
use std::path::Path;

/// Returns the number of NUMA nodes on the host, or 1 if unknown / feature off.
#[cfg(all(feature = "numa", target_os = "linux"))]
pub fn numa_node_count() -> usize {
    current_effective_nodes()
        .map(|nodes| nodes.len().max(1))
        .unwrap_or(1)
}

#[cfg(all(feature = "numa", not(target_os = "linux")))]
pub fn numa_node_count() -> usize {
    1
}

#[cfg(not(feature = "numa"))]
pub fn numa_node_count() -> usize {
    1
}

/// Pin the current OS thread to NUMA node `node_id`. No-op without feature.
#[cfg(all(feature = "numa", target_os = "linux"))]
pub fn pin_current_thread_to_node(node_id: usize) -> Result<(), String> {
    let nodes = current_effective_nodes()?;
    if let Some(node) = nodes.get(node_id) {
        let mut cpuset = CpuSet::new();
        for cpu in &node.cpus {
            cpuset.set(*cpu);
        }
        sched_setaffinity(None, &cpuset).map_err(|e| format!("bind: {e}"))?;
    }
    Ok(())
}

#[cfg(all(feature = "numa", not(target_os = "linux")))]
pub fn pin_current_thread_to_node(_node_id: usize) -> Result<(), String> {
    Ok(())
}

#[cfg(not(feature = "numa"))]
pub fn pin_current_thread_to_node(_node_id: usize) -> Result<(), String> {
    Ok(())
}

#[cfg(all(feature = "numa", target_os = "linux"))]
const SYSFS_NODE_ROOT: &str = "/sys/devices/system/node";

#[cfg(all(feature = "numa", target_os = "linux"))]
#[derive(Debug, Eq, PartialEq)]
struct EffectiveNode {
    id: usize,
    cpus: Vec<usize>,
}

#[cfg(all(feature = "numa", target_os = "linux"))]
fn current_effective_nodes() -> Result<Vec<EffectiveNode>, String> {
    let allowed = sched_getaffinity(None).map_err(|e| format!("affinity: {e}"))?;
    effective_nodes_from_sysfs(Path::new(SYSFS_NODE_ROOT), &allowed)
}

#[cfg(all(feature = "numa", target_os = "linux"))]
fn effective_nodes_from_sysfs(root: &Path, allowed: &CpuSet) -> Result<Vec<EffectiveNode>, String> {
    let entries = std::fs::read_dir(root).map_err(|e| format!("topology: {e}"))?;
    let mut nodes = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("topology: {e}"))?;
        let Some(id) = node_id(entry.file_name().as_os_str()) else {
            continue;
        };
        if !entry
            .file_type()
            .map_err(|e| format!("topology: {e}"))?
            .is_dir()
        {
            continue;
        }
        let cpulist_path = entry.path().join("cpulist");
        let cpulist = std::fs::read_to_string(&cpulist_path)
            .map_err(|e| format!("topology {}: {e}", cpulist_path.display()))?;
        let cpus = parse_cpu_list(&cpulist)
            .map_err(|e| format!("topology {}: {e}", cpulist_path.display()))?
            .into_iter()
            .filter(|cpu| allowed.is_set(*cpu))
            .collect::<Vec<_>>();
        if !cpus.is_empty() {
            nodes.push(EffectiveNode { id, cpus });
        }
    }
    nodes.sort_unstable_by_key(|node| node.id);
    Ok(nodes)
}

#[cfg(all(feature = "numa", target_os = "linux"))]
fn node_id(name: &OsStr) -> Option<usize> {
    let suffix = name.to_str()?.strip_prefix("node")?;
    if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let id = suffix.parse::<usize>().ok()?;
    (id.to_string() == suffix).then_some(id)
}

#[cfg(all(feature = "numa", target_os = "linux"))]
fn parse_cpu_list(value: &str) -> Result<Vec<usize>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("empty CPU list".to_string());
    }
    let mut cpus = BTreeSet::new();
    for item in value.split(',') {
        let item = item.trim();
        if item.is_empty() {
            return Err("empty CPU-list item".to_string());
        }
        let (start, end) = match item.split_once('-') {
            Some((start, end)) => {
                if end.contains('-') {
                    return Err(format!("invalid CPU range: {item}"));
                }
                (parse_cpu(start)?, parse_cpu(end)?)
            }
            None => {
                let cpu = parse_cpu(item)?;
                (cpu, cpu)
            }
        };
        if start > end {
            return Err(format!("descending CPU range: {item}"));
        }
        for cpu in start..=end {
            cpus.insert(cpu);
        }
    }
    Ok(cpus.into_iter().collect())
}

#[cfg(all(feature = "numa", target_os = "linux"))]
fn parse_cpu(value: &str) -> Result<usize, String> {
    let cpu = value
        .parse::<usize>()
        .map_err(|_| format!("invalid CPU ID: {value}"))?;
    if cpu >= CpuSet::MAX_CPU {
        return Err(format!("CPU ID exceeds affinity-mask capacity: {cpu}"));
    }
    Ok(cpu)
}

#[cfg(all(test, feature = "numa", target_os = "linux"))]
mod tests {
    use super::*;

    fn affinity(cpus: &[usize]) -> CpuSet {
        let mut affinity = CpuSet::new();
        for cpu in cpus {
            affinity.set(*cpu);
        }
        affinity
    }

    fn write_node(root: &Path, id: usize, cpulist: &str) {
        let node = root.join(format!("node{id}"));
        std::fs::create_dir(&node).unwrap();
        std::fs::write(node.join("cpulist"), cpulist).unwrap();
    }

    fn mask_cpus(mask: &CpuSet) -> Vec<usize> {
        (0..CpuSet::MAX_CPU)
            .filter(|cpu| mask.is_set(*cpu))
            .collect()
    }

    struct RestoreAffinity(CpuSet);

    impl Drop for RestoreAffinity {
        fn drop(&mut self) {
            let _ = sched_setaffinity(None, &self.0);
        }
    }

    #[test]
    fn parses_sparse_overlapping_cpu_lists_deterministically() {
        assert_eq!(
            parse_cpu_list("8,0-3,2-5,10,12-13\n").unwrap(),
            vec![0, 1, 2, 3, 4, 5, 8, 10, 12, 13]
        );
    }

    #[test]
    fn rejects_malformed_cpu_lists() {
        for malformed in ["", " ", "1,", ",1", "3-1", "1-2-3", "cpu0", "-1"] {
            assert!(parse_cpu_list(malformed).is_err(), "{malformed:?}");
        }
    }

    #[test]
    fn synthetic_one_and_two_node_topologies_are_numeric_and_restricted() {
        let fixture = tempfile::tempdir().unwrap();
        write_node(fixture.path(), 10, "0-3");
        let allowed = affinity(&[2, 8, 11]);
        assert_eq!(
            effective_nodes_from_sysfs(fixture.path(), &allowed).unwrap(),
            vec![EffectiveNode {
                id: 10,
                cpus: vec![2],
            }]
        );

        write_node(fixture.path(), 2, "8-12");
        assert_eq!(
            effective_nodes_from_sysfs(fixture.path(), &allowed).unwrap(),
            vec![
                EffectiveNode {
                    id: 2,
                    cpus: vec![8, 11],
                },
                EffectiveNode {
                    id: 10,
                    cpus: vec![2],
                },
            ]
        );
    }

    #[test]
    fn malformed_sysfs_and_empty_effective_topology_fall_back_safely() {
        let malformed = tempfile::tempdir().unwrap();
        write_node(malformed.path(), 0, "0-bad");
        assert!(effective_nodes_from_sysfs(malformed.path(), &affinity(&[0])).is_err());

        let restricted = tempfile::tempdir().unwrap();
        write_node(restricted.path(), 0, "0-3");
        let nodes = effective_nodes_from_sysfs(restricted.path(), &affinity(&[8])).unwrap();
        assert!(nodes.is_empty());
        assert_eq!(nodes.len().max(1), 1);
    }

    #[test]
    fn pins_current_thread_to_effective_node_and_restores_affinity() {
        let original = sched_getaffinity(None).unwrap();
        let _restore = RestoreAffinity(original);
        if let Err(error) = sched_setaffinity(None, &original) {
            eprintln!("skipping real affinity pin: sandbox denies sched_setaffinity: {error}");
            assert_eq!(sched_getaffinity(None).unwrap(), original);
            return;
        }
        let nodes = effective_nodes_from_sysfs(Path::new(SYSFS_NODE_ROOT), &original).unwrap();
        let Some(first) = nodes.first() else {
            assert_eq!(numa_node_count(), 1);
            return;
        };

        pin_current_thread_to_node(0).unwrap();
        let pinned = sched_getaffinity(None).unwrap();
        assert_eq!(mask_cpus(&pinned), first.cpus);

        sched_setaffinity(None, &original).unwrap();
        pin_current_thread_to_node(usize::MAX).unwrap();
        assert_eq!(sched_getaffinity(None).unwrap(), original);
    }
}
