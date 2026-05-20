// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_202_OPT_APPEND_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 202,
        folder: r"SQLITE_PARITY_202_OPT_APPEND_TEMPFILE",
        name: r"OPT_APPEND_TEMPFILE",
        category: r"CLI_OPTION_TEMPFILE",
        priority: r"P4",
        profile: r"tempfile",
        kind: r"argv",
        description: r"-append option smoke with short-lived file.",
        status: r"active",
        db: r":memory:",
        args: &[r"-append", r"{{TMP}}/append.db", r"SELECT 1;"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
