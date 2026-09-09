// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_160_DOT_EXCEL_EXTERNAL_APP

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 160,
        folder: r"SQLITE_PARITY_160_DOT_EXCEL_EXTERNAL_APP",
        name: r"DOT_EXCEL_EXTERNAL_APP",
        category: r"CLI_EXTERNAL_APP",
        priority: r"P4",
        profile: r"external_app",
        kind: r"cli",
        description: r".excel opens spreadsheet app; catalog-only by default.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".excel
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
