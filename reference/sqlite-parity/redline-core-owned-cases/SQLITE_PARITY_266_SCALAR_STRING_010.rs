// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_266_SCALAR_STRING_010

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 266,
        folder: r"SQLITE_PARITY_266_SCALAR_STRING_010",
        name: r"SCALAR_STRING_010",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_STRING_010.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT length('abc10'), substr('abcdef10',2,3), upper('a10b'), lower('A10B'), replace('a-b-c','-','0');",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"5|bcd|A10B|a10b|a0b0c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
