// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_156_DOT_RECOVER_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 156,
        folder: r"SQLITE_PARITY_156_DOT_RECOVER_TEMPFILE",
        name: r"DOT_RECOVER_TEMPFILE",
        category: r"CLI_TEMPFILE",
        priority: r"P3",
        profile: r"tempfile",
        kind: r"cli",
        description: r".recover on valid short-lived db file emits recovery SQL.",
        status: r"active",
        db: r"{{TMP}}/case.db",
        args: &[],
        stdin: r"CREATE TABLE t(x);
INSERT INTO t VALUES(1);
.recover
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"CREATE TABLE", r"INSERT"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
