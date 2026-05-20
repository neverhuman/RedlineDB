// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_312_SCALAR_ARITH_022

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 312,
        folder: r"SQLITE_PARITY_312_SCALAR_ARITH_022",
        name: r"SCALAR_ARITH_022",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_022.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 22+44, 66-22, 22*23, (220)/22, (220)%23;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"66|44|506|10|13
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
