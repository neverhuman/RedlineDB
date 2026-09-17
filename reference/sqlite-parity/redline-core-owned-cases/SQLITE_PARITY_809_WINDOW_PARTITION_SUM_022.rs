// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_809_WINDOW_PARTITION_SUM_022

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 809,
        folder: r"SQLITE_PARITY_809_WINDOW_PARTITION_SUM_022",
        name: r"WINDOW_PARTITION_SUM_022",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_022.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 23), (2, 24), (0, 25), (1, 26), (2, 27), (0, 28), (1, 29), (2, 30), (0, 31);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|25|1|25
0|28|2|53
0|31|3|84
1|23|1|23
1|26|2|49
1|29|3|78
2|24|1|24
2|27|2|51
2|30|3|81
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
