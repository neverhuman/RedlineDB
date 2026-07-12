// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_284_SCALAR_ARITH_015

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 284,
        folder: r"SQLITE_PARITY_284_SCALAR_ARITH_015",
        name: r"SCALAR_ARITH_015",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_015.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 15+30, 45-15, 15*16, (150)/15, (150)%16;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"45|30|240|10|6
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
