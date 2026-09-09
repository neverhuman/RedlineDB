// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_240_SCALAR_ARITH_004

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 240,
        folder: r"SQLITE_PARITY_240_SCALAR_ARITH_004",
        name: r"SCALAR_ARITH_004",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_004.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 4+8, 12-4, 4*5, (40)/4, (40)%5;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"12|8|20|10|0
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
