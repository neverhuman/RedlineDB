// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_356_SCALAR_ARITH_033

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 356,
        folder: r"SQLITE_PARITY_356_SCALAR_ARITH_033",
        name: r"SCALAR_ARITH_033",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_033.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 33+66, 99-33, 33*34, (330)/33, (330)%34;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"99|66|1122|10|24
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
