// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_114_DOT_PRINT

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 114,
        folder: r"SQLITE_PARITY_114_DOT_PRINT",
        name: r"DOT_PRINT",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".print literal output.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".print hello world
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"hello world
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
