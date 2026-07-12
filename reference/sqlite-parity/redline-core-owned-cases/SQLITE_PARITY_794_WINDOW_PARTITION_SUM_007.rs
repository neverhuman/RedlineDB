// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_794_WINDOW_PARTITION_SUM_007

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 794,
        folder: r"SQLITE_PARITY_794_WINDOW_PARTITION_SUM_007",
        name: r"WINDOW_PARTITION_SUM_007",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_007.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 8), (2, 9), (0, 10), (1, 11), (2, 12), (0, 13), (1, 14), (2, 15), (0, 16);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|10|1|10
0|13|2|23
0|16|3|39
1|8|1|8
1|11|2|19
1|14|3|33
2|9|1|9
2|12|2|21
2|15|3|36
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
