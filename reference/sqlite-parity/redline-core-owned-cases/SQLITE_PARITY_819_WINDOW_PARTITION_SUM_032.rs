// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_819_WINDOW_PARTITION_SUM_032

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 819,
        folder: r"SQLITE_PARITY_819_WINDOW_PARTITION_SUM_032",
        name: r"WINDOW_PARTITION_SUM_032",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_032.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 33), (2, 34), (0, 35), (1, 36), (2, 37), (0, 38), (1, 39), (2, 40), (0, 41);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|35|1|35
0|38|2|73
0|41|3|114
1|33|1|33
1|36|2|69
1|39|3|108
2|34|1|34
2|37|2|71
2|40|3|111
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
