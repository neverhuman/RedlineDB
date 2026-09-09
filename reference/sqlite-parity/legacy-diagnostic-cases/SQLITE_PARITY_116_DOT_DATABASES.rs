// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_116_DOT_DATABASES

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 116,
        folder: r"SQLITE_PARITY_116_DOT_DATABASES",
        name: r"DOT_DATABASES",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".databases on in-memory database.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".databases
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"main"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
