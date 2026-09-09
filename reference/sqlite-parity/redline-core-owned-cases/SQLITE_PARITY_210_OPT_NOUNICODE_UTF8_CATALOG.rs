// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_210_OPT_NOUNICODE_UTF8_CATALOG

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 210,
        folder: r"SQLITE_PARITY_210_OPT_NOUNICODE_UTF8_CATALOG",
        name: r"OPT_NOUNICODE_UTF8_CATALOG",
        category: r"CLI_OPTION_CATALOG",
        priority: r"P4",
        profile: r"catalog",
        kind: r"argv",
        description: r"-utf8 / -no-utf8 are Windows console compatibility no-ops; catalog-only by default.",
        status: r"catalog_only",
        db: r":memory:",
        args: &[r"-utf8", r":memory:", r"SELECT 1;"],
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
