// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_834_WINDOW_PARTITION_SUM_047

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 834,
        folder: r"SQLITE_PARITY_834_WINDOW_PARTITION_SUM_047",
        name: r"WINDOW_PARTITION_SUM_047",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_047.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 48), (2, 49), (0, 50), (1, 51), (2, 52), (0, 53), (1, 54), (2, 55), (0, 56);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|50|1|50
0|53|2|103
0|56|3|159
1|48|1|48
1|51|2|99
1|54|3|153
2|49|1|49
2|52|2|101
2|55|3|156
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
