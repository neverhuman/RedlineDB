// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_817_WINDOW_PARTITION_SUM_030

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 817,
        folder: r"SQLITE_PARITY_817_WINDOW_PARTITION_SUM_030",
        name: r"WINDOW_PARTITION_SUM_030",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_030.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 31), (2, 32), (0, 33), (1, 34), (2, 35), (0, 36), (1, 37), (2, 38), (0, 39);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|33|1|33
0|36|2|69
0|39|3|108
1|31|1|31
1|34|2|65
1|37|3|102
2|32|1|32
2|35|2|67
2|38|3|105
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
