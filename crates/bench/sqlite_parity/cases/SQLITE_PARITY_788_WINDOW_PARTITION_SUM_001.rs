// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_788_WINDOW_PARTITION_SUM_001

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 788,
        folder: r"SQLITE_PARITY_788_WINDOW_PARTITION_SUM_001",
        name: r"WINDOW_PARTITION_SUM_001",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_001.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 2), (2, 3), (0, 4), (1, 5), (2, 6), (0, 7), (1, 8), (2, 9), (0, 10);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|4|1|4
0|7|2|11
0|10|3|21
1|2|1|2
1|5|2|7
1|8|3|15
2|3|1|3
2|6|2|9
2|9|3|18
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
