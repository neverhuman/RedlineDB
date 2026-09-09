// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_799_WINDOW_PARTITION_SUM_012

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 799,
        folder: r"SQLITE_PARITY_799_WINDOW_PARTITION_SUM_012",
        name: r"WINDOW_PARTITION_SUM_012",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_012.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 13), (2, 14), (0, 15), (1, 16), (2, 17), (0, 18), (1, 19), (2, 20), (0, 21);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|15|1|15
0|18|2|33
0|21|3|54
1|13|1|13
1|16|2|29
1|19|3|48
2|14|1|14
2|17|2|31
2|20|3|51
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
