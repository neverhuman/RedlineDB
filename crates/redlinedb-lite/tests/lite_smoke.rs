//! Phase 6 Candidate 2 smoke tests for redlinedb-lite.
//!
//! Exercises the supported surface end-to-end via subprocess. The
//! fall-through case requires the full `redlinedb` binary to be built
//! and discoverable; we point at it via the `REDLINEDB_LITE_FULL_BIN`
//! env override (set by tests) so we don't depend on PATH layout.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use assert_cmd::cargo::cargo_bin;

fn lite_bin() -> PathBuf {
    cargo_bin("redlinedb-lite")
}

fn full_bin() -> PathBuf {
    // The full CLI ships the `redlinedb` binary. We don't depend on it
    // in Cargo.toml (would re-link the engine) so we just resolve the
    // path the same way assert_cmd does.
    cargo_bin("redlinedb")
}

fn run(args: &[&str], stdin: Option<&str>) -> (String, String, i32) {
    let mut cmd = Command::new(lite_bin());
    cmd.args(args);
    cmd.env("REDLINEDB_LITE_FULL_BIN", full_bin());
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn redlinedb-lite");
    if let Some(input) = stdin {
        use std::io::Write;
        let h = child.stdin.as_mut().expect("stdin");
        h.write_all(input.as_bytes()).expect("write stdin");
    }
    drop(child.stdin.take());
    let out = child.wait_with_output().expect("wait redlinedb-lite");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn lite_version_flag() {
    let (out, err, code) = run(&["-version"], None);
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("redlinedb v"), "stdout={out:?}");
}

#[test]
fn lite_help_flag() {
    let (out, err, code) = run(&["-help"], None);
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.to_lowercase().contains("usage"), "stdout={out:?}");
}

#[test]
fn lite_dot_help_via_stdin() {
    let (out, err, code) = run(&[":memory:"], Some(".help\n"));
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains(".help"), "stdout={out:?}");
}

#[test]
fn lite_dot_version_via_stdin() {
    let (out, err, code) = run(&[":memory:"], Some(".version\n"));
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("redlinedb v"), "stdout={out:?}");
}

#[test]
fn lite_dot_print() {
    let (out, err, code) = run(&[":memory:"], Some(".print hello world\n"));
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("hello world"), "stdout={out:?}");
}

#[test]
fn lite_select_one_plus_one() {
    let (out, err, code) = run(&[":memory:", "SELECT 1+1;"], None);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.trim_end(), "2", "stdout={out:?}");
}

#[test]
fn lite_select_concat() {
    let (out, err, code) = run(&[":memory:", "SELECT 'a' || 'b';"], None);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.trim_end(), "ab", "stdout={out:?}");
}

#[test]
fn lite_falls_through_to_full_for_select_from() {
    // SELECT * FROM t — engine territory. Lite must exec the full
    // binary, which will error because no such table exists.
    if !full_bin().exists() {
        eprintln!(
            "skipping: full redlinedb binary not built at {:?}",
            full_bin()
        );
        return;
    }
    let (_out, err, code) = run(&[":memory:", "SELECT * FROM t;"], None);
    assert_ne!(code, 0, "expected nonzero; stderr={err}");
    let lower = err.to_lowercase();
    assert!(
        lower.contains("error") || lower.contains("no such"),
        "expected engine error; stderr={err}"
    );
}
