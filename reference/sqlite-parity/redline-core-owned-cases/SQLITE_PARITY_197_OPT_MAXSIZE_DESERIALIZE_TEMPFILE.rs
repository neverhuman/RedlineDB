// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_197_OPT_MAXSIZE_DESERIALIZE_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 197,
        folder: r"SQLITE_PARITY_197_OPT_MAXSIZE_DESERIALIZE_TEMPFILE",
        name: r"OPT_MAXSIZE_DESERIALIZE_TEMPFILE",
        category: r"CLI_OPTION_TEMPFILE",
        priority: r"P3",
        profile: r"tempfile",
        kind: r"script",
        description: r"-deserialize/-maxsize smoke with temp db.",
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
"$SQLITE_BIN" "$SQLITE_PARITY_TMP/d.db" 'CREATE TABLE t(x); INSERT INTO t VALUES(1);'
"$SQLITE_BIN" -deserialize -maxsize 1000000 "$SQLITE_PARITY_TMP/d.db" 'SELECT x FROM t;'
"#),
        notes: r"",
    }
}
