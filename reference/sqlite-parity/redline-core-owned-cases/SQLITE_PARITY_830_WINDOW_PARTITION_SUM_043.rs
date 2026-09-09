// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_830_WINDOW_PARTITION_SUM_043

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 830,
        folder: r"SQLITE_PARITY_830_WINDOW_PARTITION_SUM_043",
        name: r"WINDOW_PARTITION_SUM_043",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_043.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 44), (2, 45), (0, 46), (1, 47), (2, 48), (0, 49), (1, 50), (2, 51), (0, 52);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|46|1|46
0|49|2|95
0|52|3|147
1|44|1|44
1|47|2|91
1|50|3|141
2|45|1|45
2|48|2|93
2|51|3|144
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
