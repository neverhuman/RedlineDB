// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_324_SCALAR_ARITH_025

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 324,
        folder: r"SQLITE_PARITY_324_SCALAR_ARITH_025",
        name: r"SCALAR_ARITH_025",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_025.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 25+50, 75-25, 25*26, (250)/25, (250)%26;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"75|50|650|10|16
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
