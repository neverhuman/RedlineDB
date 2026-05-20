// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_053_SELECT_WHERE_ORDER_LIMIT_OFFSET

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 53,
        folder: r"SQLITE_PARITY_053_SELECT_WHERE_ORDER_LIMIT_OFFSET",
        name: r"SELECT_WHERE_ORDER_LIMIT_OFFSET",
        category: r"SQL_SELECT",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"WHERE, ORDER BY, LIMIT, OFFSET.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
WITH vals(x) AS (VALUES(3),(1),(2))
SELECT x FROM vals WHERE x>1 ORDER BY x DESC LIMIT 1 OFFSET 0;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"3
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
