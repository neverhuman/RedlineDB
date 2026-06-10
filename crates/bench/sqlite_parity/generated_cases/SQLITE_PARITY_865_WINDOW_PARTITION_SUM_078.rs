// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_865_WINDOW_PARTITION_SUM_078

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 865,
        folder: r"SQLITE_PARITY_865_WINDOW_PARTITION_SUM_078",
        name: r"WINDOW_PARTITION_SUM_078",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_078.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 79), (2, 80), (0, 81), (1, 82), (2, 83), (0, 84), (1, 85), (2, 86), (0, 87);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|81|1|81
0|84|2|165
0|87|3|252
1|79|1|79
1|82|2|161
1|85|3|246
2|80|1|80
2|83|2|163
2|86|3|249
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
