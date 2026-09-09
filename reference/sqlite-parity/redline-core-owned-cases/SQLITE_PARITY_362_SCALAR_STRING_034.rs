// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_362_SCALAR_STRING_034

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 362,
        folder: r"SQLITE_PARITY_362_SCALAR_STRING_034",
        name: r"SCALAR_STRING_034",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_034.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc34'), substr('abcdef34',2,3), upper('a34b'), lower('A34B'), replace('a-b-c','-','4');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"5|bcd|A34B|a34b|a4b4c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
