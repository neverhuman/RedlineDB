// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_155_DOT_DBTOTXT_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 155,
        folder: r"SQLITE_PARITY_155_DOT_DBTOTXT_TEMPFILE",
        name: r"DOT_DBTOTXT_TEMPFILE",
        category: r"CLI_TEMPFILE",
        priority: r"P3",
        profile: r"tempfile",
        kind: r"cli",
        description: r".dbtotxt hex dump shape.",
        status: r"active",
        db: r"{{TMP}}/case.db",
        args: &[],
        stdin: r"CREATE TABLE t(x);
INSERT INTO t VALUES(1);
.dbtotxt
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"|"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
