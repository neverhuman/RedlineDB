//! Smoke test for `.import` after the WS-C5 prepare-hoist + transaction wrap.

use std::process::Command;

use assert_cmd::cargo::cargo_bin;
use tempfile::tempdir;

fn run_script(script: &str) -> (String, String, i32) {
    let bin = cargo_bin("redlinedb-cli");
    let mut cmd = Command::new(bin);
    cmd.arg("-batch").arg(":memory:");
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("spawn redlinedb cli");
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().expect("stdin");
        stdin.write_all(script.as_bytes()).expect("write stdin");
    }
    let output = child.wait_with_output().expect("wait redlinedb cli");
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

#[test]
fn dot_import_loads_csv_rows_inside_transaction() {
    let dir = tempdir().expect("tempdir");
    let csv_path = dir.path().join("rows.csv");
    std::fs::write(&csv_path, "1,alpha\n2,bravo\n3,charlie\n4,delta\n5,echo\n").expect("write csv");

    let script = format!(
        "CREATE TABLE t(a INT, b TEXT);\n.import {} t\nSELECT count(*) FROM t;\n",
        csv_path.display()
    );
    let (out, err, code) = run_script(&script);
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains('5'), "stdout={out}");
}
