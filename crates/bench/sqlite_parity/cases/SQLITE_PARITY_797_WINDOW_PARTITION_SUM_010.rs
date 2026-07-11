// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_797_WINDOW_PARTITION_SUM_010

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 797,
        folder: r"SQLITE_PARITY_797_WINDOW_PARTITION_SUM_010",
        name: r"WINDOW_PARTITION_SUM_010",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_010.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 11), (2, 12), (0, 13), (1, 14), (2, 15), (0, 16), (1, 17), (2, 18), (0, 19);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|13|1|13
0|16|2|29
0|19|3|48
1|11|1|11
1|14|2|25
1|17|3|42
2|12|1|12
2|15|2|27
2|18|3|45
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
