// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_859_WINDOW_PARTITION_SUM_072

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 859,
        folder: r"SQLITE_PARITY_859_WINDOW_PARTITION_SUM_072",
        name: r"WINDOW_PARTITION_SUM_072",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_072.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 73), (2, 74), (0, 75), (1, 76), (2, 77), (0, 78), (1, 79), (2, 80), (0, 81);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|75|1|75
0|78|2|153
0|81|3|234
1|73|1|73
1|76|2|149
1|79|3|228
2|74|1|74
2|77|2|151
2|80|3|231
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
