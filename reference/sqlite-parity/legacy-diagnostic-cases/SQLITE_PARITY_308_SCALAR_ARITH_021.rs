// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_308_SCALAR_ARITH_021

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 308,
        folder: r"SQLITE_PARITY_308_SCALAR_ARITH_021",
        name: r"SCALAR_ARITH_021",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_021.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 21+42, 63-21, 21*22, (210)/21, (210)%22;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"63|42|462|10|12
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
