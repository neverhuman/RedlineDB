// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_170_OPT_VERSION

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 170,
        folder: r"SQLITE_PARITY_170_OPT_VERSION",
        name: r"OPT_VERSION",
        category: r"CLI_OPTION",
        priority: r"P1",
        profile: r"memory",
        kind: r"argv",
        description: r"-version command-line option.",
        status: r"active",
        db: r":memory:",
        args: &[r"-version"],
        stdin: r"",
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
