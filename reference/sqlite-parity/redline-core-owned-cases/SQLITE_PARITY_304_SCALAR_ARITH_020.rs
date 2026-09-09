// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_304_SCALAR_ARITH_020

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 304,
        folder: r"SQLITE_PARITY_304_SCALAR_ARITH_020",
        name: r"SCALAR_ARITH_020",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_020.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 20+40, 60-20, 20*21, (200)/20, (200)%21;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"60|40|420|10|11
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
