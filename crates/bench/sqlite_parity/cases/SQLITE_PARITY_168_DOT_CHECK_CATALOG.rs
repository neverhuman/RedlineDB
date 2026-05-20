// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_168_DOT_CHECK_CATALOG

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 168,
        folder: r"SQLITE_PARITY_168_DOT_CHECK_CATALOG",
        name: r"DOT_CHECK_CATALOG",
        category: r"CLI_CATALOG",
        priority: r"P4",
        profile: r"catalog",
        kind: r"cli",
        description: r".check relies on shell test harness state; catalog entry skipped by default.",
        status: r"catalog_only",
        db: r":memory:",
        args: &[],
        stdin: r".check *
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
