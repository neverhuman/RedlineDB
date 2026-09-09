// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_244_SCALAR_ARITH_005

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 244,
        folder: r"SQLITE_PARITY_244_SCALAR_ARITH_005",
        name: r"SCALAR_ARITH_005",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_005.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 5+10, 15-5, 5*6, (50)/5, (50)%6;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"15|10|30|10|2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
