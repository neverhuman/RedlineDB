// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_131_DOT_TIMEOUT

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 131,
        folder: r"SQLITE_PARITY_131_DOT_TIMEOUT",
        name: r"DOT_TIMEOUT",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".timeout set busy timeout.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".timeout 1
SELECT 1;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
