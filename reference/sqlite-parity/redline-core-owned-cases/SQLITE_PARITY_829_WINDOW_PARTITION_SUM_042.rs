// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_829_WINDOW_PARTITION_SUM_042

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 829,
        folder: r"SQLITE_PARITY_829_WINDOW_PARTITION_SUM_042",
        name: r"WINDOW_PARTITION_SUM_042",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_042.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 43), (2, 44), (0, 45), (1, 46), (2, 47), (0, 48), (1, 49), (2, 50), (0, 51);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|45|1|45
0|48|2|93
0|51|3|144
1|43|1|43
1|46|2|89
1|49|3|138
2|44|1|44
2|47|2|91
2|50|3|141
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
