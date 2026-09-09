// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_191_OPT_BATCH

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 191,
        folder: r"SQLITE_PARITY_191_OPT_BATCH",
        name: r"OPT_BATCH",
        category: r"CLI_OPTION",
        priority: r"P2",
        profile: r"memory",
        kind: r"argv",
        description: r"-batch smoke.",
        status: r"active",
        db: r":memory:",
        args: &[r"-batch", r":memory:", r"SELECT 1;"],
        stdin: r"",
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
