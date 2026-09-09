// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_316_SCALAR_ARITH_023

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 316,
        folder: r"SQLITE_PARITY_316_SCALAR_ARITH_023",
        name: r"SCALAR_ARITH_023",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_023.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 23+46, 69-23, 23*24, (230)/23, (230)%24;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"69|46|552|10|14
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
