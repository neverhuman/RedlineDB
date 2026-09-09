// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_062_WINDOW_FRAMES_ROWS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 62,
        folder: r"SQLITE_PARITY_062_WINDOW_FRAMES_ROWS",
        name: r"WINDOW_FRAMES_ROWS",
        category: r"SQL_WINDOW",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"Window frame ROWS BETWEEN 1 PRECEDING AND CURRENT ROW.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
WITH t(x) AS (VALUES(1),(2),(3))
SELECT x, sum(x) OVER (ORDER BY x ROWS BETWEEN 1 PRECEDING AND CURRENT ROW) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|1
2|3
3|5
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
