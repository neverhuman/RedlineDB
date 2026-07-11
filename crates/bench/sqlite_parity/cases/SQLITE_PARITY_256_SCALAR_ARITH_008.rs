// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_256_SCALAR_ARITH_008

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 256,
        folder: r"SQLITE_PARITY_256_SCALAR_ARITH_008",
        name: r"SCALAR_ARITH_008",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_008.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 8+16, 24-8, 8*9, (80)/8, (80)%9;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"24|16|72|10|8
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
