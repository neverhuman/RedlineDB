// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_260_SCALAR_ARITH_009

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 260,
        folder: r"SQLITE_PARITY_260_SCALAR_ARITH_009",
        name: r"SCALAR_ARITH_009",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_009.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 9+18, 27-9, 9*10, (90)/9, (90)%10;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"27|18|90|10|0
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
