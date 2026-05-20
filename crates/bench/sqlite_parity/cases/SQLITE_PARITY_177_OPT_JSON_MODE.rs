// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_177_OPT_JSON_MODE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 177,
        folder: r"SQLITE_PARITY_177_OPT_JSON_MODE",
        name: r"OPT_JSON_MODE",
        category: r"CLI_OPTION",
        priority: r"P1",
        profile: r"memory",
        kind: r"argv",
        description: r"-json output mode.",
        status: r"active",
        db: r":memory:",
        args: &[r"-json", r":memory:", r"SELECT 1 AS a, 'x' AS b;"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r#"[{"a":1,"b":"x"}]
"#),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
