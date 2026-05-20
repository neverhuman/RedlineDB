// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_064_CTE_NON_RECURSIVE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 64,
        folder: r"SQLITE_PARITY_064_CTE_NON_RECURSIVE",
        name: r"CTE_NON_RECURSIVE",
        category: r"SQL_CTE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"WITH non-recursive common table expression.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
WITH c(x) AS (SELECT 4)
SELECT x*2 FROM c;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"8
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
