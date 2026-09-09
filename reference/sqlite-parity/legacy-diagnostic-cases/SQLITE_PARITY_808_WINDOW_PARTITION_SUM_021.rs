// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_808_WINDOW_PARTITION_SUM_021

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 808,
        folder: r"SQLITE_PARITY_808_WINDOW_PARTITION_SUM_021",
        name: r"WINDOW_PARTITION_SUM_021",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_021.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 22), (2, 23), (0, 24), (1, 25), (2, 26), (0, 27), (1, 28), (2, 29), (0, 30);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|24|1|24
0|27|2|51
0|30|3|81
1|22|1|22
1|25|2|47
1|28|3|75
2|23|1|23
2|26|2|49
2|29|3|78
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
