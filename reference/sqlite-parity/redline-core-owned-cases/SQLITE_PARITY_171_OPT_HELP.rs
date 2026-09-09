// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_171_OPT_HELP

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 171,
        folder: r"SQLITE_PARITY_171_OPT_HELP",
        name: r"OPT_HELP",
        category: r"CLI_OPTION",
        priority: r"P1",
        profile: r"memory",
        kind: r"argv",
        description: r"-help command-line option.",
        status: r"active",
        db: r":memory:",
        args: &[r"-help"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[r"OPTIONS include"],
        files: &[],
        script: None,
        notes: r"",
    }
}
