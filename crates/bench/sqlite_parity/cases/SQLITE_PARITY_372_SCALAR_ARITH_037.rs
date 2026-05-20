// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_372_SCALAR_ARITH_037

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 372,
        folder: r"SQLITE_PARITY_372_SCALAR_ARITH_037",
        name: r"SCALAR_ARITH_037",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_037.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 37+74, 111-37, 37*38, (370)/37, (370)%38;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"111|74|1406|10|28
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
