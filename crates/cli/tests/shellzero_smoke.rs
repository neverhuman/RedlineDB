//! WS-C8 ShellZero smoke tests.
//!
//! The pre-open fast path is OFF by default and must produce identical
//! behavior to the regular CLI when omitted. With `--shellzero`, a small
//! audited surface (constant-expression `SELECT`s, a few dot-commands) is
//! handled without opening a `Database`.

use std::process::Command;

use assert_cmd::cargo::cargo_bin;

fn run(args: &[&str], stdin: Option<&str>) -> (String, String, i32) {
    let bin = cargo_bin("redlinedb-cli");
    let mut cmd = Command::new(bin);
    cmd.args(args);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("spawn redlinedb cli");
    if let Some(input) = stdin {
        use std::io::Write;
        let stdin = child.stdin.as_mut().expect("stdin");
        stdin.write_all(input.as_bytes()).expect("write stdin");
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait redlinedb cli");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

#[test]
fn shellzero_help_exits_zero() {
    let (out, err, code) = run(&["--shellzero", "-batch", ":memory:"], Some(".help\n"));
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains(".help"), "expected help text in stdout: {out}");
}

#[test]
fn shellzero_select_one_plus_one() {
    let (out, err, code) = run(&["--shellzero", "-batch", ":memory:", "SELECT 1+1;"], None);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.trim_end(), "2", "stdout={out:?}");
}

#[test]
fn shellzero_falls_through_on_from_clause() {
    // SELECT * FROM t (no such table) - pre-open evaluator can't handle
    // it, so we fall through to the normal path which errors out.
    let (_out, err, code) = run(
        &["--shellzero", "-batch", ":memory:", "SELECT * FROM t;"],
        None,
    );
    assert_ne!(code, 0, "expected nonzero exit; stderr={err}");
    assert!(
        err.to_lowercase().contains("error") || err.to_lowercase().contains("no such"),
        "expected error message; stderr={err}"
    );
}

#[test]
fn without_shellzero_select_still_works() {
    // Identical behavior with the flag absent: the regular CLI path
    // opens the database and answers the same scalar.
    let (out, err, code) = run(&["-batch", ":memory:", "SELECT 1+1;"], None);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.trim_end(), "2", "stdout={out:?}");
}

#[test]
fn shellzero_version_emits_version_line() {
    let (out, err, code) = run(&["--shellzero", "-batch", ":memory:"], Some(".version\n"));
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("redlinedb v"), "stdout={out}");
}

#[test]
fn shellzero_print_arg() {
    let (out, err, code) = run(
        &["--shellzero", "-batch", ":memory:"],
        Some(".print hello world\n"),
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("hello world"), "stdout={out:?}");
}

#[test]
fn shellzero_string_concat() {
    let (out, err, code) = run(
        &["--shellzero", "-batch", ":memory:", "SELECT 'a' || 'b';"],
        None,
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.trim_end(), "ab", "stdout={out:?}");
}
