// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_132_DOT_TRACE_STDOUT

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 132,
        folder: r"SQLITE_PARITY_132_DOT_TRACE_STDOUT",
        name: r"DOT_TRACE_STDOUT",
        category: r"CLI_DOT_COMMAND_DIAGNOSTIC",
        priority: r"P2",
        profile: r"memory",
        kind: r"cli",
        description: r".trace stdout emits SQL trace.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".trace stdout
SELECT 1;
.trace off
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"SELECT 1;", r"1"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
