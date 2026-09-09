// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_801_WINDOW_PARTITION_SUM_014

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 801,
        folder: r"SQLITE_PARITY_801_WINDOW_PARTITION_SUM_014",
        name: r"WINDOW_PARTITION_SUM_014",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_014.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 15), (2, 16), (0, 17), (1, 18), (2, 19), (0, 20), (1, 21), (2, 22), (0, 23);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|17|1|17
0|20|2|37
0|23|3|60
1|15|1|15
1|18|2|33
1|21|3|54
2|16|1|16
2|19|2|35
2|22|3|57
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
