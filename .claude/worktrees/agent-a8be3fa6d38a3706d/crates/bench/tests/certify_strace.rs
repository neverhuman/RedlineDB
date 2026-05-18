//! Wave 6 Lane B tests for the certify --with-strace plumbing.
//!
//! These guard three concrete contracts:
//!   1. `--with-strace` parses through clap with the documented default.
//!   2. The `REDLINEDB_BENCH_WITH_STRACE` env var enables the same path
//!      so we never regress the legacy override.
//!   3. (Linux only) wrapping a no-op child in `strace -c` produces a
//!      non-empty syscall summary file we can parse.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use clap::Parser;
use redlinedb_bench::Cli;
use redlinedb_bench::config::{CertifyArgs, Command};

/// Shared mutex so env-var-touching tests don't race when run in
/// parallel. `cargo test` runs tests in the same process by default,
/// and the env var is process-wide.
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn parse_certify(extra: &[&str]) -> CertifyArgs {
    let mut argv = vec![
        "redlinedb-bench",
        "certify",
        "--config",
        "/tmp/cfg.toml",
        "--out-dir",
        "/tmp/out",
    ];
    argv.extend_from_slice(extra);
    let cli = Cli::try_parse_from(argv).expect("parse");
    match cli.command {
        Command::Certify(args) => args,
        other => panic!("expected Certify, got {other:?}"),
    }
}

#[test]
fn certify_with_strace_flag_parses() {
    let _guard = env_lock().lock().unwrap();
    // Wipe the env var so it can't poison the default-false path.
    let prev = std::env::var_os("REDLINEDB_BENCH_WITH_STRACE");
    // SAFETY: the env_lock() above serializes mutations in this process.
    unsafe { std::env::remove_var("REDLINEDB_BENCH_WITH_STRACE") };

    let default = parse_certify(&[]);
    assert!(
        !default.with_strace,
        "--with-strace must default to false when neither flag nor env is set"
    );
    assert!(
        !default.strace_enabled(),
        "strace_enabled() must mirror the resolved decision"
    );
    assert_eq!(default.config, PathBuf::from("/tmp/cfg.toml"));

    let with = parse_certify(&["--with-strace"]);
    assert!(with.with_strace, "--with-strace flag must set the field");
    assert!(with.strace_enabled());

    if let Some(prev) = prev {
        // SAFETY: see above.
        unsafe { std::env::set_var("REDLINEDB_BENCH_WITH_STRACE", prev) };
    }
}

#[test]
fn certify_with_strace_env_triggers() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("REDLINEDB_BENCH_WITH_STRACE");

    // SAFETY: env_lock() serializes; `unsafe` here matches the std API
    // contract introduced in 2024-edition std::env helpers.
    unsafe { std::env::set_var("REDLINEDB_BENCH_WITH_STRACE", "1") };
    let args = parse_certify(&[]);
    assert!(
        !args.with_strace,
        "the env var must NOT mutate the parsed flag"
    );
    assert!(
        args.strace_enabled(),
        "env var alone must enable strace via strace_enabled()"
    );

    // Empty value is treated as disabled.
    unsafe { std::env::set_var("REDLINEDB_BENCH_WITH_STRACE", "") };
    let empty = parse_certify(&[]);
    assert!(!empty.strace_enabled(), "empty env var must not enable");

    // Restore for sibling tests.
    unsafe { std::env::remove_var("REDLINEDB_BENCH_WITH_STRACE") };
    if let Some(prev) = prev {
        unsafe { std::env::set_var("REDLINEDB_BENCH_WITH_STRACE", prev) };
    }
}

/// On Linux, prove that wrapping a tiny child (`/bin/true`) in
/// `strace -c -o <path>` actually writes a parseable summary file.
/// The test soft-skips when strace is missing because some Linux
/// containers strip it out — we want CI to stay green there but still
/// catch real regressions on the xbabe1 host.
#[cfg(target_os = "linux")]
#[test]
fn strace_capture_hooks_child() {
    use std::process::Command;

    let which = Command::new("which").arg("strace").output();
    let strace_available =
        matches!(&which, Ok(out) if out.status.success() && !out.stdout.is_empty());
    if !strace_available {
        eprintln!("skipping: strace not on PATH");
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let out_path = tmp.path().join("strace.txt");
    let status = Command::new("strace")
        .arg("-c")
        .arg("-o")
        .arg(&out_path)
        .arg("--")
        .arg("/bin/true")
        .status()
        .expect("spawn strace");
    assert!(status.success(), "strace exit was {status:?}");
    let raw = std::fs::read_to_string(&out_path).expect("strace output exists");
    assert!(!raw.trim().is_empty(), "strace output must not be empty");

    let parsed = redlinedb_bench::strace_capture::parse_strace_summary(&raw);
    assert!(
        !parsed.is_empty(),
        "expected at least one syscall in summary, got: {raw}"
    );
}
