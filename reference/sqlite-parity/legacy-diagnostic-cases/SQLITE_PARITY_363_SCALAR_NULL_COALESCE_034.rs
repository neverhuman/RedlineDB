// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_363_SCALAR_NULL_COALESCE_034

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 363,
        folder: r"SQLITE_PARITY_363_SCALAR_NULL_COALESCE_034",
        name: r"SCALAR_NULL_COALESCE_034",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_034.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT coalesce(NULL,34), ifnull(NULL,'v34'), nullif(34,34), typeof(nullif(34,35));",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"34|v34|NULL|integer
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
