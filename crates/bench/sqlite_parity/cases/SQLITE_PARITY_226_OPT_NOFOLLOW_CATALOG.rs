// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_226_OPT_NOFOLLOW_CATALOG

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 226,
        folder: r"SQLITE_PARITY_226_OPT_NOFOLLOW_CATALOG",
        name: r"OPT_NOFOLLOW_CATALOG",
        category: r"CLI_OPTION_CATALOG",
        priority: r"P4",
        profile: r"catalog",
        kind: r"argv",
        description: r"-nofollow symlink behavior is platform-specific; catalog-only.",
        status: r"catalog_only",
        db: r":memory:",
        args: &[r"-nofollow", r"{{TMP}}/link.db", r"SELECT 1;"],
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
