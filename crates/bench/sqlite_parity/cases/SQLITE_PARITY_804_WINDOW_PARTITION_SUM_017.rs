// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_804_WINDOW_PARTITION_SUM_017

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 804,
        folder: r"SQLITE_PARITY_804_WINDOW_PARTITION_SUM_017",
        name: r"WINDOW_PARTITION_SUM_017",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_017.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 18), (2, 19), (0, 20), (1, 21), (2, 22), (0, 23), (1, 24), (2, 25), (0, 26);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|20|1|20
0|23|2|43
0|26|3|69
1|18|1|18
1|21|2|39
1|24|3|63
2|19|1|19
2|22|2|41
2|25|3|66
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
