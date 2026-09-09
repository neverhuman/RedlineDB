// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_370_SCALAR_STRING_036

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 370,
        folder: r"SQLITE_PARITY_370_SCALAR_STRING_036",
        name: r"SCALAR_STRING_036",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_036.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc36'), substr('abcdef36',2,3), upper('a36b'), lower('A36B'), replace('a-b-c','-','6');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"5|bcd|A36B|a36b|a6b6c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
