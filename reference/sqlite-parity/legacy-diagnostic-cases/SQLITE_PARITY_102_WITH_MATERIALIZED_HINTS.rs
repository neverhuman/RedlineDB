// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_102_WITH_MATERIALIZED_HINTS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 102,
        folder: r"SQLITE_PARITY_102_WITH_MATERIALIZED_HINTS",
        name: r"WITH_MATERIALIZED_HINTS",
        category: r"SQL_CTE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"AS MATERIALIZED and AS NOT MATERIALIZED CTE hints.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
WITH c(x) AS MATERIALIZED (SELECT 1) SELECT x FROM c;
WITH c(x) AS NOT MATERIALIZED (SELECT 2) SELECT x FROM c;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1
2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
