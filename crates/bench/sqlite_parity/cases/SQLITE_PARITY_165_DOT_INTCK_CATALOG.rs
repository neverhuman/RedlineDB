// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_165_DOT_INTCK_CATALOG

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 165,
        folder: r"SQLITE_PARITY_165_DOT_INTCK_CATALOG",
        name: r"DOT_INTCK_CATALOG",
        category: r"CLI_CATALOG",
        priority: r"P4",
        profile: r"catalog",
        kind: r"cli",
        description: r".intck incremental integrity check is build/version-specific; catalog entry skipped by default.",
        status: r"catalog_only",
        db: r":memory:",
        args: &[],
        stdin: r".intck
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
