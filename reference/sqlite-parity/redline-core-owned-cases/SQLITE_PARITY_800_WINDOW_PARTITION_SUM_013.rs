// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_800_WINDOW_PARTITION_SUM_013

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 800,
        folder: r"SQLITE_PARITY_800_WINDOW_PARTITION_SUM_013",
        name: r"WINDOW_PARTITION_SUM_013",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_013.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 14), (2, 15), (0, 16), (1, 17), (2, 18), (0, 19), (1, 20), (2, 21), (0, 22);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|16|1|16
0|19|2|35
0|22|3|57
1|14|1|14
1|17|2|31
1|20|3|51
2|15|1|15
2|18|2|33
2|21|3|54
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
