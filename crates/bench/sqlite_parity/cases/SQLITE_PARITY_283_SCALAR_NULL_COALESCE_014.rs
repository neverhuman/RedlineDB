// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_283_SCALAR_NULL_COALESCE_014

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 283,
        folder: r"SQLITE_PARITY_283_SCALAR_NULL_COALESCE_014",
        name: r"SCALAR_NULL_COALESCE_014",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_014.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT coalesce(NULL,14), ifnull(NULL,'v14'), nullif(14,14), typeof(nullif(14,15));",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"14|v14|NULL|integer
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
