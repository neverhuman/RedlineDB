// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_143_DOT_QUIT

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 143,
        folder: r"SQLITE_PARITY_143_DOT_QUIT",
        name: r"DOT_QUIT",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".quit stops input interpretation.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".quit
SELECT 1;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r""),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
