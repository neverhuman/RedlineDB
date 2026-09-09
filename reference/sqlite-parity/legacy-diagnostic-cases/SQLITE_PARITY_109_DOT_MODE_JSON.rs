// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_109_DOT_MODE_JSON

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 109,
        folder: r"SQLITE_PARITY_109_DOT_MODE_JSON",
        name: r"DOT_MODE_JSON",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".mode json output.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode json
SELECT 1 AS a,'x' AS b;
",
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
