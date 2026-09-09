// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_813_WINDOW_PARTITION_SUM_026

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 813,
        folder: r"SQLITE_PARITY_813_WINDOW_PARTITION_SUM_026",
        name: r"WINDOW_PARTITION_SUM_026",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_026.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 27), (2, 28), (0, 29), (1, 30), (2, 31), (0, 32), (1, 33), (2, 34), (0, 35);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|29|1|29
0|32|2|61
0|35|3|96
1|27|1|27
1|30|2|57
1|33|3|90
2|28|1|28
2|31|2|59
2|34|3|93
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
