// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_288_SCALAR_ARITH_016

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 288,
        folder: r"SQLITE_PARITY_288_SCALAR_ARITH_016",
        name: r"SCALAR_ARITH_016",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_016.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 16+32, 48-16, 16*17, (160)/16, (160)%17;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"48|32|272|10|7
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
