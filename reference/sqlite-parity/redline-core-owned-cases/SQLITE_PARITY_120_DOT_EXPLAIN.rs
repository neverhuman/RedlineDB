// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_120_DOT_EXPLAIN

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 120,
        folder: r"SQLITE_PARITY_120_DOT_EXPLAIN",
        name: r"DOT_EXPLAIN",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".explain on formats EXPLAIN output.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".explain on
EXPLAIN SELECT 1;
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"addr", r"opcode", r"Init"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
