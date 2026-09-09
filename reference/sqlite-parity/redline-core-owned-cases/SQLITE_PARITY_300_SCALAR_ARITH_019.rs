// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_300_SCALAR_ARITH_019

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 300,
        folder: r"SQLITE_PARITY_300_SCALAR_ARITH_019",
        name: r"SCALAR_ARITH_019",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_019.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 19+38, 57-19, 19*20, (190)/19, (190)%20;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"57|38|380|10|10
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
