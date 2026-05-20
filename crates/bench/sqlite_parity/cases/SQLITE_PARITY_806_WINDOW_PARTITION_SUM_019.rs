// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_806_WINDOW_PARTITION_SUM_019

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 806,
        folder: r"SQLITE_PARITY_806_WINDOW_PARTITION_SUM_019",
        name: r"WINDOW_PARTITION_SUM_019",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_019.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 20), (2, 21), (0, 22), (1, 23), (2, 24), (0, 25), (1, 26), (2, 27), (0, 28);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|22|1|22
0|25|2|47
0|28|3|75
1|20|1|20
1|23|2|43
1|26|3|69
2|21|1|21
2|24|2|45
2|27|3|72
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
