// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_352_SCALAR_ARITH_032

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 352,
        folder: r"SQLITE_PARITY_352_SCALAR_ARITH_032",
        name: r"SCALAR_ARITH_032",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_032.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 32+64, 96-32, 32*33, (320)/32, (320)%33;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"96|64|1056|10|23
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
