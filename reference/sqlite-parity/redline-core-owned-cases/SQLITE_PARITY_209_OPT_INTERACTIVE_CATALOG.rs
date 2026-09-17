// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_209_OPT_INTERACTIVE_CATALOG

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 209,
        folder: r"SQLITE_PARITY_209_OPT_INTERACTIVE_CATALOG",
        name: r"OPT_INTERACTIVE_CATALOG",
        category: r"CLI_OPTION_CATALOG",
        priority: r"P4",
        profile: r"catalog",
        kind: r"argv",
        description: r"-interactive requires terminal behavior; catalog-only by default.",
        status: r"catalog_only",
        db: r":memory:",
        args: &[r"-interactive", r":memory:"],
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
