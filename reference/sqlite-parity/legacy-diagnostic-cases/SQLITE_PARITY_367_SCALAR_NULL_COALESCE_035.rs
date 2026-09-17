// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_367_SCALAR_NULL_COALESCE_035

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 367,
        folder: r"SQLITE_PARITY_367_SCALAR_NULL_COALESCE_035",
        name: r"SCALAR_NULL_COALESCE_035",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_035.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT coalesce(NULL,35), ifnull(NULL,'v35'), nullif(35,35), typeof(nullif(35,36));",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"35|v35|NULL|integer
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
