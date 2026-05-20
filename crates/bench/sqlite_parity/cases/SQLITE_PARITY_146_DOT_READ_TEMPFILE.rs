// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_146_DOT_READ_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 146,
        folder: r"SQLITE_PARITY_146_DOT_READ_TEMPFILE",
        name: r"DOT_READ_TEMPFILE",
        category: r"CLI_TEMPFILE",
        priority: r"P1",
        profile: r"tempfile",
        kind: r"cli",
        description: r".read from a short-lived temp SQL file.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".read {{TMP}}/script.sql
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"7
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[crate::FixtureFile { path: r"script.sql", contents: r".mode list
SELECT 7;
" }],
        script: None,
        notes: r"",
    }
}
