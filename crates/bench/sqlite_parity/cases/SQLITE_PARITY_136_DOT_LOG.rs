// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_136_DOT_LOG

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 136,
        folder: r"SQLITE_PARITY_136_DOT_LOG",
        name: r"DOT_LOG",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".log stdout/on/off smoke.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".log stdout
SELECT 1;
.log off
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
