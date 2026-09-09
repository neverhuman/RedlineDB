// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_129_DOT_CONNECTION

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 129,
        folder: r"SQLITE_PARITY_129_DOT_CONNECTION",
        name: r"DOT_CONNECTION",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".connection open/switch connections.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".connection
.connection 1
.open :memory:
SELECT 1;
.connection 0
SELECT 2;
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"1", r"2"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
