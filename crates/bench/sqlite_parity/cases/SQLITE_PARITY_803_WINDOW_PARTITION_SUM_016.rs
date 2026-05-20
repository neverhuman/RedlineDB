// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_803_WINDOW_PARTITION_SUM_016

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 803,
        folder: r"SQLITE_PARITY_803_WINDOW_PARTITION_SUM_016",
        name: r"WINDOW_PARTITION_SUM_016",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_016.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 17), (2, 18), (0, 19), (1, 20), (2, 21), (0, 22), (1, 23), (2, 24), (0, 25);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|19|1|19
0|22|2|41
0|25|3|66
1|17|1|17
1|20|2|37
1|23|3|60
2|18|1|18
2|21|2|39
2|24|3|63
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
