// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_824_WINDOW_PARTITION_SUM_037

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 824,
        folder: r"SQLITE_PARITY_824_WINDOW_PARTITION_SUM_037",
        name: r"WINDOW_PARTITION_SUM_037",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_037.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 38), (2, 39), (0, 40), (1, 41), (2, 42), (0, 43), (1, 44), (2, 45), (0, 46);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|40|1|40
0|43|2|83
0|46|3|129
1|38|1|38
1|41|2|79
1|44|3|123
2|39|1|39
2|42|2|81
2|45|3|126
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
