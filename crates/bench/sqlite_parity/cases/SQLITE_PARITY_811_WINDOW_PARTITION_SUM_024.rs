// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_811_WINDOW_PARTITION_SUM_024

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 811,
        folder: r"SQLITE_PARITY_811_WINDOW_PARTITION_SUM_024",
        name: r"WINDOW_PARTITION_SUM_024",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_024.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 25), (2, 26), (0, 27), (1, 28), (2, 29), (0, 30), (1, 31), (2, 32), (0, 33);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|27|1|27
0|30|2|57
0|33|3|90
1|25|1|25
1|28|2|53
1|31|3|84
2|26|1|26
2|29|2|55
2|32|3|87
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
