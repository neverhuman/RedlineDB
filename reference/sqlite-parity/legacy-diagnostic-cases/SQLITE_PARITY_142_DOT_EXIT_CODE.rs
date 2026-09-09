// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_142_DOT_EXIT_CODE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 142,
        folder: r"SQLITE_PARITY_142_DOT_EXIT_CODE",
        name: r"DOT_EXIT_CODE",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".exit CODE returns that process code.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".exit 7
",
        expected_exit: 7,
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
