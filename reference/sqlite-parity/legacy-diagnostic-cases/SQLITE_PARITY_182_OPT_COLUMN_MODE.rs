// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_182_OPT_COLUMN_MODE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 182,
        folder: r"SQLITE_PARITY_182_OPT_COLUMN_MODE",
        name: r"OPT_COLUMN_MODE",
        category: r"CLI_OPTION",
        priority: r"P2",
        profile: r"memory",
        kind: r"argv",
        description: r"-column output mode smoke.",
        status: r"active",
        db: r":memory:",
        args: &[r"-column", r":memory:", r"SELECT 1 AS a, 'x' AS b;"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"1", r"x"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
