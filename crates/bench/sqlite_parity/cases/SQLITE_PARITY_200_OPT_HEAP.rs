// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_200_OPT_HEAP

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 200,
        folder: r"SQLITE_PARITY_200_OPT_HEAP",
        name: r"OPT_HEAP",
        category: r"CLI_OPTION",
        priority: r"P4",
        profile: r"memory",
        kind: r"argv",
        description: r"-heap smoke.",
        status: r"active",
        db: r":memory:",
        args: &[r"-heap", r"1000000", r":memory:", r"SELECT 1;"],
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
