// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_298_SCALAR_STRING_018

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 298,
        folder: r"SQLITE_PARITY_298_SCALAR_STRING_018",
        name: r"SCALAR_STRING_018",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_018.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc18'), substr('abcdef18',2,3), upper('a18b'), lower('A18B'), replace('a-b-c','-','8');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"5|bcd|A18B|a18b|a8b8c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
