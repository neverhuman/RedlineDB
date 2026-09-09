// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_113_DOT_NULLVALUE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 113,
        folder: r"SQLITE_PARITY_113_DOT_NULLVALUE",
        name: r"DOT_NULLVALUE",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".nullvalue rendering.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.nullvalue NULL
SELECT NULL,1;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"NULL|1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
