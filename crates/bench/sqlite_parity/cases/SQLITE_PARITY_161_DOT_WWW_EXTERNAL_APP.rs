// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_161_DOT_WWW_EXTERNAL_APP

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 161,
        folder: r"SQLITE_PARITY_161_DOT_WWW_EXTERNAL_APP",
        name: r"DOT_WWW_EXTERNAL_APP",
        category: r"CLI_EXTERNAL_APP",
        priority: r"P4",
        profile: r"external_app",
        kind: r"cli",
        description: r".www opens browser; catalog-only by default.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".www
SELECT 1;
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
