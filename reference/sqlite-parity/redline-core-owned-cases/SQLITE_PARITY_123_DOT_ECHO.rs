// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_123_DOT_ECHO

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 123,
        folder: r"SQLITE_PARITY_123_DOT_ECHO",
        name: r"DOT_ECHO",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".echo on echoes input.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".echo on
SELECT 1;
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
