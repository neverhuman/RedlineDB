// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_157_DOT_ARCHIVE_TEMPFILE_OPTIONAL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 157,
        folder: r"SQLITE_PARITY_157_DOT_ARCHIVE_TEMPFILE_OPTIONAL",
        name: r"DOT_ARCHIVE_TEMPFILE_OPTIONAL",
        category: r"CLI_TEMPFILE_OPTIONAL",
        priority: r"P3",
        profile: r"tempfile",
        kind: r"cli",
        description: r".archive create/list using short-lived files.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".archive --create --file {{TMP}}/a.sqlar --directory {{TMP}} payload.txt
.archive --list --file {{TMP}}/a.sqlar
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"payload.txt"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[crate::FixtureFile { path: r"payload.txt", contents: r"hello
" }],
        script: None,
        notes: r"",
    }
}
