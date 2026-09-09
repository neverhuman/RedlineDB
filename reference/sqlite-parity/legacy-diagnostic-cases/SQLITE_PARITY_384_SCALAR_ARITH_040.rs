// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_384_SCALAR_ARITH_040

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 384,
        folder: r"SQLITE_PARITY_384_SCALAR_ARITH_040",
        name: r"SCALAR_ARITH_040",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_040.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 40+80, 120-40, 40*41, (400)/40, (400)%41;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"120|80|1640|10|31
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
