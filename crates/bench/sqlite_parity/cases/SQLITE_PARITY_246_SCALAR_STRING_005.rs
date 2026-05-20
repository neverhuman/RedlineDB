// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_246_SCALAR_STRING_005

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 246,
        folder: r"SQLITE_PARITY_246_SCALAR_STRING_005",
        name: r"SCALAR_STRING_005",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_005.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc5'), substr('abcdef5',2,3), upper('a5b'), lower('A5B'), replace('a-b-c','-','5');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"4|bcd|A5B|a5b|a5b5c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
