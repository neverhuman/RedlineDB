// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_134_DOT_CRLF

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 134,
        folder: r"SQLITE_PARITY_134_DOT_CRLF",
        name: r"DOT_CRLF",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".crlf on/off with normalized line endings.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".crlf on
SELECT 1;
.crlf off
SELECT 2;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1
2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
