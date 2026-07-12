// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_133_DOT_AUTH

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 133,
        folder: r"SQLITE_PARITY_133_DOT_AUTH",
        name: r"DOT_AUTH",
        category: r"CLI_DOT_COMMAND_DIAGNOSTIC",
        priority: r"P3",
        profile: r"memory",
        kind: r"cli",
        description: r".auth on/off authorizer callback display.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".auth on
SELECT 1;
.auth off
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"1"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
