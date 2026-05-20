// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_203_OPT_ARCHIVE_A_TEMPFILE_OPTIONAL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 203,
        folder: r"SQLITE_PARITY_203_OPT_ARCHIVE_A_TEMPFILE_OPTIONAL",
        name: r"OPT_ARCHIVE_A_TEMPFILE_OPTIONAL",
        category: r"CLI_OPTION_TEMPFILE_OPTIONAL",
        priority: r"P4",
        profile: r"tempfile",
        kind: r"argv",
        description: r"-A archive command-line option smoke.",
        status: r"active",
        db: r":memory:",
        args: &[r"{{TMP}}/a.sqlar", r"-Acf", r"{{TMP}}/a.sqlar", r"{{TMP}}/payload.txt"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[crate::FixtureFile { path: r"payload.txt", contents: r"hello
" }],
        script: None,
        notes: r"",
    }
}
