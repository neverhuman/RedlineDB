// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_077_COMMENTS_AND_CLI_TERMINATORS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 77,
        folder: r"SQLITE_PARITY_077_COMMENTS_AND_CLI_TERMINATORS",
        name: r"COMMENTS_AND_CLI_TERMINATORS",
        category: r"CLI_SQL_INPUT",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r"SQL comments plus CLI GO and slash statement terminators.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
-- line comment
/* block comment */ SELECT 1
GO
SELECT 2
/
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
