// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_099_CLI_UINT_COLLATION_OPTIONAL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 99,
        folder: r"SQLITE_PARITY_099_CLI_UINT_COLLATION_OPTIONAL",
        name: r"CLI_UINT_COLLATION_OPTIONAL",
        category: r"CLI_EXTENSION_OPTIONAL",
        priority: r"P3",
        profile: r"memory",
        kind: r"sql",
        description: r"CLI-bundled UINT collation.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
WITH t(x) AS (VALUES('x10'),('x2'))
SELECT x FROM t ORDER BY x COLLATE uint;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"x2
x10
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
