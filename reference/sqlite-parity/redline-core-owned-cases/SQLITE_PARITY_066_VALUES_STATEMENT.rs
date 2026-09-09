// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_066_VALUES_STATEMENT

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 66,
        folder: r"SQLITE_PARITY_066_VALUES_STATEMENT",
        name: r"VALUES_STATEMENT",
        category: r"SQL_VALUES",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"VALUES as a standalone statement.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
VALUES(1,'a'),(2,'b');
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|a
2|b
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
