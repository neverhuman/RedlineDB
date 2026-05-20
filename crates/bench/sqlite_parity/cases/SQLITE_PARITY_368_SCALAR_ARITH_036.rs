// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_368_SCALAR_ARITH_036

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 368,
        folder: r"SQLITE_PARITY_368_SCALAR_ARITH_036",
        name: r"SCALAR_ARITH_036",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_036.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 36+72, 108-36, 36*37, (360)/36, (360)%37;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"108|72|1332|10|27
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
