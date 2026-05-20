// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_807_WINDOW_PARTITION_SUM_020

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 807,
        folder: r"SQLITE_PARITY_807_WINDOW_PARTITION_SUM_020",
        name: r"WINDOW_PARTITION_SUM_020",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_020.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 21), (2, 22), (0, 23), (1, 24), (2, 25), (0, 26), (1, 27), (2, 28), (0, 29);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|23|1|23
0|26|2|49
0|29|3|78
1|21|1|21
1|24|2|45
1|27|3|72
2|22|1|22
2|25|2|47
2|28|3|75
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
