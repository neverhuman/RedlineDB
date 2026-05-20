// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_193_OPT_READONLY_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 193,
        folder: r"SQLITE_PARITY_193_OPT_READONLY_TEMPFILE",
        name: r"OPT_READONLY_TEMPFILE",
        category: r"CLI_OPTION_TEMPFILE",
        priority: r"P2",
        profile: r"tempfile",
        kind: r"script",
        description: r"-readonly opens temp db read-only.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: Some(r#"#!/usr/bin/env bash
set -euo pipefail
"$SQLITE_BIN" "$SQLITE_PARITY_TMP/ro.db" 'CREATE TABLE t(x); INSERT INTO t VALUES(1);'
chmod 0444 "$SQLITE_PARITY_TMP/ro.db"
"$SQLITE_BIN" -readonly "$SQLITE_PARITY_TMP/ro.db" 'SELECT x FROM t;'
"#),
        notes: r"",
    }
}
