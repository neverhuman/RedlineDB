// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_832_WINDOW_PARTITION_SUM_045

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 832,
        folder: r"SQLITE_PARITY_832_WINDOW_PARTITION_SUM_045",
        name: r"WINDOW_PARTITION_SUM_045",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_045.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 46), (2, 47), (0, 48), (1, 49), (2, 50), (0, 51), (1, 52), (2, 53), (0, 54);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|48|1|48
0|51|2|99
0|54|3|153
1|46|1|46
1|49|2|95
1|52|3|147
2|47|1|47
2|50|2|97
2|53|3|150
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
