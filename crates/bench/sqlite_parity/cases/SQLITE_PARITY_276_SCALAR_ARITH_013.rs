// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_276_SCALAR_ARITH_013

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 276,
        folder: r"SQLITE_PARITY_276_SCALAR_ARITH_013",
        name: r"SCALAR_ARITH_013",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_013.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 13+26, 39-13, 13*14, (130)/13, (130)%14;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"39|26|182|10|4
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
