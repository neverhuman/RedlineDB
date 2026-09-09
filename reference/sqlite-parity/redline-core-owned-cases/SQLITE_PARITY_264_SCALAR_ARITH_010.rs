// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_264_SCALAR_ARITH_010

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 264,
        folder: r"SQLITE_PARITY_264_SCALAR_ARITH_010",
        name: r"SCALAR_ARITH_010",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_010.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 10+20, 30-10, 10*11, (100)/10, (100)%11;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"30|20|110|10|1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
