// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_098_CLI_REGEXP_OPTIONAL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 98,
        folder: r"SQLITE_PARITY_098_CLI_REGEXP_OPTIONAL",
        name: r"CLI_REGEXP_OPTIONAL",
        category: r"CLI_EXTENSION_OPTIONAL",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql",
        description: r"CLI-bundled REGEXP operator support.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 'abc' REGEXP '^a', 'abc' REGEXP 'z$';
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|0
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
