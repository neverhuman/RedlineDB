// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_866_WINDOW_PARTITION_SUM_079

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 866,
        folder: r"SQLITE_PARITY_866_WINDOW_PARTITION_SUM_079",
        name: r"WINDOW_PARTITION_SUM_079",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_079.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 80), (2, 81), (0, 82), (1, 83), (2, 84), (0, 85), (1, 86), (2, 87), (0, 88);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|82|1|82
0|85|2|167
0|88|3|255
1|80|1|80
1|83|2|163
1|86|3|249
2|81|1|81
2|84|2|165
2|87|3|252
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
