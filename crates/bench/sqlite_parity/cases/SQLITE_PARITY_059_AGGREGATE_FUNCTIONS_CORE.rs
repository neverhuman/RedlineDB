// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_059_AGGREGATE_FUNCTIONS_CORE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 59,
        folder: r"SQLITE_PARITY_059_AGGREGATE_FUNCTIONS_CORE",
        name: r"AGGREGATE_FUNCTIONS_CORE",
        category: r"SQL_FUNCTIONS",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"count, sum, total, avg, min, max, group_concat.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
WITH t(x) AS (VALUES(1),(2),(NULL))
SELECT count(*), count(x), sum(x), total(x), avg(x), min(x), max(x), group_concat(x,':') FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"3|2|3|3.0|1.5|1|2|1:2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
