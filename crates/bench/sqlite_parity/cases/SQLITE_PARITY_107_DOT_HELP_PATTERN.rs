// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_107_DOT_HELP_PATTERN

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 107,
        folder: r"SQLITE_PARITY_107_DOT_HELP_PATTERN",
        name: r"DOT_HELP_PATTERN",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".help TOPIC for mode.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".help mode
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r".mode"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
