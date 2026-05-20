// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_315_SCALAR_NULL_COALESCE_022

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 315,
        folder: r"SQLITE_PARITY_315_SCALAR_NULL_COALESCE_022",
        name: r"SCALAR_NULL_COALESCE_022",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_022.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT coalesce(NULL,22), ifnull(NULL,'v22'), nullif(22,22), typeof(nullif(22,23));",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"22|v22|NULL|integer
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
