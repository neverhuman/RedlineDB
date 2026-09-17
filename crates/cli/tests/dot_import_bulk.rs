//! WS-C5 bulk: verify `PRAGMA redline_bulk_import = ON` correctly
//! loads a CSV via the batched multi-row VALUES fast path.

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
fn bulk_import_pragma_loads_5000_rows() {
    // 5000 rows exercises the BULK_IMPORT_CHUNK=1000 path (5 full chunks)
    // and the trailing-chunk path (last batch fits exactly).
    let dir = tempdir().expect("tempdir");
    let csv_path = dir.path().join("bulk.csv");
    let mut csv = String::with_capacity(5000 * 16);
    for i in 0..5000_i64 {
        csv.push_str(&format!("{i},row-{i}\n"));
    }
    std::fs::write(&csv_path, &csv).expect("write csv");

    let script = format!(
        "CREATE TABLE t(a INT, b TEXT);\nPRAGMA redline_bulk_import = ON;\n.import {} t\nSELECT count(*) FROM t;\n",
        csv_path.display()
    );
    let (out, err, code) = run_script(&script);
    assert_eq!(code, 0, "stderr={err}");
    assert!(
        out.lines().any(|l| l.trim() == "5000"),
        "expected line `5000` in stdout, got: {out}"
    );
}

#[test]
fn bulk_import_pragma_handles_partial_trailing_chunk() {
    // 5123 rows: 5 full chunks + 1 partial trailing chunk of 123.
    let dir = tempdir().expect("tempdir");
    let csv_path = dir.path().join("bulk_partial.csv");
    let mut csv = String::with_capacity(5123 * 16);
    for i in 0..5123_i64 {
        csv.push_str(&format!("{i},row-{i}\n"));
    }
    std::fs::write(&csv_path, &csv).expect("write csv");

    let script = format!(
        "CREATE TABLE t(a INT, b TEXT);\nPRAGMA redline_bulk_import = ON;\n.import {} t\nSELECT count(*) FROM t;\n",
        csv_path.display()
    );
    let (out, err, code) = run_script(&script);
    assert_eq!(code, 0, "stderr={err}");
    assert!(
        out.lines().any(|l| l.trim() == "5123"),
        "expected line `5123` in stdout, got: {out}"
    );
}
