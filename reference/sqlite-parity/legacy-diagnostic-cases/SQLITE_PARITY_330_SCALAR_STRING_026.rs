// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_330_SCALAR_STRING_026

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 330,
        folder: r"SQLITE_PARITY_330_SCALAR_STRING_026",
        name: r"SCALAR_STRING_026",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_026.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc26'), substr('abcdef26',2,3), upper('a26b'), lower('A26B'), replace('a-b-c','-','6');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"5|bcd|A26B|a26b|a6b6c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
