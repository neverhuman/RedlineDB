// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_721_CTE_RECURSIVE_MATRIX_014

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 721,
        folder: r"SQLITE_PARITY_721_CTE_RECURSIVE_MATRIX_014",
        name: r"CTE_RECURSIVE_MATRIX_014",
        category: r"GEN_SQL_CTE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_014.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM c WHERE x < 3)
SELECT x, x*x FROM c ORDER BY x;
WITH data(v) AS (VALUES (14),(15),(16)) SELECT sum(v), max(v)-min(v) FROM data;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|1
2|4
3|9
45|2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
