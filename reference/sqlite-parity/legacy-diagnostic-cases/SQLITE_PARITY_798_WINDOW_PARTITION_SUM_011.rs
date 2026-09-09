// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_798_WINDOW_PARTITION_SUM_011

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 798,
        folder: r"SQLITE_PARITY_798_WINDOW_PARTITION_SUM_011",
        name: r"WINDOW_PARTITION_SUM_011",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_011.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 12), (2, 13), (0, 14), (1, 15), (2, 16), (0, 17), (1, 18), (2, 19), (0, 20);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|14|1|14
0|17|2|31
0|20|3|51
1|12|1|12
1|15|2|27
1|18|3|45
2|13|1|13
2|16|2|29
2|19|3|48
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
