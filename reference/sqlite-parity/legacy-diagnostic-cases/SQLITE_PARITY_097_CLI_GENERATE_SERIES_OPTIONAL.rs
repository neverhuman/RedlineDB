// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_097_CLI_GENERATE_SERIES_OPTIONAL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 97,
        folder: r"SQLITE_PARITY_097_CLI_GENERATE_SERIES_OPTIONAL",
        name: r"CLI_GENERATE_SERIES_OPTIONAL",
        category: r"CLI_EXTENSION_OPTIONAL",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql",
        description: r"CLI-bundled generate_series() table-valued function.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT value FROM generate_series(1,3);
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1
2
3
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
