// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_069_COLLATE_NOCASE_RTRIM_BINARY

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 69,
        folder: r"SQLITE_PARITY_069_COLLATE_NOCASE_RTRIM_BINARY",
        name: r"COLLATE_NOCASE_RTRIM_BINARY",
        category: r"SQL_COLLATION",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"NOCASE, RTRIM, BINARY collation behavior.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 'A' = 'a' COLLATE NOCASE,
       'x ' = 'x' COLLATE RTRIM,
       'A' = 'a' COLLATE BINARY;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|1|0
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
