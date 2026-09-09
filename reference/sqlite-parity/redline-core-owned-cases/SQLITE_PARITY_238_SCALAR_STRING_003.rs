// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_238_SCALAR_STRING_003

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 238,
        folder: r"SQLITE_PARITY_238_SCALAR_STRING_003",
        name: r"SCALAR_STRING_003",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_003.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc3'), substr('abcdef3',2,3), upper('a3b'), lower('A3B'), replace('a-b-c','-','3');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"4|bcd|A3B|a3b|a3b3c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
