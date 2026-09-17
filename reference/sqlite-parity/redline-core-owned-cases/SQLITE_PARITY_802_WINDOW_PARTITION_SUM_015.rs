// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_802_WINDOW_PARTITION_SUM_015

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 802,
        folder: r"SQLITE_PARITY_802_WINDOW_PARTITION_SUM_015",
        name: r"WINDOW_PARTITION_SUM_015",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_015.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 16), (2, 17), (0, 18), (1, 19), (2, 20), (0, 21), (1, 22), (2, 23), (0, 24);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|18|1|18
0|21|2|39
0|24|3|63
1|16|1|16
1|19|2|35
1|22|3|57
2|17|1|17
2|20|2|37
2|23|3|60
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
