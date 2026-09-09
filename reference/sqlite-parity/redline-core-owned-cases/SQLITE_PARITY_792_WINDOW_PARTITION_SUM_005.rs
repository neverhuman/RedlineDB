// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_792_WINDOW_PARTITION_SUM_005

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 792,
        folder: r"SQLITE_PARITY_792_WINDOW_PARTITION_SUM_005",
        name: r"WINDOW_PARTITION_SUM_005",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_005.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 6), (2, 7), (0, 8), (1, 9), (2, 10), (0, 11), (1, 12), (2, 13), (0, 14);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|8|1|8
0|11|2|19
0|14|3|33
1|6|1|6
1|9|2|15
1|12|3|27
2|7|1|7
2|10|2|17
2|13|3|30
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
