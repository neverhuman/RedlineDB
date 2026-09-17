// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_831_WINDOW_PARTITION_SUM_044

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 831,
        folder: r"SQLITE_PARITY_831_WINDOW_PARTITION_SUM_044",
        name: r"WINDOW_PARTITION_SUM_044",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_044.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 45), (2, 46), (0, 47), (1, 48), (2, 49), (0, 50), (1, 51), (2, 52), (0, 53);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|47|1|47
0|50|2|97
0|53|3|150
1|45|1|45
1|48|2|93
1|51|3|144
2|46|1|46
2|49|2|95
2|52|3|147
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
