// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_858_WINDOW_PARTITION_SUM_071

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 858,
        folder: r"SQLITE_PARITY_858_WINDOW_PARTITION_SUM_071",
        name: r"WINDOW_PARTITION_SUM_071",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_071.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 72), (2, 73), (0, 74), (1, 75), (2, 76), (0, 77), (1, 78), (2, 79), (0, 80);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|74|1|74
0|77|2|151
0|80|3|231
1|72|1|72
1|75|2|147
1|78|3|225
2|73|1|73
2|76|2|149
2|79|3|228
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
