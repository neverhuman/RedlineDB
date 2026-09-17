// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_063_WINDOW_EXCLUDE_CURRENT_ROW

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 63,
        folder: r"SQLITE_PARITY_063_WINDOW_EXCLUDE_CURRENT_ROW",
        name: r"WINDOW_EXCLUDE_CURRENT_ROW",
        category: r"SQL_WINDOW",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"Window EXCLUDE CURRENT ROW.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
WITH t(x) AS (VALUES(1),(2),(3))
SELECT x, sum(x) OVER (
  ORDER BY x ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE CURRENT ROW
) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|5
2|4
3|3
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
