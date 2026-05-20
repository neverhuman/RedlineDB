// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_789_WINDOW_PARTITION_SUM_002

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 789,
        folder: r"SQLITE_PARITY_789_WINDOW_PARTITION_SUM_002",
        name: r"WINDOW_PARTITION_SUM_002",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_002.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 3), (2, 4), (0, 5), (1, 6), (2, 7), (0, 8), (1, 9), (2, 10), (0, 11);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|5|1|5
0|8|2|13
0|11|3|24
1|3|1|3
1|6|2|9
1|9|3|18
2|4|1|4
2|7|2|11
2|10|3|21
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
