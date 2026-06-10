// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_796_WINDOW_PARTITION_SUM_009

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 796,
        folder: r"SQLITE_PARITY_796_WINDOW_PARTITION_SUM_009",
        name: r"WINDOW_PARTITION_SUM_009",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_009.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 10), (2, 11), (0, 12), (1, 13), (2, 14), (0, 15), (1, 16), (2, 17), (0, 18);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|12|1|12
0|15|2|27
0|18|3|45
1|10|1|10
1|13|2|23
1|16|3|39
2|11|1|11
2|14|2|25
2|17|3|42
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
