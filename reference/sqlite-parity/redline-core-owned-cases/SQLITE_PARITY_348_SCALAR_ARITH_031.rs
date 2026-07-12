// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_348_SCALAR_ARITH_031

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 348,
        folder: r"SQLITE_PARITY_348_SCALAR_ARITH_031",
        name: r"SCALAR_ARITH_031",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_031.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 31+62, 93-31, 31*32, (310)/31, (310)%32;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"93|62|992|10|22
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
