// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_194_OPT_IFEXISTS_NEGATIVE_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 194,
        folder: r"SQLITE_PARITY_194_OPT_IFEXISTS_NEGATIVE_TEMPFILE",
        name: r"OPT_IFEXISTS_NEGATIVE_TEMPFILE",
        category: r"CLI_OPTION_TEMPFILE",
        priority: r"P2",
        profile: r"tempfile",
        kind: r"argv",
        description: r"-ifexists refuses missing temp db.",
        status: r"active",
        db: r":memory:",
        args: &[r"-ifexists", r"{{TMP}}/missing.db", r"SELECT 1;"],
        stdin: r"",
        expected_exit: 1,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[r"unable to open database"],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
