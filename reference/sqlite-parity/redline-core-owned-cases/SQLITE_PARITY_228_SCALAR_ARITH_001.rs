// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_228_SCALAR_ARITH_001

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 228,
        folder: r"SQLITE_PARITY_228_SCALAR_ARITH_001",
        name: r"SCALAR_ARITH_001",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_001.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 1+2, 3-1, 1*2, (10)/1, (10)%2;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"3|2|2|10|0
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
