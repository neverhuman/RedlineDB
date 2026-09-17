// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_127_DOT_LIMIT

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 127,
        folder: r"SQLITE_PARITY_127_DOT_LIMIT",
        name: r"DOT_LIMIT",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".limit query/change SQLITE_LIMIT.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".limit length 1000000
.limit length
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"length"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
