// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_375_SCALAR_NULL_COALESCE_037

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 375,
        folder: r"SQLITE_PARITY_375_SCALAR_NULL_COALESCE_037",
        name: r"SCALAR_NULL_COALESCE_037",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_037.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT coalesce(NULL,37), ifnull(NULL,'v37'), nullif(37,37), typeof(nullif(37,38));",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"37|v37|NULL|integer
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
