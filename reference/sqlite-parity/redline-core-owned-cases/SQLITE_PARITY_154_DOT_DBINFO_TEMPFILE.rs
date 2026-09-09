// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_154_DOT_DBINFO_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 154,
        folder: r"SQLITE_PARITY_154_DOT_DBINFO_TEMPFILE",
        name: r"DOT_DBINFO_TEMPFILE",
        category: r"CLI_TEMPFILE",
        priority: r"P2",
        profile: r"tempfile",
        kind: r"cli",
        description: r".dbinfo on temp database file.",
        status: r"active",
        db: r"{{TMP}}/case.db",
        args: &[],
        stdin: r"CREATE TABLE t(x);
.dbinfo
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"database page size"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
