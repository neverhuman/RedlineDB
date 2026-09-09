// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_262_SCALAR_STRING_009

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 262,
        folder: r"SQLITE_PARITY_262_SCALAR_STRING_009",
        name: r"SCALAR_STRING_009",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_009.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc9'), substr('abcdef9',2,3), upper('a9b'), lower('A9B'), replace('a-b-c','-','9');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"4|bcd|A9B|a9b|a9b9c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
