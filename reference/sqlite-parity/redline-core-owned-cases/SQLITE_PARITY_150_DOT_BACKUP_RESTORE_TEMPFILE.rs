// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_150_DOT_BACKUP_RESTORE_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 150,
        folder: r"SQLITE_PARITY_150_DOT_BACKUP_RESTORE_TEMPFILE",
        name: r"DOT_BACKUP_RESTORE_TEMPFILE",
        category: r"CLI_TEMPFILE",
        priority: r"P1",
        profile: r"tempfile",
        kind: r"cli",
        description: r".backup and .restore via short-lived temp database file.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r"CREATE TABLE t(x);
INSERT INTO t VALUES(5);
.backup {{TMP}}/b.db
.open :memory:
.restore main {{TMP}}/b.db
SELECT x FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"5
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
