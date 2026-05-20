// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_278_SCALAR_STRING_013

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 278,
        folder: r"SQLITE_PARITY_278_SCALAR_STRING_013",
        name: r"SCALAR_STRING_013",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_013.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc13'), substr('abcdef13',2,3), upper('a13b'), lower('A13B'), replace('a-b-c','-','3');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"5|bcd|A13B|a13b|a3b3c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
