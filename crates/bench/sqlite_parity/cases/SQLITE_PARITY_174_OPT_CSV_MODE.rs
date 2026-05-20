// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_174_OPT_CSV_MODE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 174,
        folder: r"SQLITE_PARITY_174_OPT_CSV_MODE",
        name: r"OPT_CSV_MODE",
        category: r"CLI_OPTION",
        priority: r"P1",
        profile: r"memory",
        kind: r"argv",
        description: r"-csv output mode.",
        status: r"active",
        db: r":memory:",
        args: &[r"-csv", r":memory:", r"SELECT 1 AS a, 'x,y' AS b;"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r#"1,"x,y"
"#),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
