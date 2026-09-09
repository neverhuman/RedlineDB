// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_302_SCALAR_STRING_019

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 302,
        folder: r"SQLITE_PARITY_302_SCALAR_STRING_019",
        name: r"SCALAR_STRING_019",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_019.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc19'), substr('abcdef19',2,3), upper('a19b'), lower('A19B'), replace('a-b-c','-','9');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"5|bcd|A19B|a19b|a9b9c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
