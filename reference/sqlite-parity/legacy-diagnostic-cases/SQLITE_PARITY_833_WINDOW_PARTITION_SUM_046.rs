// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_833_WINDOW_PARTITION_SUM_046

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 833,
        folder: r"SQLITE_PARITY_833_WINDOW_PARTITION_SUM_046",
        name: r"WINDOW_PARTITION_SUM_046",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_046.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 47), (2, 48), (0, 49), (1, 50), (2, 51), (0, 52), (1, 53), (2, 54), (0, 55);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|49|1|49
0|52|2|101
0|55|3|156
1|47|1|47
1|50|2|97
1|53|3|150
2|48|1|48
2|51|2|99
2|54|3|153
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
