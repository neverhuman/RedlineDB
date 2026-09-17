// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_250_SCALAR_STRING_006

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 250,
        folder: r"SQLITE_PARITY_250_SCALAR_STRING_006",
        name: r"SCALAR_STRING_006",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_006.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc6'), substr('abcdef6',2,3), upper('a6b'), lower('A6B'), replace('a-b-c','-','6');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"4|bcd|A6B|a6b|a6b6c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
