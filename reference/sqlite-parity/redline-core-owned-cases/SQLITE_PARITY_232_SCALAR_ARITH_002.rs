// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_232_SCALAR_ARITH_002

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 232,
        folder: r"SQLITE_PARITY_232_SCALAR_ARITH_002",
        name: r"SCALAR_ARITH_002",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_002.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 2+4, 6-2, 2*3, (20)/2, (20)%3;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"6|4|6|10|2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
