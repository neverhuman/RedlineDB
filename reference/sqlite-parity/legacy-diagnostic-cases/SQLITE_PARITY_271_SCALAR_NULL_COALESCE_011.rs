// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_271_SCALAR_NULL_COALESCE_011

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 271,
        folder: r"SQLITE_PARITY_271_SCALAR_NULL_COALESCE_011",
        name: r"SCALAR_NULL_COALESCE_011",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_011.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT coalesce(NULL,11), ifnull(NULL,'v11'), nullif(11,11), typeof(nullif(11,12));",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"11|v11|NULL|integer
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
