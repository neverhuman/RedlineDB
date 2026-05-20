// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_185_OPT_SEPARATOR

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 185,
        folder: r"SQLITE_PARITY_185_OPT_SEPARATOR",
        name: r"OPT_SEPARATOR",
        category: r"CLI_OPTION",
        priority: r"P1",
        profile: r"memory",
        kind: r"argv",
        description: r"-separator output separator.",
        status: r"active",
        db: r":memory:",
        args: &[r"-separator", r":", r":memory:", r"SELECT 1,2;"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1:2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
