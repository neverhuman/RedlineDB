// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_269_SCALAR_CAST_TYPEOF_011

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 269,
        folder: r"SQLITE_PARITY_269_SCALAR_CAST_TYPEOF_011",
        name: r"SCALAR_CAST_TYPEOF_011",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_011.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT typeof(11), typeof(11.5), typeof('11'), CAST('11' AS INTEGER)+1, CAST(11 AS TEXT)||'x';",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"integer|real|text|12|11x
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
