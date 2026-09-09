// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_204_OPT_ZIP_TEMPFILE_CATALOG

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 204,
        folder: r"SQLITE_PARITY_204_OPT_ZIP_TEMPFILE_CATALOG",
        name: r"OPT_ZIP_TEMPFILE_CATALOG",
        category: r"CLI_OPTION_CATALOG",
        priority: r"P4",
        profile: r"catalog",
        kind: r"argv",
        description: r"-zip opens ZIP archive; catalog-only unless a zip fixture is added.",
        status: r"catalog_only",
        db: r":memory:",
        args: &[r"-zip", r"{{TMP}}/archive.zip", r".schema"],
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
