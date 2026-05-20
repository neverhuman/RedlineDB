// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_208_OPT_VFSTRACE_CATALOG

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 208,
        folder: r"SQLITE_PARITY_208_OPT_VFSTRACE_CATALOG",
        name: r"OPT_VFSTRACE_CATALOG",
        category: r"CLI_OPTION_CATALOG",
        priority: r"P4",
        profile: r"catalog",
        kind: r"argv",
        description: r"-vfstrace diagnostic option; catalog-only by default.",
        status: r"catalog_only",
        db: r":memory:",
        args: &[r"-vfstrace", r":memory:", r"SELECT 1;"],
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
