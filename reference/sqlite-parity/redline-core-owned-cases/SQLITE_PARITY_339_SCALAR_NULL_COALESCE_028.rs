// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_339_SCALAR_NULL_COALESCE_028

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 339,
        folder: r"SQLITE_PARITY_339_SCALAR_NULL_COALESCE_028",
        name: r"SCALAR_NULL_COALESCE_028",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_028.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT coalesce(NULL,28), ifnull(NULL,'v28'), nullif(28,28), typeof(nullif(28,29));",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"28|v28|NULL|integer
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
