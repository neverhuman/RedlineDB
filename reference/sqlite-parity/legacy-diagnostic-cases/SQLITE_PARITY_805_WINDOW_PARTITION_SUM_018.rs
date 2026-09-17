// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_805_WINDOW_PARTITION_SUM_018

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 805,
        folder: r"SQLITE_PARITY_805_WINDOW_PARTITION_SUM_018",
        name: r"WINDOW_PARTITION_SUM_018",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_018.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 19), (2, 20), (0, 21), (1, 22), (2, 23), (0, 24), (1, 25), (2, 26), (0, 27);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|21|1|21
0|24|2|45
0|27|3|72
1|19|1|19
1|22|2|41
1|25|3|66
2|20|1|20
2|23|2|43
2|26|3|69
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
