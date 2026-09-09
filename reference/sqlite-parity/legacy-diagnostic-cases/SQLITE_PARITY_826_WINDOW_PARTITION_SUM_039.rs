// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_826_WINDOW_PARTITION_SUM_039

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 826,
        folder: r"SQLITE_PARITY_826_WINDOW_PARTITION_SUM_039",
        name: r"WINDOW_PARTITION_SUM_039",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_039.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 40), (2, 41), (0, 42), (1, 43), (2, 44), (0, 45), (1, 46), (2, 47), (0, 48);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|42|1|42
0|45|2|87
0|48|3|135
1|40|1|40
1|43|2|83
1|46|3|129
2|41|1|41
2|44|2|85
2|47|3|132
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
