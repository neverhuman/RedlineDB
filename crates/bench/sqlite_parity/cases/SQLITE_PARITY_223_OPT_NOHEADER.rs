// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_223_OPT_NOHEADER

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 223,
        folder: r"SQLITE_PARITY_223_OPT_NOHEADER",
        name: r"OPT_NOHEADER",
        category: r"CLI_OPTION",
        priority: r"P2",
        profile: r"memory",
        kind: r"argv",
        description: r"-noheader overrides header output.",
        status: r"active",
        db: r":memory:",
        args: &[r"-header", r"-noheader", r":memory:", r"SELECT 1 AS one;"],
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
