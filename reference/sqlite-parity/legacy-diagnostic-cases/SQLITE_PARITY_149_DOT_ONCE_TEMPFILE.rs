// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_149_DOT_ONCE_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 149,
        folder: r"SQLITE_PARITY_149_DOT_ONCE_TEMPFILE",
        name: r"DOT_ONCE_TEMPFILE",
        category: r"CLI_TEMPFILE",
        priority: r"P1",
        profile: r"tempfile",
        kind: r"cli",
        description: r".once writes one statement to temp file only.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".once {{TMP}}/once.txt
SELECT 2;
SELECT 3;
SELECT hex(readfile('{{TMP}}/once.txt'));
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"3
320A
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
