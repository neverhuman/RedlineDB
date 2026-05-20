// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_124_DOT_BAIL_OFF

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 124,
        folder: r"SQLITE_PARITY_124_DOT_BAIL_OFF",
        name: r"DOT_BAIL_OFF",
        category: r"CLI_DOT_COMMAND_NEGATIVE",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".bail off allows later statements after error.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".bail off
SELECT bad_column;
SELECT 1;
",
        expected_exit: 1,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"1"],
        expected_stderr_contains: &[r"no such column"],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
