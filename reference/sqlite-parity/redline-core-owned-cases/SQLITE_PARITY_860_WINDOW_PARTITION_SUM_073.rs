// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_860_WINDOW_PARTITION_SUM_073

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 860,
        folder: r"SQLITE_PARITY_860_WINDOW_PARTITION_SUM_073",
        name: r"WINDOW_PARTITION_SUM_073",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_073.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 74), (2, 75), (0, 76), (1, 77), (2, 78), (0, 79), (1, 80), (2, 81), (0, 82);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|76|1|76
0|79|2|155
0|82|3|237
1|74|1|74
1|77|2|151
1|80|3|231
2|75|1|75
2|78|2|153
2|81|3|234
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
