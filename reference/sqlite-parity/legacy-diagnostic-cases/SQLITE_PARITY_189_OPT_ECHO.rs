// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_189_OPT_ECHO

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 189,
        folder: r"SQLITE_PARITY_189_OPT_ECHO",
        name: r"OPT_ECHO",
        category: r"CLI_OPTION",
        priority: r"P2",
        profile: r"memory",
        kind: r"argv",
        description: r"-echo echoes input.",
        status: r"active",
        db: r":memory:",
        args: &[r"-echo", r":memory:", r"SELECT 1;"],
        stdin: r"",
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
