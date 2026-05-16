//! Tests for the chaos suite.
//!
//! Split into two groups:
//!
//! 1. Stats-schema unit tests that drive `chaos_stats` + `record_chaos_counters`
//!    directly and pin the JSON shape per workload. These were inherited from
//!    main and ensure the counter naming convention is stable.
//!
//! 2. End-to-end `*_stats_keys_unchanged` tests that build a real engine,
//!    invoke each `run_*` function, and assert that the resulting `RunRecord`
//!    publishes the expected `chaos_*` keys plus a non-empty content hash.
//!    These pin the contract between the per-workload modules and the
//!    helpers; if a future refactor drops a counter the test fails.

use std::sync::atomic::AtomicU64;
use std::time::Duration;

use super::helpers::{chaos_stats, record_chaos_counters};
use super::{
    run_checkpoint_thrash, run_connection_churn, run_index_hammer, run_lock_convoy,
    run_schema_storm, run_sort_spill_convoy,
};
use crate::config::{DurabilityKind, EngineKind, RunSpec, WorkloadKind};
use crate::engine::{self, BenchEngine};
use crate::report::RunRecord;

/// Exercises the counter shape used by `run_connection_churn`. The
/// connection-churn workload reports `connection_opens / reads / writes /
/// commits / rollbacks`; after `record_chaos_counters` runs we must see
/// every counter materialised under the `chaos_<name>` key with the
/// snapshot value, and no extra keys polluting the map.
#[test]
fn record_chaos_counters_writes_connection_churn_shape() {
    let mut stats = chaos_stats("bounded", "connection-churn", 16, 10);
    let connection_opens = AtomicU64::new(11);
    let reads = AtomicU64::new(22);
    let writes = AtomicU64::new(33);
    let commits = AtomicU64::new(44);
    let rollbacks = AtomicU64::new(55);

    record_chaos_counters(
        &mut stats,
        &[
            ("connection_opens", &connection_opens),
            ("reads", &reads),
            ("writes", &writes),
            ("commits", &commits),
            ("rollbacks", &rollbacks),
        ],
    );

    assert_eq!(
        stats.get("chaos_connection_opens"),
        Some(&serde_json::json!(11))
    );
    assert_eq!(stats.get("chaos_reads"), Some(&serde_json::json!(22)));
    assert_eq!(stats.get("chaos_writes"), Some(&serde_json::json!(33)));
    assert_eq!(stats.get("chaos_commits"), Some(&serde_json::json!(44)));
    assert_eq!(stats.get("chaos_rollbacks"), Some(&serde_json::json!(55)));
    // Workload identifier survives the counter pass.
    assert_eq!(
        stats.get("chaos_workload"),
        Some(&serde_json::json!("connection-churn"))
    );
    // No accidental key drift (chaos_* counters added + 6 keys from chaos_stats).
    assert!(stats.contains_key("chaos_suite"));
}

/// Exercises the counter shape used by `run_index_hammer`, which adds a
/// `deletes` counter on top of the connection-churn shape. Asserts the
/// snapshot lands under `chaos_deletes` and the rest of the keys come
/// across unchanged so the certification consumer sees the same JSON
/// schema regardless of how many counters the caller passes in.
#[test]
fn record_chaos_counters_writes_index_hammer_shape() {
    let mut stats = chaos_stats("bounded", "index-hammer", 1024, 20);
    let reads = AtomicU64::new(101);
    let writes = AtomicU64::new(202);
    let deletes = AtomicU64::new(303);
    let commits = AtomicU64::new(404);
    let rollbacks = AtomicU64::new(505);

    record_chaos_counters(
        &mut stats,
        &[
            ("reads", &reads),
            ("writes", &writes),
            ("deletes", &deletes),
            ("commits", &commits),
            ("rollbacks", &rollbacks),
        ],
    );

    assert_eq!(stats.get("chaos_reads"), Some(&serde_json::json!(101)));
    assert_eq!(stats.get("chaos_writes"), Some(&serde_json::json!(202)));
    assert_eq!(stats.get("chaos_deletes"), Some(&serde_json::json!(303)));
    assert_eq!(stats.get("chaos_commits"), Some(&serde_json::json!(404)));
    assert_eq!(stats.get("chaos_rollbacks"), Some(&serde_json::json!(505)));
    assert_eq!(
        stats.get("chaos_workload"),
        Some(&serde_json::json!("index-hammer"))
    );
    // The 5-counter shape preserves the seed-row count from chaos_stats.
    assert_eq!(
        stats.get("chaos_seed_rows"),
        Some(&serde_json::json!(1024_usize))
    );
}

fn spec_for(workload: WorkloadKind, label: &str) -> (RunSpec, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("chaos-helper-{label}-{}", std::process::id()));
    let spec = RunSpec {
        engine: EngineKind::Redline,
        workload,
        durability: DurabilityKind::Strict,
        threads: 2,
        rows: 32,
        duration: Duration::from_millis(50),
        cache_bytes: 1024 * 1024,
        seed: 11,
        base_dir: dir.clone(),
    };
    (spec, dir)
}

fn build_engine(spec: &RunSpec, db_dir: &std::path::Path) -> Box<dyn BenchEngine> {
    let _ = std::fs::remove_dir_all(db_dir);
    let engine = engine::open(spec, db_dir).expect("engine open");
    engine.setup_schema().expect("schema setup");
    engine
}

fn assert_stats_keys(record: &RunRecord, workload: &str, required: &[&str]) {
    let stats = record
        .engine_stats
        .as_object()
        .expect("engine_stats object");
    assert_eq!(
        stats.get("chaos_workload").and_then(|v| v.as_str()),
        Some(workload),
        "chaos_workload tag must match"
    );
    assert_eq!(
        stats.get("chaos_suite").and_then(|v| v.as_str()),
        Some("dick-head-choas"),
        "chaos_suite tag must be present"
    );
    assert_eq!(
        stats.get("test_code_path").and_then(|v| v.as_str()),
        Some("crates/bench/src/chaos/mod.rs"),
        "test_code_path tag must point at chaos module"
    );
    for key in required {
        let value = stats
            .get(*key)
            .unwrap_or_else(|| panic!("workload {workload} must publish stats key {key}"));
        assert!(
            !value.is_null(),
            "workload {workload} key {key} must not be null"
        );
    }
    assert!(
        !record.checksum.content_hash.is_empty(),
        "workload {workload} must publish a non-empty content_hash"
    );
}

#[test]
fn lock_convoy_stats_keys_unchanged() {
    let (spec, dir) = spec_for(WorkloadKind::ChaosLockConvoy, "lock-convoy");
    let bench_engine = build_engine(&spec, &dir);
    let record = run_lock_convoy(bench_engine.as_ref(), &spec).expect("lock_convoy runs");
    assert_stats_keys(
        &record,
        "lock-convoy",
        &["chaos_commits", "chaos_rollbacks", "chaos_busy_timeout_ms"],
    );
}

#[test]
fn connection_churn_stats_keys_unchanged() {
    let (spec, dir) = spec_for(WorkloadKind::ChaosConnectionChurn, "connection-churn");
    let bench_engine = build_engine(&spec, &dir);
    let record =
        run_connection_churn(bench_engine.as_ref(), &spec).expect("connection_churn runs");
    assert_stats_keys(
        &record,
        "connection-churn",
        &[
            "chaos_connection_opens",
            "chaos_reads",
            "chaos_writes",
            "chaos_commits",
            "chaos_rollbacks",
        ],
    );
}

#[test]
fn checkpoint_thrash_stats_keys_unchanged() {
    let (spec, dir) = spec_for(WorkloadKind::ChaosCheckpointThrash, "checkpoint-thrash");
    let bench_engine = build_engine(&spec, &dir);
    let record =
        run_checkpoint_thrash(bench_engine.as_ref(), &spec).expect("checkpoint_thrash runs");
    assert_stats_keys(
        &record,
        "checkpoint-thrash",
        &["chaos_checkpoint_calls", "chaos_reads", "chaos_writes"],
    );
}

#[test]
fn index_hammer_stats_keys_unchanged() {
    let (spec, dir) = spec_for(WorkloadKind::ChaosIndexHammer, "index-hammer");
    let bench_engine = build_engine(&spec, &dir);
    let record = run_index_hammer(bench_engine.as_ref(), &spec).expect("index_hammer runs");
    assert_stats_keys(
        &record,
        "index-hammer",
        &[
            "chaos_reads",
            "chaos_writes",
            "chaos_deletes",
            "chaos_commits",
            "chaos_rollbacks",
        ],
    );
}

#[test]
fn sort_spill_convoy_stats_keys_unchanged() {
    let (spec, dir) = spec_for(WorkloadKind::ChaosSortSpillConvoy, "sort-spill-convoy");
    let bench_engine = build_engine(&spec, &dir);
    let record =
        run_sort_spill_convoy(bench_engine.as_ref(), &spec).expect("sort_spill_convoy runs");
    assert_stats_keys(
        &record,
        "sort-spill-convoy",
        &["chaos_spill_queries", "chaos_writes"],
    );
}

#[test]
fn schema_storm_stats_keys_unchanged() {
    let (spec, dir) = spec_for(WorkloadKind::ChaosSchemaStorm, "schema-storm");
    let bench_engine = build_engine(&spec, &dir);
    let record = run_schema_storm(bench_engine.as_ref(), &spec).expect("schema_storm runs");
    assert_stats_keys(&record, "schema-storm", &["chaos_ddl_ops"]);
}
