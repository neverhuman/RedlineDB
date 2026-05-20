// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_812_WINDOW_PARTITION_SUM_025

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 812,
        folder: r"SQLITE_PARITY_812_WINDOW_PARTITION_SUM_025",
        name: r"WINDOW_PARTITION_SUM_025",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_025.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 26), (2, 27), (0, 28), (1, 29), (2, 30), (0, 31), (1, 32), (2, 33), (0, 34);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|28|1|28
0|31|2|59
0|34|3|93
1|26|1|26
1|29|2|55
1|32|3|87
2|27|1|27
2|30|2|57
2|33|3|90
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
