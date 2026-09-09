// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_163_DOT_FILECTRL_CATALOG

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 163,
        folder: r"SQLITE_PARITY_163_DOT_FILECTRL_CATALOG",
        name: r"DOT_FILECTRL_CATALOG",
        category: r"CLI_CATALOG",
        priority: r"P4",
        profile: r"catalog",
        kind: r"cli",
        description: r".filectrl is build/VFS-specific; catalog entry intentionally skipped unless custom case is added.",
        status: r"catalog_only",
        db: r":memory:",
        args: &[],
        stdin: r".filectrl
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Requires selecting a concrete file control operation for the target VFS.",
    }
}
