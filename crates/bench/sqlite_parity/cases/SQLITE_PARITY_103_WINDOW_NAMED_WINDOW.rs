// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_103_WINDOW_NAMED_WINDOW

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 103,
        folder: r"SQLITE_PARITY_103_WINDOW_NAMED_WINDOW",
        name: r"WINDOW_NAMED_WINDOW",
        category: r"SQL_WINDOW",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"Named WINDOW clause.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
WITH t(x) AS (VALUES(1),(2),(3))
SELECT x, sum(x) OVER win FROM t WINDOW win AS (ORDER BY x ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW);
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|1
2|3
3|6
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
