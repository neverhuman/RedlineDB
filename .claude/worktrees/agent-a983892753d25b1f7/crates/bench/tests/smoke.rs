use std::time::Duration;

use redlinedb_bench::Cli;
use redlinedb_bench::config::{CompareConfig, EngineKind, WorkloadKind};

fn run_spec(engine: redlinedb_bench::config::EngineKind) -> redlinedb_bench::config::RunSpec {
    redlinedb_bench::config::RunSpec {
        engine,
        workload: redlinedb_bench::config::WorkloadKind::PointReadPk,
        durability: redlinedb_bench::config::DurabilityKind::Strict,
        threads: 1,
        rows: 32,
        duration: Duration::from_millis(100),
        cache_bytes: 4 * 1024 * 1024,
        seed: 7,
        base_dir: std::env::temp_dir().join("redlinedb-bench-tests"),
    }
}

#[test]
fn redline_run_produces_operations() {
    let record = redlinedb_bench::workload::run_once(&run_spec(
        redlinedb_bench::config::EngineKind::Redline,
    ))
    .expect("record");
    assert!(record.metrics.operations > 0);
}

#[test]
fn sqlite_run_produces_operations() {
    let record =
        redlinedb_bench::workload::run_once(&run_spec(redlinedb_bench::config::EngineKind::Sqlite))
            .expect("record");
    assert!(record.metrics.operations > 0);
}

#[test]
fn smoke_compare_config_exists() {
    assert!(redlinedb_bench::default_smoke_config_path().exists());
    assert!(redlinedb_bench::default_certification_config_path().exists());
    assert!(redlinedb_bench::default_recovery_matrix_path().exists());
    let _ = std::mem::size_of::<Cli>();
}

#[test]
fn phase10_v3_configs_register_feature_workloads() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("bench/certification-phase10-v3-smoke.toml");
    let config = CompareConfig::load(&path).expect("load phase10 v3 smoke config");
    for workload in [
        WorkloadKind::JsonPathExtract,
        WorkloadKind::JsonPathUpdate,
        WorkloadKind::VectorFlatSearch,
        WorkloadKind::VectorAnnSearch,
        WorkloadKind::VectorAnnSearchDisk,
        WorkloadKind::CommitStormBatched,
        WorkloadKind::LargeSortSpill,
    ] {
        assert!(
            config.workloads.contains(&workload),
            "{workload:?} missing from phase10 v3 smoke config"
        );
    }
}

#[test]
fn phase10_v3_sqlite_compare_config_filters_ann_workloads() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("bench/certification-phase10-v3-compare.toml");
    let config = CompareConfig::load(&path).expect("load phase10 v3 compare config");
    assert_eq!(
        config.engines,
        vec![EngineKind::Redline, EngineKind::Sqlite]
    );
    for workload in [
        WorkloadKind::JsonPathExtract,
        WorkloadKind::JsonPathUpdate,
        WorkloadKind::VectorFlatSearch,
        WorkloadKind::CommitStormBatched,
        WorkloadKind::LargeSortSpill,
    ] {
        assert!(
            config.workloads.contains(&workload),
            "{workload:?} missing from phase10 v3 compare config"
        );
    }
    for workload in [
        WorkloadKind::VectorAnnSearch,
        WorkloadKind::VectorAnnSearchDisk,
    ] {
        assert!(
            !config.workloads.contains(&workload),
            "{workload:?} should be excluded from the SQLite comparison config"
        );
    }
}
