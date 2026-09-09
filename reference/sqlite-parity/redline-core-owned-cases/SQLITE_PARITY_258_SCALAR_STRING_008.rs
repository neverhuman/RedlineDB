// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_258_SCALAR_STRING_008

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 258,
        folder: r"SQLITE_PARITY_258_SCALAR_STRING_008",
        name: r"SCALAR_STRING_008",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_008.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc8'), substr('abcdef8',2,3), upper('a8b'), lower('A8B'), replace('a-b-c','-','8');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"4|bcd|A8B|a8b|a8b8c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
