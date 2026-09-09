// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_141_DOT_SHA3SUM

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 141,
        folder: r"SQLITE_PARITY_141_DOT_SHA3SUM",
        name: r"DOT_SHA3SUM",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".sha3sum database content hash shape.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r"CREATE TABLE t(x);
INSERT INTO t VALUES(1);
.sha3sum
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
