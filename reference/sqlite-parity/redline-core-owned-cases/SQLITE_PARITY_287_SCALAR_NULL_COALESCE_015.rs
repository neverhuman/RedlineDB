// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_287_SCALAR_NULL_COALESCE_015

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 287,
        folder: r"SQLITE_PARITY_287_SCALAR_NULL_COALESCE_015",
        name: r"SCALAR_NULL_COALESCE_015",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_015.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT coalesce(NULL,15), ifnull(NULL,'v15'), nullif(15,15), typeof(nullif(15,16));",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"15|v15|NULL|integer
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
