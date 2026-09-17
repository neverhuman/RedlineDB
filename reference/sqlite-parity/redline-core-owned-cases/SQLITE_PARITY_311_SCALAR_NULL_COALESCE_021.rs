// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_311_SCALAR_NULL_COALESCE_021

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 311,
        folder: r"SQLITE_PARITY_311_SCALAR_NULL_COALESCE_021",
        name: r"SCALAR_NULL_COALESCE_021",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_021.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT coalesce(NULL,21), ifnull(NULL,'v21'), nullif(21,21), typeof(nullif(21,22));",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"21|v21|NULL|integer
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
