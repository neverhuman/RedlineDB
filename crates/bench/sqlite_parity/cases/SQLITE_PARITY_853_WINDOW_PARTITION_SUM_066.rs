// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_853_WINDOW_PARTITION_SUM_066

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 853,
        folder: r"SQLITE_PARITY_853_WINDOW_PARTITION_SUM_066",
        name: r"WINDOW_PARTITION_SUM_066",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_066.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 67), (2, 68), (0, 69), (1, 70), (2, 71), (0, 72), (1, 73), (2, 74), (0, 75);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|69|1|69
0|72|2|141
0|75|3|216
1|67|1|67
1|70|2|137
1|73|3|210
2|68|1|68
2|71|2|139
2|74|3|213
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
