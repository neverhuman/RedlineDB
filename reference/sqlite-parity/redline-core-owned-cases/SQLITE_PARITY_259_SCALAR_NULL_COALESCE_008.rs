// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_259_SCALAR_NULL_COALESCE_008

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 259,
        folder: r"SQLITE_PARITY_259_SCALAR_NULL_COALESCE_008",
        name: r"SCALAR_NULL_COALESCE_008",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_008.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT coalesce(NULL,8), ifnull(NULL,'v8'), nullif(8,8), typeof(nullif(8,9));",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"8|v8|NULL|integer
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
