// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_060_FILTER_CLAUSE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 60,
        folder: r"SQLITE_PARITY_060_FILTER_CLAUSE",
        name: r"FILTER_CLAUSE",
        category: r"SQL_AGGREGATE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"Aggregate FILTER clause.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
WITH t(x) AS (VALUES(1),(2),(3),(4))
SELECT sum(x) FILTER (WHERE x%2=0), count(*) FILTER (WHERE x>2) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"6|2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
