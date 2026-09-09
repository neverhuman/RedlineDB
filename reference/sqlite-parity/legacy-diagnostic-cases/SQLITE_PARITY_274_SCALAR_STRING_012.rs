// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_274_SCALAR_STRING_012

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 274,
        folder: r"SQLITE_PARITY_274_SCALAR_STRING_012",
        name: r"SCALAR_STRING_012",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_012.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc12'), substr('abcdef12',2,3), upper('a12b'), lower('A12B'), replace('a-b-c','-','2');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"5|bcd|A12B|a12b|a2b2c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
