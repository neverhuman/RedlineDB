// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_151_DOT_SAVE_RESTORE_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 151,
        folder: r"SQLITE_PARITY_151_DOT_SAVE_RESTORE_TEMPFILE",
        name: r"DOT_SAVE_RESTORE_TEMPFILE",
        category: r"CLI_TEMPFILE",
        priority: r"P1",
        profile: r"tempfile",
        kind: r"cli",
        description: r".save alias for backup, then restore.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r"CREATE TABLE t(x);
INSERT INTO t VALUES(6);
.save {{TMP}}/s.db
.open :memory:
.restore main {{TMP}}/s.db
SELECT x FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"6
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
