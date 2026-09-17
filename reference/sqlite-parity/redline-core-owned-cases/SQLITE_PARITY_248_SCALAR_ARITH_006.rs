// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_248_SCALAR_ARITH_006

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 248,
        folder: r"SQLITE_PARITY_248_SCALAR_ARITH_006",
        name: r"SCALAR_ARITH_006",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_006.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 6+12, 18-6, 6*7, (60)/6, (60)%7;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"18|12|42|10|4
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
