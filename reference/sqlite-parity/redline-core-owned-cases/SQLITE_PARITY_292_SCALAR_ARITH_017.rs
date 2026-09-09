// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_292_SCALAR_ARITH_017

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 292,
        folder: r"SQLITE_PARITY_292_SCALAR_ARITH_017",
        name: r"SCALAR_ARITH_017",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_017.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 17+34, 51-17, 17*18, (170)/17, (170)%18;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"51|34|306|10|8
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
