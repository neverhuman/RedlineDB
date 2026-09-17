// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_111_DOT_HEADERS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 111,
        folder: r"SQLITE_PARITY_111_DOT_HEADERS",
        name: r"DOT_HEADERS",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".headers on/off.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers on
SELECT 1 AS one,2 AS two;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"one|two
1|2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
