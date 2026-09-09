// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_180_OPT_BOX_MODE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 180,
        folder: r"SQLITE_PARITY_180_OPT_BOX_MODE",
        name: r"OPT_BOX_MODE",
        category: r"CLI_OPTION",
        priority: r"P2",
        profile: r"memory",
        kind: r"argv",
        description: r"-box output mode smoke.",
        status: r"active",
        db: r":memory:",
        args: &[r"-box", r":memory:", r"SELECT 1 AS a, 'x' AS b;"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"a", r"x"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
