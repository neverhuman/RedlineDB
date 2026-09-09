// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_137_DOT_VERSION

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 137,
        folder: r"SQLITE_PARITY_137_DOT_VERSION",
        name: r"DOT_VERSION",
        category: r"CLI_DOT_COMMAND_DIAGNOSTIC",
        priority: r"P2",
        profile: r"memory",
        kind: r"cli",
        description: r".version version output smoke.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".version
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"SQLite"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
