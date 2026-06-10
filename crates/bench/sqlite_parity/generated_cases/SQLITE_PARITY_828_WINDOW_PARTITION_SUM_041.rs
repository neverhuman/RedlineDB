// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_828_WINDOW_PARTITION_SUM_041

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 828,
        folder: r"SQLITE_PARITY_828_WINDOW_PARTITION_SUM_041",
        name: r"WINDOW_PARTITION_SUM_041",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_041.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 42), (2, 43), (0, 44), (1, 45), (2, 46), (0, 47), (1, 48), (2, 49), (0, 50);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|44|1|44
0|47|2|91
0|50|3|141
1|42|1|42
1|45|2|87
1|48|3|135
2|43|1|43
2|46|2|89
2|49|3|138
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
