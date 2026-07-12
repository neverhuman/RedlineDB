// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_282_SCALAR_STRING_014

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 282,
        folder: r"SQLITE_PARITY_282_SCALAR_STRING_014",
        name: r"SCALAR_STRING_014",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_014.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc14'), substr('abcdef14',2,3), upper('a14b'), lower('A14B'), replace('a-b-c','-','4');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"5|bcd|A14B|a14b|a4b4c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
