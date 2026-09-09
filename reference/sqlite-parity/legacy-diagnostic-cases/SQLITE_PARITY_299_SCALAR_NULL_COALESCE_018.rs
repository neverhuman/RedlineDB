// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_299_SCALAR_NULL_COALESCE_018

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 299,
        folder: r"SQLITE_PARITY_299_SCALAR_NULL_COALESCE_018",
        name: r"SCALAR_NULL_COALESCE_018",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_018.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT coalesce(NULL,18), ifnull(NULL,'v18'), nullif(18,18), typeof(nullif(18,19));",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"18|v18|NULL|integer
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
