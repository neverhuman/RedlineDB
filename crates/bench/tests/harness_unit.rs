//! Batch F2 (jankurai repair cycle 2): focused unit-style integration tests
//! for the bench harness's manager/dispatch/seeding surfaces.
//!
//! Audit context: the bench crate ships ~5K LOC behind a handful of
//! integration test files, and the coverage audit flagged the harness's
//! workload dispatch / repetition seeding / canonical-name surfaces as a
//! proof gap adjacent to HLT-022. These tests cover the **publicly
//! reachable** harness contracts that the report writers, CLI consumers,
//! and the certify scheduler all depend on:
//!
//! 1. Canonical engine/workload/durability tokens — the strings the CLI
//!    arg parser, the JSONL report, and the failpoint/recover child
//!    invocations all serialize must round-trip 1:1 with the enum
//!    variants. A drift here silently corrupts every downstream report.
//! 2. `RunArgs::into_run_spec` — the harness's manager-state shaping
//!    layer that clamps zero/one inputs to sane minimums and threads the
//!    repetition seed through unchanged.
//! 3. `CompareConfig::load` / `run_spec` — config validation rejects
//!    empty workload/durability/threads, and per-(engine, workload) run
//!    specs propagate the same caller seed so cross-engine repetitions
//!    are deterministic.
//! 4. `CertifyArgs::strace_enabled` — flag-OR-env contract that gates
//!    the strace capture path.
//! 5. `FailpointMatrixConfig::load` — rejects empty case lists so the
//!    matrix never silently passes with zero coverage.
//! 6. `ProcessMetrics::default` — the all-`None` fallback used on
//!    platforms without `/proc` or `getrusage` support.
//!
//! Note on scope: the private `engine` module's `engine_name` and
//! `seeded_blob` helpers are `pub(crate)`-only (they live inside the
//! private `mod engine` tree under `crates/bench/src/engine/{redline,
//! sqlite}.rs`), so this file exercises their *contract* at the public
//! `EngineKind` token surface rather than reaching across the visibility
//! boundary. The `kv-seed` payload determinism is already covered by
//! `crates/bench/src/checksum.rs::tests::*` and the engine-level seeding
//! is covered by `crates/bench/src/engine/{redline,sqlite}.rs` `#[cfg(
//! test)]` modules.

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use redlinedb_bench::config::{
    CertifyArgs, CompareConfig, DurabilityKind, EngineKind, FailpointMatrixConfig, RunArgs,
    WorkloadKind,
};
use redlinedb_bench::process_metrics::ProcessMetrics;
use tempfile::tempdir;

/// Canonical engine tokens. The failpoint-matrix child, the recover
/// child, and the JSONL report all serialize EngineKind via serde's
/// kebab-case rename; the in-tree `engine_name(EngineKind)` helpers in
/// `recover.rs` and `failpoint_matrix.rs` *must* agree with these tokens
/// for round-trip parsing to work. This test pins them at the public
/// surface so a rename anywhere is loud.
#[test]
fn engine_kind_canonical_tokens_round_trip() {
    let redline_json = serde_json::to_string(&EngineKind::Redline).expect("serialize redline");
    let sqlite_json = serde_json::to_string(&EngineKind::Sqlite).expect("serialize sqlite");
    assert_eq!(redline_json, "\"redline\"", "EngineKind::Redline token");
    assert_eq!(sqlite_json, "\"sqlite\"", "EngineKind::Sqlite token");

    let back_redline: EngineKind =
        serde_json::from_str("\"redline\"").expect("parse redline token");
    let back_sqlite: EngineKind = serde_json::from_str("\"sqlite\"").expect("parse sqlite token");
    assert_eq!(back_redline, EngineKind::Redline);
    assert_eq!(back_sqlite, EngineKind::Sqlite);
}

/// Every `WorkloadKind::as_str()` token must parse back through
/// `FromStr` to the same variant. The certify scheduler stringifies the
/// workload to pass it as `--workload <name>` to the child, and the
/// child then re-parses via `clap::ValueEnum`; this test guarantees the
/// in-process equivalent parser keeps the two paths in lockstep. Drift
/// here was the root cause of a Phase 10 silent-skip bug.
#[test]
fn workload_kind_as_str_round_trips_through_from_str() {
    use WorkloadKind::*;
    let all = [
        SingleRowInsert,
        BatchedInsert100,
        PointReadPk,
        SecondaryIndexRead,
        SecondaryIndexRange,
        WritersDisjoint,
        HotRowUpdate,
        MixedOltp,
        Mixed95Read5Write,
        Mixed80Read20Write,
        Mixed50Read50Write,
        ConnectionLimit,
        LargeSortSpill,
        JsonPathExtract,
        JsonPathUpdate,
        VectorFlatSearch,
        VectorAnnSearch,
        VectorAnnSearchDisk,
        CommitStormBatched,
        SecondaryIndexCount,
        SecondaryIndexOrderedLimit,
        CoveredRangeCold,
        CoveredRangeWarm,
        HotCounterUpdate,
        QueueMixed,
        ChaosLockConvoy,
        ChaosConnectionChurn,
        ChaosCheckpointThrash,
        ChaosIndexHammer,
        ChaosSortSpillConvoy,
        ChaosSchemaStorm,
    ];
    for workload in all {
        let token = workload.as_str();
        let parsed = WorkloadKind::from_str(token).unwrap_or_else(|err| {
            panic!("workload {workload:?} token {token:?} did not parse: {err}")
        });
        assert_eq!(
            parsed, workload,
            "WorkloadKind::as_str({workload:?}) -> {token:?} did not round-trip"
        );
    }
}

/// `DurabilityKind::as_str` must agree byte-for-byte with the serde
/// kebab-case representation that the report writers use. The
/// recover/failpoint children take the string form on the command line,
/// so any drift breaks the wire contract.
#[test]
fn durability_kind_canonical_tokens() {
    assert_eq!(DurabilityKind::Strict.as_str(), "strict");
    assert_eq!(DurabilityKind::Normal.as_str(), "normal");
    assert_eq!(DurabilityKind::Unsafe.as_str(), "unsafe");

    // And matching serde tokens (RunRecord serializes via serde).
    assert_eq!(
        serde_json::to_string(&DurabilityKind::Strict).expect("ser"),
        "\"strict\""
    );
    assert_eq!(
        serde_json::to_string(&DurabilityKind::Unsafe).expect("ser"),
        "\"unsafe\""
    );
}

/// `RunArgs::into_run_spec` is the manager-state shaping layer that
/// clamps zero/one inputs to sane minimums. The harness relies on these
/// clamps so a malformed CLI invocation never silently produces a
/// zero-thread, zero-row, zero-second workload (which would deadlock or
/// divide by zero downstream).
#[test]
fn run_args_into_run_spec_clamps_zero_inputs_to_minimum() {
    let args = RunArgs {
        engine: EngineKind::Redline,
        workload: WorkloadKind::PointReadPk,
        durability: DurabilityKind::Strict,
        threads: 0,
        rows: 0,
        seconds: 0,
        cache_mib: 0,
        seed: 42,
        out: None,
    };
    let spec = args.into_run_spec().expect("into_run_spec");
    assert!(spec.threads >= 1, "threads clamped to >= 1");
    assert!(spec.rows >= 1, "rows clamped to >= 1");
    assert!(
        spec.duration >= Duration::from_secs(1),
        "duration clamped to >= 1 second"
    );
    assert!(spec.cache_bytes >= 1024 * 1024, "cache_bytes >= 1 MiB");
    assert_eq!(spec.seed, 42, "seed propagated through unchanged");
    assert_eq!(spec.engine, EngineKind::Redline);
    assert_eq!(spec.workload, WorkloadKind::PointReadPk);
    assert_eq!(spec.durability, DurabilityKind::Strict);
}

/// Two `RunArgs` with identical seeds must produce `RunSpec`s with the
/// same seed regardless of engine/workload choice. This is the
/// "repetition seeding is independent across engines" contract — the
/// certify scheduler runs N repetitions per (engine, workload) cell and
/// each repetition shares the run-level seed so the workload's RNG is
/// deterministic.
#[test]
fn run_args_preserves_seed_across_engine_and_workload() {
    let mk = |engine, workload| RunArgs {
        engine,
        workload,
        durability: DurabilityKind::Strict,
        threads: 1,
        rows: 8,
        seconds: 1,
        cache_mib: 1,
        seed: 0xDEAD_BEEF,
        out: None,
    };
    let a = mk(EngineKind::Redline, WorkloadKind::PointReadPk)
        .into_run_spec()
        .expect("redline spec");
    let b = mk(EngineKind::Sqlite, WorkloadKind::PointReadPk)
        .into_run_spec()
        .expect("sqlite spec");
    let c = mk(EngineKind::Sqlite, WorkloadKind::SingleRowInsert)
        .into_run_spec()
        .expect("sqlite insert spec");
    assert_eq!(a.seed, 0xDEAD_BEEF);
    assert_eq!(b.seed, 0xDEAD_BEEF);
    assert_eq!(c.seed, 0xDEAD_BEEF);
    assert_eq!(
        a.seed, b.seed,
        "cross-engine workloads must share the caller seed"
    );
    assert_eq!(
        b.seed, c.seed,
        "cross-workload runs on the same engine must share the seed"
    );
}

/// `CompareConfig::load` validates that the user supplied non-empty
/// workloads/durabilities/threads lists. A silent pass on an empty list
/// would produce a "compare config produced no benchmark runs" failure
/// only after the harness booted, which obscures the real misconfig.
#[test]
fn compare_config_load_rejects_empty_workloads() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("empty-workloads.toml");
    std::fs::write(
        &path,
        r#"
out_dir = "/tmp/out"
workloads = []
durabilities = ["strict"]
threads = [1]
"#,
    )
    .expect("write toml");
    let err = CompareConfig::load(&path).expect_err("must reject empty workloads");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("non-empty workloads")
            || msg.contains("workloads")
            || msg.contains("durabilities")
            || msg.contains("threads"),
        "error message must mention the empty axis, got: {msg}"
    );
}

/// `CompareConfig::run_spec` is the inner loop of `run_compare`: it
/// builds a `RunSpec` for each (engine, workload, durability, threads)
/// cell using the caller's seed. This test verifies that the seed
/// reaches every produced spec unchanged — the cross-engine matrix
/// relies on this to guarantee reproducibility across rows of the
/// comparison report.
#[test]
fn compare_config_run_spec_propagates_seed() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("minimal-compare.toml");
    std::fs::write(
        &path,
        r#"
out_dir = "/tmp/redlinedb-bench-compare"
workloads = ["point-read-pk"]
durabilities = ["strict"]
threads = [1, 4]
rows = 32
seconds = 1
cache_mib = 1
"#,
    )
    .expect("write toml");
    let config = CompareConfig::load(&path).expect("load");
    let seed = 1_234_567_u64;
    for engine in &config.engines {
        for workload in &config.workloads {
            for durability in &config.durabilities {
                for &threads in &config.threads {
                    let spec = config
                        .run_spec(engine, workload, durability, threads, seed)
                        .expect("run_spec");
                    assert_eq!(spec.seed, seed, "seed must reach every cell");
                    assert!(spec.threads >= 1, "threads clamped");
                    assert_eq!(spec.engine, *engine);
                    assert_eq!(spec.workload, *workload);
                    assert_eq!(spec.durability, *durability);
                    assert_eq!(
                        spec.base_dir,
                        PathBuf::from("/tmp/redlinedb-bench-compare/dbs")
                    );
                }
            }
        }
    }
}

/// `CertifyArgs::strace_enabled` is the OR of `--with-strace` and
/// `REDLINEDB_BENCH_WITH_STRACE`. This guards the flag-only branch (no
/// env var) — the env-var branch is already covered by
/// `tests/certify_strace.rs::certify_with_strace_env_triggers`. We
/// construct `CertifyArgs` directly (rather than via clap) so this test
/// cannot interfere with the env-mutating tests in the sibling file.
#[test]
fn certify_args_strace_enabled_reflects_flag_when_env_absent() {
    let with_flag = CertifyArgs {
        config: PathBuf::from("/tmp/cfg.toml"),
        out_dir: PathBuf::from("/tmp/out"),
        seed: 7,
        repetitions: 1,
        warmup: 0,
        with_strace: true,
    };
    let without_flag = CertifyArgs {
        with_strace: false,
        ..with_flag.clone()
    };
    assert!(
        with_flag.with_strace,
        "field is preserved on the constructed args"
    );
    assert!(
        !without_flag.with_strace,
        "field is preserved on the constructed args"
    );
    // Note: strace_enabled() also reads REDLINEDB_BENCH_WITH_STRACE; if
    // some other test or the harness has set it, the env path may
    // shortcut this test to true. We assert the flag-only invariant: a
    // true flag MUST yield true. The false-flag branch is a soft
    // assertion that holds when the env var is absent or empty.
    assert!(
        with_flag.strace_enabled(),
        "--with-strace=true must enable regardless of env"
    );
    if std::env::var_os("REDLINEDB_BENCH_WITH_STRACE")
        .filter(|v| !v.is_empty())
        .is_none()
    {
        assert!(
            !without_flag.strace_enabled(),
            "with no flag and no env var, strace must be disabled"
        );
    }
}

/// `FailpointMatrixConfig::load` must reject configs with zero cases.
/// A silent pass on an empty matrix would make the failpoint gate
/// trivially green (no cases ran, so no cases failed), which is the
/// exact regression the audit gates against.
#[test]
fn failpoint_matrix_config_rejects_empty_cases() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("empty-failpoint.toml");
    std::fs::write(
        &path,
        r#"
durabilities = ["strict"]
cases = []
"#,
    )
    .expect("write toml");
    let err = FailpointMatrixConfig::load(&path).expect_err("must reject empty cases");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("at least one case") || msg.contains("cases"),
        "error must mention empty cases, got: {msg}"
    );
}

/// Benchmark integrity guard: the RedlineDB CLI must not contain a parity
/// replay layer. SQLite parity cases must be executed by the real CLI/SQL
/// path, not by code generated from expected corpus output.
#[test]
fn cli_source_has_no_sqlite_parity_replay_fastpath() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("repo root")
        .to_path_buf();
    let cli_src = repo.join("crates/cli/src");
    let forbidden_files = [
        repo.join("crates/cli/src/sqlite_parity_fast.rs"),
        repo.join("crates/cli/build.rs"),
    ];
    for path in forbidden_files {
        assert!(
            !path.exists(),
            "{} must not exist; parity replay tables are forbidden",
            path.display()
        );
    }

    let mut files = Vec::new();
    collect_rust_files(&cli_src, &mut files);
    for path in files {
        let text = fs::read_to_string(&path).expect("read cli source");
        for needle in [
            "sqlite_parity_fast",
            "GENERATED_STDIN_CASES",
            "GENERATED_ARG_CASES",
            "expected_stdout",
            "finish_fast_output",
            "surface_output",
            "without_engine_startup",
        ] {
            assert!(
                !text.contains(needle),
                "{} contains forbidden parity replay marker `{}`",
                path.display(),
                needle
            );
        }
    }
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read source directory") {
        let path = entry.expect("source entry").path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// `ProcessMetrics::default` is the fallback for platforms without
/// `/proc` (Linux) or `getrusage`-friendly counters (macOS). Every
/// field is `Option<u64>` and the default must yield `None` for every
/// one, so the harness JSON output omits the keys instead of emitting
/// bogus zeros that downstream dashboards would treat as real
/// measurements.
#[test]
fn process_metrics_default_is_all_none() {
    let m = ProcessMetrics::default();
    assert_eq!(m.rss_peak_bytes, None);
    assert_eq!(m.proc_io_read_bytes, None);
    assert_eq!(m.proc_io_write_bytes, None);
    assert_eq!(m.fsync_count, None);
    assert_eq!(m.fdatasync_count, None);
    assert_eq!(m.pwrite_count, None);
    assert_eq!(m.write_count, None);
    assert_eq!(m.voluntary_ctx_switches, None);
    assert_eq!(m.involuntary_ctx_switches, None);

    // Round-trip through serde to verify the JSON shape used by the
    // report writers is what we expect on unsupported hosts.
    let json = serde_json::to_value(&m).expect("serialize");
    let object = json.as_object().expect("object");
    for (key, value) in object {
        assert!(
            value.is_null(),
            "process metric {key} must serialize as null on default"
        );
    }
}
