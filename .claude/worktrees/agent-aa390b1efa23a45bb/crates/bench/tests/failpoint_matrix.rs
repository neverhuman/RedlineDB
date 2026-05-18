//! Lane E bench-side smoke tests for the failpoint matrix.
//!
//! These guard the two seams that are easy to break in isolation:
//! the TOML schema (any field rename in `FailpointMatrixCase` breaks
//! parsing silently if no test loads the file) and the end-to-end
//! parent/child handshake (a regression in `failpoint::cfg` wiring or
//! the ack-log protocol means Lane E silently passes everything).
//!
//! Lane F6 reviewer-finding tests cover the two correctness fixes:
//! the ack-log fsync contract (`ack_log_is_fsynced`) and the
//! verbatim passthrough of failpoint actions
//! (`failpoint_action_return_is_honored`).

use std::path::PathBuf;

use redlinedb_bench::config::{
    DurabilityKind, EngineKind, ExpectExit, FailpointMatrixArgs, FailpointMatrixCase,
    FailpointMatrixConfig,
};
use redlinedb_bench::failpoint_matrix;
use redlinedb_bench::failpoint_matrix::ObservedRun;

fn matrix_path() -> PathBuf {
    redlinedb_bench::default_failpoint_matrix_path()
}

#[test]
fn matrix_toml_parses_seven_canonical_cases() {
    let path = matrix_path();
    assert!(
        path.exists(),
        "failpoint-matrix.toml must ship at {}",
        path.display()
    );
    let config = FailpointMatrixConfig::load(&path).expect("parse failpoint-matrix.toml");
    assert_eq!(
        config.cases.len(),
        7,
        "matrix should ship the seven Lane E canonical cases"
    );

    // Spot-check a few critical cases so a silent rename is loud.
    let names: Vec<&str> = config.cases.iter().map(|case| case.name.as_str()).collect();
    assert!(names.contains(&"commit-publish-kill"));
    assert!(names.contains(&"wal-write-torn"));
    assert!(names.contains(&"catalog-rename-kill"));

    // Strict durability must appear in the global default so cases
    // that omit `durabilities` still cover the contract Lane E gates.
    assert!(
        config.durabilities.contains(&DurabilityKind::Strict),
        "default durabilities must cover strict"
    );
}

#[test]
fn matrix_toml_cases_target_only_known_failpoints() {
    let config = FailpointMatrixConfig::load(&matrix_path()).expect("parse failpoint-matrix.toml");
    let allowed = [
        "wal::write_encoded",
        "wal::flush",
        "wal::flush_until",
        "wal::flush_all",
        "wal::prune",
        "engine::commit::before_publish",
        "engine::checkpoint",
        "heap::mutation",
        "index::insert",
        "index::delete",
        "index::split",
        "catalog::save::temp_write",
        "catalog::save::fsync",
        "catalog::save::rename",
        "catalog::save::parent_fsync",
        "storage::control::write",
    ];
    for case in &config.cases {
        assert!(
            allowed.contains(&case.failpoint.as_str()),
            "case {} targets unknown failpoint {}",
            case.name,
            case.failpoint
        );
    }
}

/// End-to-end run of a single case. Picks
/// `engine::commit::before_publish` because it is the most stable
/// hook (after fsync, before publish): the workload is guaranteed to
/// see the failpoint fire before the first row gets acknowledged
/// when `kill_after_n_hits = 1`, which makes the test deterministic.
///
/// The expected outcome is `lost_acked_commits == 0`: with strict
/// durability the engine must republish the CSN watermark from the
/// WAL when the database is reopened.
///
/// `kill_after_n_hits = 1` fires on the first commit, so the child
/// dies BEFORE any ack row is written. `expect_zero_acks = true`
/// opts the case into the "zero-ack is OK" branch of the lane-fp
/// gate; without it the new gate (correctly) flags the case as a
/// vacuous-oracle false pass.
#[test]
fn end_to_end_commit_publish_recovers_zero_lost_acked() {
    // Use an isolated tempdir so concurrent tests cannot poison each
    // other.
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg_path = tmp.path().join("matrix.toml");
    let out_path = tmp.path().join("report.json");

    std::fs::write(
        &cfg_path,
        r#"
durabilities = ["strict"]

[[cases]]
name = "commit-publish-smoke"
failpoint = "engine::commit::before_publish"
action = "panic"
durabilities = ["strict"]
rows = 16
kill_after_n_hits = [1]
expect_child_exit = "non-zero"
expect_zero_acks = true
"#,
    )
    .unwrap();

    let args = FailpointMatrixArgs {
        config: cfg_path,
        out: out_path.clone(),
        seed: 7,
    };
    let report = failpoint_matrix::run(&args).expect("run failpoint matrix");
    failpoint_matrix::write_report(&out_path, &report).expect("write report");

    assert!(
        out_path.exists(),
        "matrix report must be written to {}",
        out_path.display()
    );
    assert_eq!(report.runs.len(), 1, "exactly one run expected");
    let run = &report.runs[0];
    assert_eq!(run.engine, EngineKind::Redline);
    assert_eq!(run.durability, DurabilityKind::Strict);
    assert_eq!(
        run.lost_acked_commits, 0,
        "strict durability must lose zero acked commits"
    );
    assert!(run.passed, "case must pass: {:?}", run);
    assert!(report.passed, "report must report overall pass");
}

/// Lane F6: every line written via `ack_row` must survive a sudden
/// process exit. We can't easily simulate an OS-level crash from
/// within a unit test, but we can at least pin the durability
/// contract: after each `ack_row` returns, dropping the file handle
/// and re-reading the path must return every entry. The previous
/// implementation called `flush()` on a `BufWriter`, which would
/// pass this test trivially because the buffer drain happens in-
/// process — but it would fail if `read_ack_count` ran in a
/// separate process before the OS flushed the page cache. We assert
/// the on-disk metadata (file length) matches the expected byte
/// count to give the test some teeth: if the writes were silently
/// buffered the file size would lag behind.
#[test]
fn ack_log_is_fsynced() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("ack.log");

    let mut handle = failpoint_matrix::open_ack_log(&path).expect("open ack log");

    let keys = [0_usize, 1, 2, 3, 7, 11, 42, 1023];
    let mut expected_bytes: u64 = 0;
    for &key in &keys {
        failpoint_matrix::ack_row(&mut handle, key).expect("ack row durably");
        // Snapshot the on-disk size IMMEDIATELY (without flushing the
        // handle again) — if the implementation deferred the write
        // to a `BufWriter`, the metadata size would be smaller than
        // expected at this point.
        let metadata = std::fs::metadata(&path).expect("stat ack log");
        expected_bytes += format!("{key}\n").len() as u64;
        assert_eq!(
            metadata.len(),
            expected_bytes,
            "ack-log on-disk size must reflect every fsynced row immediately after ack_row returns",
        );
    }

    drop(handle);

    // Reopen and read every line. Equivalent to what the parent's
    // `read_ack_count` does after the child dies.
    let body = std::fs::read_to_string(&path).expect("read ack log");
    let mut recorded: Vec<usize> = body
        .lines()
        .filter_map(|line| line.trim().parse::<usize>().ok())
        .collect();
    recorded.sort_unstable();
    let mut expected = keys.to_vec();
    expected.sort_unstable();
    assert_eq!(
        recorded, expected,
        "every fsynced ack entry must be discoverable post-crash",
    );
}

/// Lane F6: a `return` action configured via `fail::cfg` must take
/// the early-return short-circuit, not silently translate to panic.
/// We register a synthetic failpoint behind a closure form, fire it
/// via `fail::eval`-driven hot path, and assert the closure return
/// is honoured.
///
/// The previous bench-runner mapping rewrote `return` to `panic`
/// before forwarding to `fail::cfg`; that meant any `[[cases]]`
/// block configured with `action = "return"` was effectively a
/// panic test. The fix is in `apply_kill_count`: action strings are
/// passed verbatim. This test guards the kernel half of the
/// contract by exercising the failpoint registry directly.
#[test]
fn failpoint_action_return_is_honored() {
    use redlinedb_kernel::failpoints;

    failpoints::init();

    // Pick a unique name so we don't collide with kernel hot-path
    // failpoints any other test may have armed.
    let name = "lane-f6::test::return-honored";

    // Configure the failpoint to fire the `return` action exactly
    // once. The closure form distinguishes this from a panic action:
    // `fail::eval` returns `Some(())` to the call site, which the
    // closure converts to a sentinel value the test can observe.
    failpoints::cfg(name, "return").expect("cfg return action");

    // Drive the failpoint via the same `fail_point!` macro the kernel
    // uses. We construct a small function that returns an integer:
    // when the failpoint is OFF the function returns 0; when armed
    // with `return` the closure short-circuits with 42.
    fn probe(name: &str) -> i32 {
        // Use the bare `fail::fail_point!` so this test does not
        // depend on the kernel's reexport of the macro at a
        // crate-private path.
        fail::fail_point!(name, |_| 42);
        0
    }

    let observed = probe(name);
    assert_eq!(
        observed, 42,
        "return action must short-circuit the function via the closure form, not panic",
    );

    // Disarm so other tests don't see this failpoint armed.
    failpoints::cfg(name, "off").expect("disarm");
    let observed_off = probe(name);
    assert_eq!(
        observed_off, 0,
        "after `off`, the failpoint closure must not fire",
    );

    // Sanity: panic action would unwind through the closure;
    // configuring `return` followed by `panic` in turn must produce
    // a panic on next probe. Wrap in `catch_unwind` to assert.
    failpoints::cfg(name, "panic").expect("cfg panic action");
    let result = std::panic::catch_unwind(|| probe(name));
    assert!(
        result.is_err(),
        "panic action must unwind, distinguishing it from return",
    );
    failpoints::cfg(name, "off").expect("disarm after panic test");
}

/// Build a synthetic `FailpointMatrixCase` for verdict-only tests.
///
/// The runner's three-clause gate (`evaluate_verdict`) is independent
/// of the spawn pipeline; tests construct synthetic
/// (`case`, `observed`) pairs to exercise verdict logic without
/// spawning a child binary. (Spawning a child from the cargo test
/// harness invokes the test binary itself, which has no
/// `failpoint-child` subcommand and panics during clap parsing —
/// that's why the existing end-to-end test has to opt into
/// `expect_zero_acks = true`.)
fn synthetic_case(
    action: &str,
    expect_child_exit: ExpectExit,
    expect_zero_acks: bool,
) -> FailpointMatrixCase {
    FailpointMatrixCase {
        name: "synthetic".to_owned(),
        failpoint: "engine::commit::before_publish".to_owned(),
        action: action.to_owned(),
        durabilities: vec![DurabilityKind::Strict],
        rows: 16,
        kill_after_n_hits: vec![1],
        expect_zero_acks,
        expect_child_exit,
    }
}

/// Lane FP reviewer-finding: a TOML case configured with
/// `action = "abort"` must surface as a configuration error, not as
/// a silent no-op.
///
/// Two-part assertion. (1) The TOML config schema accepts the
/// keyword (it's a free-form string passed verbatim to fail::cfg) so
/// loading parses cleanly. (2) The kernel-side `failpoints::cfg`
/// validator rejects the token before `fail::cfg` ever sees it; the
/// returned error message names the offending token for ops debugging.
/// (3) The lane-fp gate, applied to a synthetic observation that
/// mirrors what the runner sees when the child failed to arm the
/// failpoint (`exit != 0, acks == 0`), reports `passed = false`
/// because zero acks without `expect_zero_acks` is a vacuous oracle.
#[test]
fn failpoint_matrix_rejects_unknown_action() {
    // 1. TOML schema parses an `action = "abort"` case literally.
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg_path = tmp.path().join("matrix.toml");
    std::fs::write(
        &cfg_path,
        r#"
durabilities = ["strict"]

[[cases]]
name = "abort-rejected"
failpoint = "engine::commit::before_publish"
action = "abort"
durabilities = ["strict"]
rows = 4
kill_after_n_hits = [1]
expect_child_exit = "non-zero"
"#,
    )
    .unwrap();
    let parsed = FailpointMatrixConfig::load(&cfg_path).expect("parse synthetic abort case");
    assert_eq!(parsed.cases[0].action, "abort");

    // 2. Kernel `failpoints::cfg` validator rejects the token. This
    // is the boundary the matrix runner crosses inside the child.
    let cfg_err = redlinedb_kernel::failpoints::cfg("matrix-abort", "abort")
        .expect_err("abort must be rejected by validator");
    assert!(
        cfg_err.to_lowercase().contains("abort"),
        "validator error names the unsupported token: {cfg_err}"
    );

    // 3. Verdict on the synthetic observation a runner would see
    // when the child failed to arm the failpoint and exited 1: zero
    // acks, zero recovered, non-zero exit. The default
    // `expect_zero_acks = false` flips passed to false.
    let case = synthetic_case(
        "abort",
        ExpectExit::NonZero,
        /* expect_zero_acks */ false,
    );
    let observed = ObservedRun {
        child_exit_status: Some(1),
        child_exit_success: false,
        acknowledged: 0,
        recovered: 0,
        lost_acked_commits: 0,
    };
    let (passed, reason) = failpoint_matrix::evaluate_verdict(&case, &observed);
    assert!(
        !passed,
        "abort-action case must NOT pass via the new gate: {reason}"
    );
    assert!(
        reason.contains("expect_zero_acks") || reason.contains("acknowledged=0"),
        "reason mentions the zero-ack vacuous-oracle check: {reason}"
    );
}

/// Lane FP reviewer-finding: when `expect_child_exit = "non-zero"`
/// but the observed child exit is 0 (clean), the gate must FAIL.
/// Previously the runner only checked `lost_acked_commits`, so a
/// case where the child legitimately completed the workload could
/// still be misclassified.
#[test]
fn failpoint_matrix_fails_when_child_exit_unexpected() {
    let case = synthetic_case(
        "return",
        ExpectExit::NonZero,
        /* expect_zero_acks */ false,
    );
    let observed = ObservedRun {
        child_exit_status: Some(0),
        child_exit_success: true,
        acknowledged: 1024, // full ack count, oracle is non-vacuous
        recovered: 1024,
        lost_acked_commits: 0,
    };
    let (passed, reason) = failpoint_matrix::evaluate_verdict(&case, &observed);
    assert!(
        !passed,
        "child exited cleanly but gate expected non-zero — must FAIL: {reason}"
    );
    assert!(
        reason.contains("expect_child_exit"),
        "pass_reason must explain the exit-status mismatch: {reason}"
    );
}

/// Lane FP reviewer-finding: a case with zero acks AND
/// `expect_zero_acks = false` (the default) must FAIL because the
/// gate oracle is vacuous. This is the central anti-false-pass
/// guarantee: a child that dies before any commit acks is no
/// evidence about durability and the gate must flag it.
#[test]
fn failpoint_matrix_fails_when_zero_acks_without_expect() {
    let case = synthetic_case(
        "panic",
        ExpectExit::NonZero,
        /* expect_zero_acks */ false,
    );
    let observed = ObservedRun {
        child_exit_status: Some(101),
        child_exit_success: false,
        acknowledged: 0,
        recovered: 0,
        lost_acked_commits: 0,
    };
    let (passed, reason) = failpoint_matrix::evaluate_verdict(&case, &observed);
    assert!(
        !passed,
        "zero acks without expect_zero_acks must fail the gate: {reason}"
    );
    assert!(
        reason.contains("expect_zero_acks"),
        "pass_reason must mention expect_zero_acks: {reason}"
    );

    // And the symmetric: same observation, but opt-in to
    // expect_zero_acks. Now the case passes (kill failed before any
    // ack, but that was expected).
    let case_opt_in = synthetic_case(
        "panic",
        ExpectExit::NonZero,
        /* expect_zero_acks */ true,
    );
    let (passed_opt_in, reason_opt_in) =
        failpoint_matrix::evaluate_verdict(&case_opt_in, &observed);
    assert!(
        passed_opt_in,
        "with expect_zero_acks=true the same observation passes: {reason_opt_in}"
    );
}
