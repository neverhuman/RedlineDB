// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_152_DOT_CLONE_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 152,
        folder: r"SQLITE_PARITY_152_DOT_CLONE_TEMPFILE",
        name: r"DOT_CLONE_TEMPFILE",
        category: r"CLI_TEMPFILE",
        priority: r"P2",
        profile: r"tempfile",
        kind: r"cli",
        description: r".clone into short-lived temp database file.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r"CREATE TABLE t(x);
INSERT INTO t VALUES(8);
.clone {{TMP}}/clone.db
.open {{TMP}}/clone.db
SELECT x FROM t;
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"8"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
