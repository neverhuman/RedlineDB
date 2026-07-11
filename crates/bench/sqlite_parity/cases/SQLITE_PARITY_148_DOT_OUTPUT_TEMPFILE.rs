// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_148_DOT_OUTPUT_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 148,
        folder: r"SQLITE_PARITY_148_DOT_OUTPUT_TEMPFILE",
        name: r"DOT_OUTPUT_TEMPFILE",
        category: r"CLI_TEMPFILE",
        priority: r"P1",
        profile: r"tempfile",
        kind: r"cli",
        description: r".output writes next results to temp file then readfile verifies.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".output {{TMP}}/out.txt
SELECT 1;
.output
SELECT hex(readfile('{{TMP}}/out.txt'));
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"310A
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
