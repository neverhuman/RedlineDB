// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_190_OPT_BAIL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 190,
        folder: r"SQLITE_PARITY_190_OPT_BAIL",
        name: r"OPT_BAIL",
        category: r"CLI_OPTION_NEGATIVE",
        priority: r"P1",
        profile: r"memory",
        kind: r"argv",
        description: r"-bail exits after first SQL error.",
        status: r"active",
        db: r":memory:",
        args: &[r"-bail", r":memory:", r"SELECT bad_column; SELECT 1;"],
        stdin: r"",
        expected_exit: 1,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[r"no such column"],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
