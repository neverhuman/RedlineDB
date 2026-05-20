// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_254_SCALAR_STRING_007

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 254,
        folder: r"SQLITE_PARITY_254_SCALAR_STRING_007",
        name: r"SCALAR_STRING_007",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_007.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc7'), substr('abcdef7',2,3), upper('a7b'), lower('A7B'), replace('a-b-c','-','7');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"4|bcd|A7B|a7b|a7b7c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
