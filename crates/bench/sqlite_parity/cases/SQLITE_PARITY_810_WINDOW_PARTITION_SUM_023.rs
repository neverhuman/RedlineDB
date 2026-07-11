// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_810_WINDOW_PARTITION_SUM_023

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 810,
        folder: r"SQLITE_PARITY_810_WINDOW_PARTITION_SUM_023",
        name: r"WINDOW_PARTITION_SUM_023",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_023.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 24), (2, 25), (0, 26), (1, 27), (2, 28), (0, 29), (1, 30), (2, 31), (0, 32);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|26|1|26
0|29|2|55
0|32|3|87
1|24|1|24
1|27|2|51
1|30|3|81
2|25|1|25
2|28|2|53
2|31|3|84
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
