// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_325_SCALAR_CAST_TYPEOF_025

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 325,
        folder: r"SQLITE_PARITY_325_SCALAR_CAST_TYPEOF_025",
        name: r"SCALAR_CAST_TYPEOF_025",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_025.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT typeof(25), typeof(25.5), typeof('25'), CAST('25' AS INTEGER)+1, CAST(25 AS TEXT)||'x';",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"integer|real|text|26|25x
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
