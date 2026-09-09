// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_791_WINDOW_PARTITION_SUM_004

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 791,
        folder: r"SQLITE_PARITY_791_WINDOW_PARTITION_SUM_004",
        name: r"WINDOW_PARTITION_SUM_004",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_004.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 5), (2, 6), (0, 7), (1, 8), (2, 9), (0, 10), (1, 11), (2, 12), (0, 13);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|7|1|7
0|10|2|17
0|13|3|30
1|5|1|5
1|8|2|13
1|11|3|24
2|6|1|6
2|9|2|15
2|12|3|27
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
