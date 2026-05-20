// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_861_WINDOW_PARTITION_SUM_074

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 861,
        folder: r"SQLITE_PARITY_861_WINDOW_PARTITION_SUM_074",
        name: r"WINDOW_PARTITION_SUM_074",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_074.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 75), (2, 76), (0, 77), (1, 78), (2, 79), (0, 80), (1, 81), (2, 82), (0, 83);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|77|1|77
0|80|2|157
0|83|3|240
1|75|1|75
1|78|2|153
1|81|3|234
2|76|1|76
2|79|2|155
2|82|3|237
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
