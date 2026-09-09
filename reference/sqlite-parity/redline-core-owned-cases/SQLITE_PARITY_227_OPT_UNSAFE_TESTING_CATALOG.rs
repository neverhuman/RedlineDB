// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_227_OPT_UNSAFE_TESTING_CATALOG

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 227,
        folder: r"SQLITE_PARITY_227_OPT_UNSAFE_TESTING_CATALOG",
        name: r"OPT_UNSAFE_TESTING_CATALOG",
        category: r"CLI_OPTION_CATALOG",
        priority: r"P4",
        profile: r"catalog",
        kind: r"argv",
        description: r"-unsafe-testing enables dangerous test controls; catalog-only.",
        status: r"catalog_only",
        db: r":memory:",
        args: &[r"-unsafe-testing", r":memory:", r"SELECT 1;"],
        stdin: r"",
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
