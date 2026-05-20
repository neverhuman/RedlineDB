// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_322_SCALAR_STRING_024

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 322,
        folder: r"SQLITE_PARITY_322_SCALAR_STRING_024",
        name: r"SCALAR_STRING_024",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_024.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc24'), substr('abcdef24',2,3), upper('a24b'), lower('A24B'), replace('a-b-c','-','4');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"5|bcd|A24B|a24b|a4b4c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
