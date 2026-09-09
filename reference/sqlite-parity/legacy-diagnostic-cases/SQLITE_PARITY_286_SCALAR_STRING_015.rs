// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_286_SCALAR_STRING_015

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 286,
        folder: r"SQLITE_PARITY_286_SCALAR_STRING_015",
        name: r"SCALAR_STRING_015",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_015.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc15'), substr('abcdef15',2,3), upper('a15b'), lower('A15B'), replace('a-b-c','-','5');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"5|bcd|A15B|a15b|a5b5c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
