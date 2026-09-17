// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_112_DOT_SEPARATOR

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 112,
        folder: r"SQLITE_PARITY_112_DOT_SEPARATOR",
        name: r"DOT_SEPARATOR",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".separator column separator.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.separator :
SELECT 1,2;
",
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
