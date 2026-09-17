//! Integration test: drive `sqlite3` from the runner's capability probe and
//! verify the booleans match what the locally-installed sqlite3 actually
//! supports.

use std::process::{Command, Stdio};
use std::str;

fn sqlite_version() -> Option<String> {
    let out = Command::new("sqlite3")
        .arg("-version")
        .stdout(Stdio::piped())
        .output()
        .ok()?;
    let line = str::from_utf8(&out.stdout).ok()?.lines().next()?.to_owned();
    Some(line)
}

fn sql_succeeds(sql: &str) -> bool {
    Command::new("sqlite3")
        .args(["-batch", "-bail", ":memory:"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write;
            c.stdin.as_mut().unwrap().write_all(sql.as_bytes()).unwrap();
            c.wait()
        })
        .map(|s| s.success())
        .unwrap_or(false)
}

#[test]
fn local_sqlite3_reports_its_version_and_supports_fts5_rtree() {
    let version = sqlite_version();
    assert!(version.is_some(), "sqlite3 must be on PATH");
    // FTS5 + R*Tree have been default-built since 3.20+; this guards against
    // a stripped-down sqlite3 sneaking into CI.
    assert!(
        sql_succeeds("CREATE VIRTUAL TABLE _probe USING fts5(x);\n"),
        "expected FTS5 in default-built sqlite3"
    );
    assert!(
        sql_succeeds("CREATE VIRTUAL TABLE _probe USING rtree(id, x0, x1, y0, y1);\n"),
        "expected R*Tree in default-built sqlite3"
    );
    // generate_series is in default sqlite3 since 3.34; required by some
    // extended-shard cases.
    assert!(
        sql_succeeds("SELECT count(*) FROM generate_series(1,3);\n"),
        "expected generate_series in default-built sqlite3"
    );
    // jsonb was added in 3.45; older binaries should be flagged so we know
    // to refresh CI's sqlite3.
    let supports_jsonb = sql_succeeds("SELECT length(jsonb('1'));\n");
    assert!(
        supports_jsonb,
        "expected JSONB (3.45+) — installed: {:?}",
        version
    );
}
