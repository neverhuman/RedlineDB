// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_072_ORDER_BY_NULLS_FIRST_LAST

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 72,
        folder: r"SQLITE_PARITY_072_ORDER_BY_NULLS_FIRST_LAST",
        name: r"ORDER_BY_NULLS_FIRST_LAST",
        category: r"SQL_SELECT",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"ORDER BY NULLS FIRST/LAST.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
WITH v(x) AS (VALUES(NULL),(1),(2))
SELECT ifnull(x,99) FROM v ORDER BY x NULLS LAST;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1
2
99
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
