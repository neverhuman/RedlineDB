// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_364_SCALAR_ARITH_035

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 364,
        folder: r"SQLITE_PARITY_364_SCALAR_ARITH_035",
        name: r"SCALAR_ARITH_035",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_035.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 35+70, 105-35, 35*36, (350)/35, (350)%36;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"105|70|1260|10|26
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
