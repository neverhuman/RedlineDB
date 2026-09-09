// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_360_SCALAR_ARITH_034

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 360,
        folder: r"SQLITE_PARITY_360_SCALAR_ARITH_034",
        name: r"SCALAR_ARITH_034",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_034.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 34+68, 102-34, 34*35, (340)/34, (340)%35;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"102|68|1190|10|25
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
