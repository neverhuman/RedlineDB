// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_296_SCALAR_ARITH_018

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 296,
        folder: r"SQLITE_PARITY_296_SCALAR_ARITH_018",
        name: r"SCALAR_ARITH_018",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_018.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 18+36, 54-18, 18*19, (180)/18, (180)%19;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"54|36|342|10|9
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
