// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_795_WINDOW_PARTITION_SUM_008

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 795,
        folder: r"SQLITE_PARITY_795_WINDOW_PARTITION_SUM_008",
        name: r"WINDOW_PARTITION_SUM_008",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_008.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 9), (2, 10), (0, 11), (1, 12), (2, 13), (0, 14), (1, 15), (2, 16), (0, 17);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|11|1|11
0|14|2|25
0|17|3|42
1|9|1|9
1|12|2|21
1|15|3|36
2|10|1|10
2|13|2|23
2|16|3|39
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
