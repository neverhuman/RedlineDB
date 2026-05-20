// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_252_SCALAR_ARITH_007

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 252,
        folder: r"SQLITE_PARITY_252_SCALAR_ARITH_007",
        name: r"SCALAR_ARITH_007",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_007.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 7+14, 21-7, 7*8, (70)/7, (70)%8;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"21|14|56|10|6
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
