// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_851_WINDOW_PARTITION_SUM_064

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 851,
        folder: r"SQLITE_PARITY_851_WINDOW_PARTITION_SUM_064",
        name: r"WINDOW_PARTITION_SUM_064",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_064.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 65), (2, 66), (0, 67), (1, 68), (2, 69), (0, 70), (1, 71), (2, 72), (0, 73);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|67|1|67
0|70|2|137
0|73|3|210
1|65|1|65
1|68|2|133
1|71|3|204
2|66|1|66
2|69|2|135
2|72|3|207
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
