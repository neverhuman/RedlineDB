// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_793_WINDOW_PARTITION_SUM_006

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 793,
        folder: r"SQLITE_PARITY_793_WINDOW_PARTITION_SUM_006",
        name: r"WINDOW_PARTITION_SUM_006",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_006.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 7), (2, 8), (0, 9), (1, 10), (2, 11), (0, 12), (1, 13), (2, 14), (0, 15);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|9|1|9
0|12|2|21
0|15|3|36
1|7|1|7
1|10|2|17
1|13|3|30
2|8|1|8
2|11|2|19
2|14|3|33
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
