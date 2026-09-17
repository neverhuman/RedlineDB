// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_153_DOT_CD_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 153,
        folder: r"SQLITE_PARITY_153_DOT_CD_TEMPFILE",
        name: r"DOT_CD_TEMPFILE",
        category: r"CLI_TEMPFILE",
        priority: r"P2",
        profile: r"tempfile",
        kind: r"cli",
        description: r".cd into temp directory.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".cd {{TMP}}
.print cd-ok
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"cd-ok
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
