// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_815_WINDOW_PARTITION_SUM_028

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 815,
        folder: r"SQLITE_PARITY_815_WINDOW_PARTITION_SUM_028",
        name: r"WINDOW_PARTITION_SUM_028",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_028.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 29), (2, 30), (0, 31), (1, 32), (2, 33), (0, 34), (1, 35), (2, 36), (0, 37);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|31|1|31
0|34|2|65
0|37|3|102
1|29|1|29
1|32|2|61
1|35|3|96
2|30|1|30
2|33|2|63
2|36|3|99
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
