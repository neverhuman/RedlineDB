// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_340_SCALAR_ARITH_029

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 340,
        folder: r"SQLITE_PARITY_340_SCALAR_ARITH_029",
        name: r"SCALAR_ARITH_029",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_029.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 29+58, 87-29, 29*30, (290)/29, (290)%30;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"87|58|870|10|20
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
