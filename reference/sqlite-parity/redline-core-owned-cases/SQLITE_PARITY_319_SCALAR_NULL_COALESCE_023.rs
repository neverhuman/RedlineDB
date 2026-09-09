// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_319_SCALAR_NULL_COALESCE_023

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 319,
        folder: r"SQLITE_PARITY_319_SCALAR_NULL_COALESCE_023",
        name: r"SCALAR_NULL_COALESCE_023",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_023.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT coalesce(NULL,23), ifnull(NULL,'v23'), nullif(23,23), typeof(nullif(23,24));",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"23|v23|NULL|integer
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
